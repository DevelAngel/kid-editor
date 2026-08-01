use super::McpService;

use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData as McpError};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::Deserialize;

use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Deserialize, JsonSchema)]
struct TreeInput {
    /// Directory to start from, relative or absolute (default: workspace root)
    #[serde(default)]
    path: Option<String>,
    /// How many levels deep to show, 1 = only direct children (default: unlimited)
    #[serde(default)]
    max_depth: Option<usize>,
}

#[tool_router(router = view_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(
        description = "Show a directory as a tree, like the Unix `tree` command — faster overview than repeated `view` calls"
    )]
    fn tree(&self, Parameters(input): Parameters<TreeInput>) -> Result<CallToolResult, McpError> {
        let root = match &input.path {
            Some(p) => self.resolve(p)?,
            None => self.workspace_root.clone(),
        };
        let metadata = fs::metadata(&root)
            .map_err(|e| not_found_or_io(input.path.as_deref().unwrap_or("."), e))?;
        if !metadata.is_dir() {
            return Err(McpError::invalid_params(
                format!(
                    "{} is not a directory",
                    input.path.as_deref().unwrap_or(".")
                ),
                None,
            ));
        }

        let mut out = String::from(".\n");
        let mut dirs = 0usize;
        let mut files = 0usize;
        build_tree(&root, "", input.max_depth, &mut out, &mut dirs, &mut files)?;
        out.push_str(&format!(
            "\n{dirs} director{}, {files} file{}\n",
            if dirs == 1 { "y" } else { "ies" },
            if files == 1 { "" } else { "s" },
        ));

        Ok(CallToolResult::success(vec![ContentBlock::text(out)]))
    }
}

fn not_found_or_io(display_path: &str, e: io::Error) -> McpError {
    if e.kind() == io::ErrorKind::NotFound {
        McpError::invalid_params(format!("{display_path}: no such file or directory"), None)
    } else {
        McpError::internal_error(format!("{display_path}: {e}"), None)
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
    out: &mut String,
    dirs: &mut usize,
    files: &mut usize,
) -> Result<(), McpError> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| McpError::internal_error(format!("{}: {e}", dir.display()), None))?
        .filter_map(|entry| entry.ok())
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
                build_tree(&entry.path(), &child_prefix, next_depth, out, dirs, files)?;
            }
        } else {
            *files += 1;
        }
    }
    Ok(())
}
