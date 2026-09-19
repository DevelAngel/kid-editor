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
    /// Branch name.
    name: String,
    /// Create the branch before switching to it.
    #[serde(default)]
    create: bool,
    /// Optional workspace-relative working directory.
    #[serde(default)]
    cwd: Option<UnresolvedPath>,
}

#[tool_router(router = git_switch_tool_router, vis = "pub(crate)")]
impl McpService {
    #[tool(
        description = "Switches to a Git branch, optionally creating it first",
        annotations(
            title = "Git Switch",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn git_switch(
        &self,
        Parameters(input): Parameters<Input>,
    ) -> Result<CallToolResult, McpError> {
        let cwd = resolve_cwd(input.cwd, &self.workspace_root, &self.ignore)?;
        let args = if input.create {
            vec!["switch", "--create", &input.name]
        } else {
            vec!["switch", &input.name]
        };
        command_result("git", &args, "git switch", cwd.absolute()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::git::{git, service};
    use assert_fs::TempDir;
    use std::process::Command;

    #[tokio::test]
    async fn git_switch_creates_and_switches_to_branch() {
        let dir = TempDir::new().unwrap();
        git(dir.path(), &["init", "--quiet"]);
        service(dir.path())
            .git_switch(Parameters(Input {
                name: "feature".to_owned(),
                create: true,
                cwd: None,
            }))
            .await
            .unwrap();
        let output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "feature");
    }

    #[tokio::test]
    async fn git_switch_switches_to_existing_branch() {
        let dir = TempDir::new().unwrap();
        git(dir.path(), &["init", "--quiet"]);
        git(dir.path(), &["switch", "--create", "feature"]);
        std::fs::write(dir.path().join("file.txt"), "content\n").unwrap();
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
        git(dir.path(), &["switch", "--create", "other"]);
        service(dir.path())
            .git_switch(Parameters(Input {
                name: "feature".to_owned(),
                create: false,
                cwd: None,
            }))
            .await
            .unwrap();
        let output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "feature");
    }
}
