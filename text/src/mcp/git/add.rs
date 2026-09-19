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

#[tool_router(router = git_add_tool_router, vis = "pub(crate)")]
impl McpService {
    #[tool(
        description = "Stages a file or directory with Git",
        annotations(
            title = "Git Add",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn git_add(
        &self,
        Parameters(input): Parameters<Input>,
    ) -> Result<CallToolResult, McpError> {
        let cwd = resolve_cwd(input.cwd, &self.workspace_root, &self.ignore)?;
        let path = resolve_path(input.path, &cwd, &self.workspace_root, &self.ignore)?;
        let path = path.relative_to(&cwd);
        let path = if path.as_os_str().is_empty() {
            "."
        } else {
            path.to_str()
                .ok_or_else(|| McpError::invalid_params("git add path must be valid UTF-8", None))?
        };
        command_result("git", &["add", path], "git add", cwd.absolute()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::git::{git, service};
    use assert_fs::TempDir;
    use std::assert_matches;
    use std::fs;

    #[tokio::test]
    async fn git_add_rejects_paths_outside_workspace() {
        let dir = TempDir::new().unwrap();
        let result = service(dir.path())
            .git_add(Parameters(Input {
                path: UnresolvedPath::new("../outside"),
                cwd: None,
            }))
            .await;
        assert_matches!(
            result,
            Err(McpError {
                code: rmcp::model::ErrorCode::INVALID_PARAMS,
                ..
            })
        );
    }

    #[tokio::test]
    async fn git_add_resolves_path_relative_to_cwd() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path().join("repo");
        fs::create_dir(&repo).unwrap();
        git(&repo, &["init", "--quiet"]);
        fs::write(repo.join("file.txt"), "content\n").unwrap();
        service(dir.path())
            .git_add(Parameters(Input {
                path: UnresolvedPath::new("file.txt"),
                cwd: Some(UnresolvedPath::new("repo")),
            }))
            .await
            .unwrap();
        let output = std::process::Command::new("git")
            .args(["status", "--short"])
            .current_dir(&repo)
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&output.stdout).contains("A  file.txt"));
    }
}
