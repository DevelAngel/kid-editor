use super::super::workspace_path::UnresolvedPath;
use super::McpService;
use super::shared::{command_result, resolve_cwd, resolve_path};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData as McpError};
use rmcp::schemars::JsonSchema;
use rmcp::{tool, tool_router};
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct Input {
    /// File or directory path relative to the working directory.
    path: UnresolvedPath,
    /// Optional workspace-relative working directory.
    #[serde(default)]
    cwd: Option<UnresolvedPath>,
}

#[tool_router(router = git_restore_tool_router, vis = "pub(crate)")]
impl McpService {
    #[tool(
        description = "Unstages a file or directory with Git",
        annotations(
            title = "Git Restore",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn git_restore(
        &self,
        Parameters(input): Parameters<Input>,
    ) -> Result<CallToolResult, McpError> {
        let cwd = resolve_cwd(input.cwd, &self.workspace_root, &self.ignore)?;
        let path = resolve_path(input.path, &cwd, &self.workspace_root, &self.ignore)?;
        let path = path.relative_to(&cwd).to_str().ok_or_else(|| {
            McpError::invalid_params("git restore path must be valid UTF-8", None)
        })?;
        command_result(
            "git",
            &["restore", "--staged", path],
            "git restore",
            cwd.absolute(),
        )
        .await
    }
}
