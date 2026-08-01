use super::McpService;

use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData as McpError};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::Deserialize;

use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, JsonSchema)]
struct ViewInput {
    /// Path to a file or directory, relative or absolute (resolved against the workspace root)
    pub path: String,
    /// Optional 1-indexed inclusive line range, e.g. [1, 50]. Only valid for files.
    #[serde(default)]
    pub view_range: Option<[usize; 2]>,
}

#[tool_router(router = tree_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(
        description = "View a file's contents (numbered lines, optionally a line range) or list a directory's entries"
    )]
    fn view(&self, Parameters(input): Parameters<ViewInput>) -> Result<CallToolResult, McpError> {
        let path = self.resolve(&input.path)?;
        let metadata = fs::metadata(&path).map_err(|e| not_found_or_io(&input.path, e))?;

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

        let content = std::fs::read_to_string(&path)
            .map_err(|e| McpError::internal_error(format!("{}: {e}", input.path), None))?;
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

fn list_directory(path: &Path) -> Result<String, McpError> {
    let mut entries: Vec<String> = std::fs::read_dir(path)
        .map_err(|e| McpError::internal_error(format!("{}: {e}", path.display()), None))?
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

fn not_found_or_io(display_path: &str, e: std::io::Error) -> McpError {
    if e.kind() == std::io::ErrorKind::NotFound {
        McpError::invalid_params(format!("{display_path}: no such file or directory"), None)
    } else {
        McpError::internal_error(format!("{display_path}: {e}"), None)
    }
}
