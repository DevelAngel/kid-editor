use super::McpService;
use super::workspace_path::UnresolvedPath;

use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData as McpError};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::Deserialize;

use std::io::Write;

#[derive(Debug, Deserialize, JsonSchema)]
struct CreateInput {
    path: UnresolvedPath,
    /// Full file content. Overwrites the file if it already exists.
    file_text: String,
}

#[tool_router(router = create_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(
        description = "Create a file with the given content, overwriting it if it already exists"
    )]
    fn fs_create(
        &self,
        Parameters(input): Parameters<CreateInput>,
    ) -> Result<CallToolResult, McpError> {
        let path = input.path.resolve(&self.workspace_root, &self.ignore)?;
        let write = path.into_write_buffer(self.recipe_toml_protected_path.as_deref())?;
        write
            .open()
            .and_then(|mut file| file.write_all(input.file_text.as_bytes()))
            .map_err(|e| McpError::internal_error(format!("{write}: {e}"), None))?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "wrote {write}"
        ))]))
    }
}
