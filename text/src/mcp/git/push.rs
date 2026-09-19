use super::super::workspace_path::UnresolvedPath;
use super::McpService;
use super::shared::{command_result, resolve_cwd};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ErrorData as McpError};
use rmcp::schemars::JsonSchema;
use rmcp::{tool, tool_router};
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct Input {
    /// Use force-with-lease instead of a normal push.
    #[serde(default)]
    force: bool,
    /// Optional workspace-relative working directory.
    #[serde(default)]
    cwd: Option<UnresolvedPath>,
}

#[tool_router(router = git_push_tool_router, vis = "pub(crate)")]
impl McpService {
    #[tool(
        description = "Pushes the current branch to origin, optionally using force-with-lease",
        annotations(
            title = "Git Push",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn git_push(
        &self,
        Parameters(input): Parameters<Input>,
    ) -> Result<CallToolResult, McpError> {
        let cwd = resolve_cwd(input.cwd, &self.workspace_root, &self.ignore)?;
        let args = if input.force {
            ["push", "--force-with-lease", "origin", "HEAD"]
        } else {
            ["push", "--set-upstream", "origin", "HEAD"]
        };
        command_result("git", &args, "git push", cwd.absolute()).await
    }
}
