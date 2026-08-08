use super::McpService;
use super::workspace_path::{IgnorePattern, UnresolvedPath, WorkspacePath, not_found_or_io};

use anyhow::Result;
use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::sinks::UTF8;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData as McpError};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::Deserialize;

use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchInput {
    /// Exact text to search for (matched literally, not as a regex)
    query: String,
    /// File or directory to search in, relative or absolute (default: workspace root)
    #[serde(default)]
    path: Option<UnresolvedPath>,
    /// Case-insensitive match (default: false)
    #[serde(default)]
    case_insensitive: bool,
}

#[tool_router(router = search_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(
        description = "Search file contents for an exact substring, like `grep -F` — faster and more precise than reading files to look for text"
    )]
    fn fs_search(
        &self,
        Parameters(input): Parameters<SearchInput>,
    ) -> Result<CallToolResult, McpError> {
        let root = input
            .path
            .map(|p| p.resolve(&self.workspace_root, &self.ignore))
            .transpose()?
            .unwrap_or_else(|| WorkspacePath::root(&self.workspace_root));

        let search = WorkspaceSearch::new(&input.query, input.case_insensitive)?;
        let matches = search.run(&root, &self.ignore)?;

        if matches.is_empty() {
            return Ok(CallToolResult::success(vec![ContentBlock::text(
                "no matches found".to_owned(),
            )]));
        }

        let text = matches
            .iter()
            .map(SearchMatch::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }
}

/// One line matching a search query, workspace-relative for display.
struct SearchMatch {
    file: PathBuf,
    line: u64,
    text: String,
}

impl Display for SearchMatch {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.file.display(), self.line, self.text)
    }
}

/// Recursively greps every file under a [`WorkspacePath`] for a literal
/// query, honoring the same ignore rules `fs_tree` applies. A thin
/// wrapper around a [`RegexMatcher`] built from an escaped, literal
/// pattern — "search", not "regex search".
struct WorkspaceSearch {
    matcher: RegexMatcher,
}

impl WorkspaceSearch {
    fn new(query: &str, case_insensitive: bool) -> Result<Self, McpError> {
        let matcher = RegexMatcherBuilder::new()
            .case_insensitive(case_insensitive)
            .build(&regex::escape(query))
            .map_err(|e| McpError::internal_error(format!("invalid search query: {e}"), None))?;
        Ok(Self { matcher })
    }

    /// Searches `root` — a file or a directory, walked recursively — and
    /// returns every match sorted by file, then line number.
    fn run(
        &self,
        root: &WorkspacePath,
        ignore: &[IgnorePattern],
    ) -> Result<Vec<SearchMatch>, McpError> {
        let metadata = root.metadata().map_err(|e| not_found_or_io(root, e))?;
        let mut matches = Vec::new();
        if metadata.is_dir() {
            self.walk_dir(root.absolute(), root.relative(), 0, ignore, &mut matches)?;
        } else {
            self.search_file(root.absolute(), root.relative(), &mut matches)?;
        }
        matches.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
        Ok(matches)
    }

    /// Depth-first walk mirroring `tree::build_tree`'s ignore handling:
    /// every entry is checked against `ignore` by name and depth before
    /// being descended into or searched.
    fn walk_dir(
        &self,
        dir: &Path,
        relative: &Path,
        depth: usize,
        ignore: &[IgnorePattern],
        matches: &mut Vec<SearchMatch>,
    ) -> Result<(), McpError> {
        let entries = fs::read_dir(dir)
            .map_err(|e| McpError::internal_error(format!("{}: {e}", dir.display()), None))?;
        let mut entries: Vec<_> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                !ignore
                    .iter()
                    .any(|pattern| pattern.matches_name_at_depth(&name, depth))
            })
            .collect();
        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            let child_absolute = entry.path();
            let child_relative = relative.join(entry.file_name());
            if child_absolute.is_dir() {
                self.walk_dir(&child_absolute, &child_relative, depth + 1, ignore, matches)?;
            } else {
                self.search_file(&child_absolute, &child_relative, matches)?;
            }
        }
        Ok(())
    }

    /// Searches one file, appending any matches to `matches`. Binary
    /// files are detected via a NUL byte and skipped, same convention as
    /// `grep`/`ripgrep`.
    fn search_file(
        &self,
        absolute: &Path,
        relative: &Path,
        matches: &mut Vec<SearchMatch>,
    ) -> Result<(), McpError> {
        let mut searcher = SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(0))
            .build();
        searcher
            .search_path(
                &self.matcher,
                absolute,
                UTF8(|line_number, line| {
                    matches.push(SearchMatch {
                        file: relative.to_path_buf(),
                        line: line_number,
                        text: line.trim_end().to_owned(),
                    });
                    Ok(true)
                }),
            )
            .map_err(|e| McpError::internal_error(format!("{}: {e}", absolute.display()), None))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use recipe::RecipeFile;
    use std::fs;

    fn service_with(dir: &TempDir, ignore: &[&str]) -> McpService {
        let ignore = ignore.iter().map(|p| p.parse().unwrap()).collect();
        McpService::new(dir.to_path_buf(), ignore, RecipeFile::default(), None)
    }

    fn search_text(svc: &McpService, query: &str, case_insensitive: bool) -> String {
        let result = svc
            .fs_search(Parameters(SearchInput {
                query: query.to_owned(),
                path: None,
                case_insensitive,
            }))
            .unwrap();
        match &result.content[0] {
            ContentBlock::Text(t) => t.text.clone(),
            _ => panic!("expected text content"),
        }
    }

    #[test]
    fn finds_match_in_single_file() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("f.txt"), "hello world\n").unwrap();
        let svc = service_with(&dir, &[]);
        assert_eq!(search_text(&svc, "world", false), "f.txt:1:hello world");
    }

    #[test]
    fn finds_matches_across_multiple_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "needle here\n").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/b.txt"), "needle there\n").unwrap();
        let svc = service_with(&dir, &[]);
        assert_eq!(
            search_text(&svc, "needle", false),
            "a.txt:1:needle here\nsub/b.txt:1:needle there"
        );
    }

    #[test]
    fn case_insensitive_flag_controls_matching() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("f.txt"), "Hello\n").unwrap();
        let svc = service_with(&dir, &[]);
        assert_eq!(search_text(&svc, "hello", false), "no matches found");
        assert_eq!(search_text(&svc, "hello", true), "f.txt:1:Hello");
    }

    #[test]
    fn ignored_paths_are_not_searched() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("target/out.txt"), "needle\n").unwrap();
        fs::write(dir.path().join("keep.txt"), "needle\n").unwrap();
        let svc = service_with(&dir, &["target"]);
        assert_eq!(search_text(&svc, "needle", false), "keep.txt:1:needle");
    }

    #[test]
    fn no_match_returns_empty_result_not_error() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("f.txt"), "hello\n").unwrap();
        let svc = service_with(&dir, &[]);
        assert_eq!(search_text(&svc, "absent", false), "no matches found");
    }
}
