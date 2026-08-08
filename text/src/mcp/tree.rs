use super::McpService;
use super::workspace_path::{IgnorePattern, UnresolvedPath, WorkspacePath, not_found_or_io};

use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData as McpError};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::Deserialize;

use std::fs::{self, ReadDir};

#[derive(Debug, Deserialize, JsonSchema)]
struct TreeInput {
    /// Directory to start from, relative or absolute (default: workspace root)
    #[serde(default)]
    path: Option<UnresolvedPath>,
    /// How many levels deep to show, 1 = only direct children (default: unlimited)
    #[serde(default)]
    max_depth: Option<usize>,
}

#[tool_router(router = tree_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(
        description = "Show a directory as a tree, like the Unix `tree` command — faster overview than repeated `fs_view` calls"
    )]
    fn fs_tree(
        &self,
        Parameters(input): Parameters<TreeInput>,
    ) -> Result<CallToolResult, McpError> {
        let resolved = input
            .path
            .map(|p| p.resolve(&self.workspace_root, &self.ignore))
            .transpose()?
            .unwrap_or_else(|| WorkspacePath::root(&self.workspace_root));

        let metadata = resolved
            .metadata()
            .map_err(|e| not_found_or_io(&resolved, e))?;
        if !metadata.is_dir() {
            return Err(McpError::invalid_params(
                format!("{resolved} is not a directory"),
                None,
            ));
        }

        let mut acc = TreeAccumulator {
            out: String::from(".\n"),
            dirs: 0,
            files: 0,
        };
        let root_entries = resolved
            .read_dir()
            .map_err(|e| McpError::internal_error(format!("{resolved}: {e}"), None))?;
        build_tree(root_entries, "", 0, input.max_depth, &self.ignore, &mut acc)?;
        let TreeAccumulator {
            mut out,
            dirs,
            files,
        } = acc;
        out.push_str(&format!(
            "\n{dirs} director{}, {files} file{}\n",
            if dirs == 1 { "y" } else { "ies" },
            if files == 1 { "" } else { "s" },
        ));

        Ok(CallToolResult::success(vec![ContentBlock::text(out)]))
    }
}

/// Accumulates `build_tree`'s output and running counts across recursive
/// calls — grouped into one struct so the recursion only threads one
/// mutable argument instead of three.
struct TreeAccumulator {
    out: String,
    dirs: usize,
    files: usize,
}

/// Recursively renders `entries`' contents in the style of the Unix `tree`
/// command (`├──`, `└──`, `│   ` continuation prefixes), depth-first,
/// directories and files sorted together by name. Counts are accumulated
/// into `acc` as it goes.
///
/// Only the top-level `ReadDir` comes from a resolved [`WorkspacePath`];
/// every recursive step below it reads a subdirectory the top level itself
/// already reported, so there is nothing left to re-resolve or re-check —
/// the ignore-name filter below is reapplied at every level regardless,
/// exactly as it always was.
fn build_tree(
    entries: ReadDir,
    prefix: &str,
    depth: usize,
    remaining_depth: Option<usize>,
    ignore: &[IgnorePattern],
    acc: &mut TreeAccumulator,
) -> Result<(), McpError> {
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

    let last_index = entries.len().checked_sub(1);
    for (i, entry) in entries.iter().enumerate() {
        let is_last = Some(i) == last_index;
        let connector = if is_last { "└── " } else { "├── " };
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.path().is_dir();

        acc.out.push_str(prefix);
        acc.out.push_str(connector);
        acc.out.push_str(&name);
        acc.out.push('\n');

        if is_dir {
            acc.dirs += 1;
            // `remaining_depth` counts levels still allowed to be *shown*.
            // At 1, this level was shown but its children are not.
            let descend = !matches!(remaining_depth, Some(d) if d <= 1);
            if descend {
                let child_prefix = format!("{prefix}{}", if is_last { "    " } else { "│   " });
                let next_depth = remaining_depth.map(|d| d - 1);
                let child_entries = fs::read_dir(entry.path()).map_err(|e| {
                    McpError::internal_error(format!("{}: {e}", entry.path().display()), None)
                })?;
                build_tree(
                    child_entries,
                    &child_prefix,
                    depth + 1,
                    next_depth,
                    ignore,
                    acc,
                )?;
            }
        } else {
            acc.files += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use indoc::indoc;
    use recipe::RecipeFile;
    use std::fs;

    #[test]
    fn tree_renders_nested_structure_with_counts() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/lib.rs"), "").unwrap();

        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        let result = svc
            .fs_tree(Parameters(TreeInput {
                path: None,
                max_depth: None,
            }))
            .unwrap();
        let text = match &result.content[0] {
            ContentBlock::Text(t) => t.text.clone(),
            _ => panic!("expected text content"),
        };

        assert_eq!(
            text,
            indoc! {"
                .
                ├── Cargo.toml
                └── src
                    └── lib.rs

                1 directory, 2 files
            "}
        );
    }

    #[test]
    fn tree_hides_ignored_entries() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join(".git/HEAD"), "").unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();

        let svc = McpService::new(
            dir.to_path_buf(),
            vec![".git".parse().unwrap(), "target".parse().unwrap()],
            RecipeFile::default(),
            None,
        );
        let result = svc
            .fs_tree(Parameters(TreeInput {
                path: None,
                max_depth: None,
            }))
            .unwrap();
        let text = match &result.content[0] {
            ContentBlock::Text(t) => t.text.clone(),
            _ => panic!("expected text content"),
        };

        assert_eq!(
            text,
            indoc! {"
                .
                └── Cargo.toml

                0 directories, 1 file
            "}
        );
    }

    #[test]
    fn tree_respects_max_depth() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("a/b")).unwrap();
        fs::write(dir.path().join("a/b/deep.txt"), "").unwrap();

        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        let result = svc
            .fs_tree(Parameters(TreeInput {
                path: None,
                max_depth: Some(1),
            }))
            .unwrap();
        let text = match &result.content[0] {
            ContentBlock::Text(t) => t.text.clone(),
            _ => panic!("expected text content"),
        };

        assert_eq!(
            text,
            indoc! {"
                .
                └── a

                1 directory, 0 files
            "}
        );
    }
}
