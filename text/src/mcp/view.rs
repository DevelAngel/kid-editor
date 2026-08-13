use super::McpService;
use super::workspace_path::{UnresolvedPath, WorkspacePath, not_found_or_io};

use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData as McpError};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct ViewInput {
    /// Path to a file or directory, relative or absolute (resolved against the workspace root)
    pub path: UnresolvedPath,
    /// Optional 1-indexed inclusive line range, e.g. [1, 50]. Only valid for files.
    #[serde(default)]
    pub view_range: Option<[usize; 2]>,
}

#[tool_router(router = view_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(
        description = "View a file's contents (numbered lines, optionally a line range) or list a directory's entries",
        annotations(
            title = "View File",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    fn fs_view(
        &self,
        Parameters(input): Parameters<ViewInput>,
    ) -> Result<CallToolResult, McpError> {
        let path = input.path.resolve(&self.workspace_root, &self.ignore)?;
        let metadata = path.metadata().map_err(|e| not_found_or_io(&path, e))?;

        if metadata.is_dir() {
            if input.view_range.is_some() {
                return Err(McpError::invalid_params(
                    "view_range is only valid for files",
                    None,
                ));
            }
            return Ok(CallToolResult::success(vec![ContentBlock::text(
                list_directory(&path)?,
            )]));
        }

        let content = path
            .read_to_string()
            .map_err(|e| McpError::internal_error(format!("{path}: {e}"), None))?;
        let text = render_numbered(&content, input.view_range)?;
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }
}

fn render_numbered(content: &str, view_range: Option<[usize; 2]>) -> Result<String, McpError> {
    let lines: Vec<&str> = content.lines().collect();
    let (start, end) = match view_range {
        Some([start, end]) => {
            if start == 0 || start > lines.len() || end < start {
                return Err(McpError::invalid_params(
                    format!(
                        "invalid view_range [{start}, {end}] for a {}-line file",
                        lines.len()
                    ),
                    None,
                ));
            }
            (start, end.min(lines.len()))
        }
        None => (1, lines.len()),
    };

    let mut out = String::with_capacity(content.len() + lines.len() * 8);
    for (i, line) in lines[start - 1..end].iter().enumerate() {
        out.push_str(&format!("{:6}\t{}\n", start + i, line));
    }
    Ok(out)
}

fn list_directory(path: &WorkspacePath) -> Result<String, McpError> {
    let mut entries: Vec<String> = path
        .read_dir()
        .map_err(|e| McpError::internal_error(format!("{path}: {e}"), None))?
        .filter_map(|entry| entry.ok())
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_dir() {
                format!("{name}/")
            } else {
                name
            }
        })
        .collect();
    entries.sort();
    Ok(entries.join("\n"))
}
