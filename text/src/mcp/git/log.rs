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
    /// Maximum number of commits to show.
    #[serde(default = "default_limit")]
    limit: u32,
    /// Optional workspace-relative working directory.
    #[serde(default)]
    cwd: Option<UnresolvedPath>,
}

fn default_limit() -> u32 {
    20
}

#[tool_router(router = git_log_tool_router, vis = "pub(crate)")]
impl McpService {
    #[tool(
        description = "Shows recent Git commits",
        annotations(
            title = "Git Log",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn git_log(
        &self,
        Parameters(input): Parameters<Input>,
    ) -> Result<CallToolResult, McpError> {
        if input.limit == 0 {
            return Err(McpError::invalid_params(
                "git log limit must be greater than zero",
                None,
            ));
        }
        let cwd = resolve_cwd(input.cwd, &self.workspace_root, &self.ignore)?;
        let limit = input.limit.to_string();
        command_result(
            "git",
            &[
                "log",
                "--no-color",
                "--graph",
                "--pretty=format:%h • %s (%(decorate:prefix=,suffix= • )%cr)",
                "-n",
                &limit,
            ],
            "git log",
            cwd.absolute(),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::git::{git, service};
    use assert_fs::TempDir;
    use std::fs;

    #[tokio::test]
    async fn git_log_respects_limit() {
        let dir = TempDir::new().unwrap();
        git(dir.path(), &["init", "--quiet"]);
        for index in 0..3 {
            fs::write(dir.path().join("file.txt"), format!("{index}\n")).unwrap();
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
                    "commit",
                ],
            );
        }
        let result = service(dir.path())
            .git_log(Parameters(Input {
                limit: 2,
                cwd: None,
            }))
            .await
            .unwrap();
        assert_eq!(result.content[0].as_text().unwrap().text.lines().count(), 2);
    }
}
