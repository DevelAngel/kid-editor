use super::SearchMatch;
use crate::mcp::workspace_path::{IgnorePattern, WorkspacePath, not_found_or_io};

use grep_regex::{RegexMatcher, RegexMatcherBuilder};
use grep_searcher::sinks::UTF8;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use rmcp::model::ErrorData as McpError;

use std::fs;
use std::path::Path;

/// Recursively greps every file under a [`WorkspacePath`] for a literal
/// query, honoring the same ignore rules `fs_tree` applies. A thin
/// wrapper around a [`RegexMatcher`] built from an escaped, literal
/// pattern — "search", not "regex search".
pub(super) struct WorkspaceSearch {
    matcher: RegexMatcher,
}

impl WorkspaceSearch {
    pub(super) fn new(query: &str, case_insensitive: bool) -> Result<Self, McpError> {
        let matcher = RegexMatcherBuilder::new()
            .case_insensitive(case_insensitive)
            .build(&regex::escape(query))
            .map_err(|e| McpError::internal_error(format!("invalid search query: {e}"), None))?;
        Ok(Self { matcher })
    }

    /// Searches `root` — a file or a directory, walked recursively — and
    /// returns every match sorted by file, then line number.
    pub(super) fn run(
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
