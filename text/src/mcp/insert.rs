use super::McpService;
use super::tree;

use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData as McpError};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::Deserialize;

use std::fs;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InsertInput {
    path: String,
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
        let path = self.resolve(&input.path)?;
        let content =
            fs::read_to_string(&path).map_err(|e| tree::not_found_or_io(&input.path, e))?;

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
        fs::write(&path, updated)
            .map_err(|e| McpError::internal_error(format!("{}: {e}", input.path), None))?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "inserted after line {} in {}",
            input.insert_line, input.path
        ))]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use std::fs;

    #[test]
    fn insert_adds_line_after_given_index() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf());
        fs::write(dir.path().join("f.txt"), "a\nb\n").unwrap();
        svc.insert(Parameters(InsertInput {
            path: "f.txt".into(),
            insert_line: 1,
            new_str: "x".into(),
        }))
        .unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "a\nx\nb\n");
    }
}
