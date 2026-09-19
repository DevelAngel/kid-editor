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
    /// Current path relative to the working directory.
    source: UnresolvedPath,
    /// Destination path relative to the working directory.
    destination: UnresolvedPath,
    /// Optional workspace-relative working directory.
    #[serde(default)]
    cwd: Option<UnresolvedPath>,
}

#[tool_router(router = git_mv_tool_router, vis = "pub(crate)")]
impl McpService {
    #[tool(
        description = "Renames or moves a tracked file with Git",
        annotations(
            title = "Git Move",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn git_mv(
        &self,
        Parameters(input): Parameters<Input>,
    ) -> Result<CallToolResult, McpError> {
        let cwd = resolve_cwd(input.cwd, &self.workspace_root, &self.ignore)?;
        let source = resolve_path(input.source, &cwd, &self.workspace_root, &self.ignore)?;
        let destination =
            resolve_path(input.destination, &cwd, &self.workspace_root, &self.ignore)?;
        let source = source
            .relative_to(&cwd)
            .to_str()
            .ok_or_else(|| McpError::invalid_params("git mv source must be valid UTF-8", None))?;
        let destination = destination.relative_to(&cwd).to_str().ok_or_else(|| {
            McpError::invalid_params("git mv destination must be valid UTF-8", None)
        })?;
        command_result(
            "git",
            &["mv", source, destination],
            "git mv",
            cwd.absolute(),
        )
        .await
    }
}
