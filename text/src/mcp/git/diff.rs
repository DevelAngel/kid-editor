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
    /// Optional workspace-relative working directory.
    #[serde(default)]
    cwd: Option<UnresolvedPath>,
}

#[tool_router(router = git_diff_tool_router, vis = "pub(crate)")]
impl McpService {
    #[tool(
        description = "Shows the current Git diff",
        annotations(
            title = "Git Diff",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn git_diff(
        &self,
        Parameters(input): Parameters<Input>,
    ) -> Result<CallToolResult, McpError> {
        let cwd = resolve_cwd(input.cwd, &self.workspace_root, &self.ignore)?;
        command_result("git", &["diff"], "git diff", cwd.absolute()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::git::{git, service};
    use assert_fs::TempDir;
    use std::fs;

    #[tokio::test]
    async fn git_diff_uses_workspace_root() {
        let dir = TempDir::new().unwrap();
        git(dir.path(), &["init", "--quiet"]);
        fs::write(dir.path().join("file.txt"), "content\n").unwrap();
        git(dir.path(), &["add", "file.txt"]);
        git(
            dir.path(),
            &[
                "-c",
                "user.name=Test User",
                "-c",
                "user.email=test@example.com",
                "commit",
                "--quiet",
                "-m",
                "initial",
            ],
        );
        fs::write(dir.path().join("file.txt"), "changed\n").unwrap();
        let result = service(dir.path())
            .git_diff(Parameters(Input { cwd: None }))
            .await
            .unwrap();
        assert!(
            result.content[0]
                .as_text()
                .unwrap()
                .text
                .contains("changed")
        );
    }
}
