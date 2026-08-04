use super::McpService;
use super::workspace_path::{UnresolvedPath, not_found_or_io};

use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData as McpError};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::Deserialize;

use std::io::Write;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InsertInput {
    path: UnresolvedPath,
    /// Line number after which to insert; 0 inserts at the start of the file
    insert_line: usize,
    new_str: String,
}

#[tool_router(router = insert_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(description = "Insert text after a given line number (0 = start of file)")]
    fn insert(
        &self,
        Parameters(input): Parameters<InsertInput>,
    ) -> Result<CallToolResult, McpError> {
        let path = input.path.resolve(&self.workspace_root, &self.ignore)?;
        let content = path
            .read_to_string()
            .map_err(|e| not_found_or_io(&path, e))?;

        let mut lines: Vec<&str> = content.lines().collect();
        if input.insert_line > lines.len() {
            return Err(McpError::invalid_params(
                format!(
                    "insert_line {} is past the end of the file ({} lines)",
                    input.insert_line,
                    lines.len()
                ),
                None,
            ));
        }
        lines.insert(input.insert_line, input.new_str.as_str());
        let mut updated = lines.join("\n");
        if content.ends_with('\n') {
            updated.push('\n');
        }
        let write = path.into_write_buffer()?;
        write
            .open()
            .and_then(|mut file| file.write_all(updated.as_bytes()))
            .map_err(|e| McpError::internal_error(format!("{write}: {e}"), None))?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "inserted after line {} in {write}",
            input.insert_line
        ))]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use std::collections::BTreeMap;
    use std::fs;

    #[test]
    fn insert_adds_line_after_given_index() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], BTreeMap::new());
        fs::write(dir.path().join("f.txt"), "a\nb\n").unwrap();
        svc.insert(Parameters(InsertInput {
            path: UnresolvedPath::new("f.txt"),
            insert_line: 1,
            new_str: "x".into(),
        }))
        .unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "a\nx\nb\n");
    }
}
