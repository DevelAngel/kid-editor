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

#[tool_router(router = git_rm_tool_router, vis = "pub(crate)")]
impl McpService {
    #[tool(
        description = "Removes a tracked file with Git",
        annotations(
            title = "Git Remove",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn git_rm(
        &self,
        Parameters(input): Parameters<Input>,
    ) -> Result<CallToolResult, McpError> {
        let cwd = resolve_cwd(input.cwd, &self.workspace_root, &self.ignore)?;
        let path = resolve_path(input.path, &cwd, &self.workspace_root, &self.ignore)?;
        let path = path
            .relative_to(&cwd)
            .to_str()
            .ok_or_else(|| McpError::invalid_params("git rm path must be valid UTF-8", None))?;
        command_result("git", &["rm", path], "git rm", cwd.absolute()).await
    }
}
