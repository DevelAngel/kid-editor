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
pub struct StrReplaceInput {
    path: String,
    /// Exact text to replace — must occur exactly once in the file
    old_str: String,
    /// Replacement text
    #[serde(default)]
    new_str: String,
}

#[tool_router(router = str_replace_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(description = "Replace an exact, unique occurrence of old_str with new_str in a file")]
    fn str_replace(
        &self,
        Parameters(input): Parameters<StrReplaceInput>,
    ) -> Result<CallToolResult, McpError> {
        let path = self.resolve(&input.path)?;
        let content =
            fs::read_to_string(&path).map_err(|e| tree::not_found_or_io(&input.path, e))?;

        let occurrences = content.matches(input.old_str.as_str()).count();
        if occurrences == 0 {
            return Err(McpError::invalid_params(
                format!("old_str not found in {}", input.path),
                None,
            ));
        }
        if occurrences > 1 {
            return Err(McpError::invalid_params(
                format!(
                    "old_str occurs {occurrences} times in {} — include more surrounding context to make it unique",
                    input.path
                ),
                None,
            ));
        }

        let updated = content.replacen(&input.old_str, &input.new_str, 1);
        fs::write(&path, updated)
            .map_err(|e| McpError::internal_error(format!("{}: {e}", input.path), None))?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "replaced 1 occurrence in {}",
            input.path
        ))]))
    }
}
