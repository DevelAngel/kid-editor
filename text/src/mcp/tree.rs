use super::McpService;
use super::workspace_path::{UnresolvedPath, not_found_or_io};

use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData as McpError};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::Deserialize;

use std::fs;
use std::path::Path;

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
        description = "Show a directory as a tree, like the Unix `tree` command — faster overview than repeated `view` calls"
    )]
    fn tree(&self, Parameters(input): Parameters<TreeInput>) -> Result<CallToolResult, McpError> {
        let resolved = input
            .path
            .map(|p| p.resolve(&self.workspace_root, &self.ignore))
            .transpose()?;
        // `resolved` stays a `WorkspacePath` (or absent, meaning "the
        // workspace root itself" — already trusted, nothing to resolve)
        // for its whole lifetime here; only borrow from it at the point of
        // use, never take it apart into loose `PathBuf`/`String` values.
        let root: &Path = resolved
            .as_ref()
            .map_or(&self.workspace_root, |w| w.as_path());
        let display = resolved
            .as_ref()
            .map_or_else(|| ".".to_owned(), ToString::to_string);

        let metadata = fs::metadata(root).map_err(|e| not_found_or_io(&display, e))?;
        if !metadata.is_dir() {
            return Err(McpError::invalid_params(
                format!("{display} is not a directory"),
                None,
            ));
        }

        let mut out = String::from(".\n");
        let mut dirs = 0usize;
        let mut files = 0usize;
        build_tree(
            root,
            "",
            input.max_depth,
            &self.ignore,
            &mut out,
            &mut dirs,
            &mut files,
        )?;
        out.push_str(&format!(
            "\n{dirs} director{}, {files} file{}\n",
            if dirs == 1 { "y" } else { "ies" },
            if files == 1 { "" } else { "s" },
        ));

        Ok(CallToolResult::success(vec![ContentBlock::text(out)]))
    }
}

/// Recursively renders `dir`'s contents in the style of the Unix `tree`
/// command (`├──`, `└──`, `│   ` continuation prefixes), depth-first,
/// directories and files sorted together by name. Counts are accumulated
/// into `dirs`/`files` as it goes.
fn build_tree(
    dir: &Path,
    prefix: &str,
    remaining_depth: Option<usize>,
    ignore: &[String],
    out: &mut String,
    dirs: &mut usize,
    files: &mut usize,
) -> Result<(), McpError> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| McpError::internal_error(format!("{}: {e}", dir.display()), None))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let name = entry.file_name();
            !ignore
                .iter()
                .any(|ignored| ignored.as_str() == name.to_string_lossy())
        })
        .collect();
    entries.sort_by_key(|entry| entry.file_name());

    let last_index = entries.len().checked_sub(1);
    for (i, entry) in entries.iter().enumerate() {
        let is_last = Some(i) == last_index;
        let connector = if is_last { "└── " } else { "├── " };
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.path().is_dir();

        out.push_str(prefix);
        out.push_str(connector);
        out.push_str(&name);
        out.push('\n');

        if is_dir {
            *dirs += 1;
            // `remaining_depth` counts levels still allowed to be *shown*.
            // At 1, this level was shown but its children are not.
            let descend = !matches!(remaining_depth, Some(d) if d <= 1);
            if descend {
                let child_prefix = format!("{prefix}{}", if is_last { "    " } else { "│   " });
                let next_depth = remaining_depth.map(|d| d - 1);
                build_tree(
                    &entry.path(),
                    &child_prefix,
                    next_depth,
                    ignore,
                    out,
                    dirs,
                    files,
                )?;
            }
        } else {
            *files += 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use indoc::indoc;
    use std::collections::HashSet;
    use std::fs;

    #[test]
    fn tree_renders_nested_structure_with_counts() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        fs::create_dir(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/lib.rs"), "").unwrap();

        let svc = McpService::new(dir.to_path_buf(), vec![], HashSet::new());
        let result = svc
            .tree(Parameters(TreeInput {
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
            vec![".git".to_string(), "target".to_string()],
            HashSet::new(),
        );
        let result = svc
            .tree(Parameters(TreeInput {
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

        let svc = McpService::new(dir.to_path_buf(), vec![], HashSet::new());
        let result = svc
            .tree(Parameters(TreeInput {
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
