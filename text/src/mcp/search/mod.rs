mod exact;
mod fuzzy;
mod semantic;

use super::McpService;
use super::workspace_path::{UnresolvedPath, WorkspacePath};
use exact::WorkspaceSearch;
use fuzzy::FuzzySearch;

use anyhow::Result as AnyResult;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData as McpError};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::Deserialize;

use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;
use std::sync::OnceLock;

/// Process-wide, not per-workspace: the model is the same regardless
/// of which workspace is being searched, and loading it is expensive
/// enough (network fetch on a cold `hf-hub` cache) that every
/// `SearchMode::Semantic` call should share one instance.
static SEMANTIC_EMBEDDER: OnceLock<AnyResult<semantic::MiniLmEmbedder>> = OnceLock::new();

#[derive(Debug, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
enum SearchMode {
    /// Literal substring match
    Exact {
        /// Case-insensitive match (default: false)
        #[serde(default)]
        case_insensitive: bool,
    },
    /// Approximate, typo-tolerant match ranked by relevance — already
    /// matches case-insensitively unless the query itself contains an
    /// uppercase letter
    Fuzzy,
    /// Meaning-based match ranked by cosine similarity of a local
    /// embedding model. First call may be slower while the model
    /// cache and the workspace's semantic index warm up.
    Semantic,
}

impl Default for SearchMode {
    fn default() -> Self {
        Self::Exact {
            case_insensitive: false,
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchInput {
    /// Text to search for
    query: String,
    /// File or directory to search in, relative or absolute (default: workspace root)
    #[serde(default)]
    path: Option<UnresolvedPath>,
    /// Search mode (default: exact)
    #[serde(default)]
    mode: SearchMode,
}

#[tool_router(router = search_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(
        description = "Search file contents for text. Faster and more precise than reading files to look for text."
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

        let matches = match input.mode {
            SearchMode::Exact { case_insensitive } => {
                let search = WorkspaceSearch::new(&input.query, case_insensitive)?;
                search.run(&root, &self.ignore)?
            }
            SearchMode::Fuzzy => {
                let mut search = FuzzySearch::new(&input.query);
                search.run(&root, &self.ignore)?
            }
            SearchMode::Semantic => {
                let embedder = SEMANTIC_EMBEDDER.get_or_init(semantic::MiniLmEmbedder::load);
                let embedder = embedder.as_ref().map_err(|err| {
                    McpError::internal_error(format!("semantic search unavailable: {err}"), None)
                })?;
                let workspace = WorkspacePath::root(&self.workspace_root);
                semantic::SemanticSearch::new(embedder).run(
                    &input.query,
                    &root,
                    &workspace,
                    &self.ignore,
                )?
            }
        };

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
        write!(f, "{}:{}: {}", self.file.display(), self.line, self.text)
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
        search_text_mode(svc, query, SearchMode::Exact { case_insensitive })
    }

    fn search_text_mode(svc: &McpService, query: &str, mode: SearchMode) -> String {
        let result = svc
            .fs_search(Parameters(SearchInput {
                query: query.to_owned(),
                path: None,
                mode,
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
        assert_eq!(search_text(&svc, "world", false), "f.txt:1: hello world");
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
            "a.txt:1: needle here\nsub/b.txt:1: needle there"
        );
    }

    #[test]
    fn case_insensitive_flag_controls_matching() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("f.txt"), "Hello\n").unwrap();
        let svc = service_with(&dir, &[]);
        assert_eq!(search_text(&svc, "hello", false), "no matches found");
        assert_eq!(search_text(&svc, "hello", true), "f.txt:1: Hello");
    }

    #[test]
    fn ignored_paths_are_not_searched() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("target/out.txt"), "needle\n").unwrap();
        fs::write(dir.path().join("keep.txt"), "needle\n").unwrap();
        let svc = service_with(&dir, &["target"]);
        assert_eq!(search_text(&svc, "needle", false), "keep.txt:1: needle");
    }

    #[test]
    fn no_match_returns_empty_result_not_error() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("f.txt"), "hello\n").unwrap();
        let svc = service_with(&dir, &[]);
        assert_eq!(search_text(&svc, "absent", false), "no matches found");
    }

    #[test]
    fn fuzzy_finds_match_despite_gaps() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("f.txt"), "workspace_root\n").unwrap();
        let svc = service_with(&dir, &[]);
        assert_eq!(
            search_text_mode(&svc, "wsroot", SearchMode::Fuzzy),
            "f.txt:1: workspace_root"
        );
    }

    #[test]
    fn fuzzy_ranks_best_match_first() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("f.txt"),
            "search\nresearching everything else\n",
        )
        .unwrap();
        let svc = service_with(&dir, &[]);
        assert_eq!(
            search_text_mode(&svc, "search", SearchMode::Fuzzy),
            "f.txt:1: search\nf.txt:2: researching everything else"
        );
    }

    #[test]
    fn fuzzy_ignores_ignored_paths() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("target/out.txt"), "needle\n").unwrap();
        fs::write(dir.path().join("keep.txt"), "needle\n").unwrap();
        let svc = service_with(&dir, &["target"]);
        assert_eq!(
            search_text_mode(&svc, "needle", SearchMode::Fuzzy),
            "keep.txt:1: needle"
        );
    }

    #[test]
    fn fuzzy_no_match_returns_empty_result_not_error() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("f.txt"), "hello\n").unwrap();
        let svc = service_with(&dir, &[]);
        assert_eq!(
            search_text_mode(&svc, "zzz", SearchMode::Fuzzy),
            "no matches found"
        );
    }
}
