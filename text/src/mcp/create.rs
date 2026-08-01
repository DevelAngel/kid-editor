use super::McpService;

use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData as McpError};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::Deserialize;

use std::fs;

#[derive(Debug, Deserialize, JsonSchema)]
struct CreateInput {
    path: String,
    /// Full file content. Overwrites the file if it already exists.
    file_text: String,
}

#[tool_router(router = create_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(
        description = "Create a file with the given content, overwriting it if it already exists"
    )]
    fn create(
        &self,
        Parameters(input): Parameters<CreateInput>,
    ) -> Result<CallToolResult, McpError> {
        let path = self.resolve(&input.path)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| McpError::internal_error(format!("{}: {e}", input.path), None))?;
        }
        fs::write(&path, &input.file_text)
            .map_err(|e| McpError::internal_error(format!("{}: {e}", input.path), None))?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "wrote {}",
            input.path
        ))]))
    }
}
