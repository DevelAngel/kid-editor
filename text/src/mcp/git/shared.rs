use super::super::workspace_path::{IgnorePattern, UnresolvedPath, WorkspacePath};

use rmcp::model::{CallToolResult, ContentBlock, ErrorData as McpError};
use serde_json::json;
use std::path::Path;
use std::process::{Command, Stdio};
use tokio::task;

#[derive(Debug)]
struct ProcessOutput {
    status: i32,
    stdout: String,
    stderr: String,
}

pub(crate) async fn command_result(
    program: &str,
    args: &[&str],
    operation: &str,
    cwd: &Path,
) -> Result<CallToolResult, McpError> {
    let output = run_process(program, cwd, args, operation).await?;
    if output.status == 0 {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            output.stdout,
        )]))
    } else {
        Err(McpError::internal_error(
            format!("{operation} failed: {}", output.stderr.trim()),
            Some(json!({
                "status": output.status,
                "stdout": output.stdout,
                "stderr": output.stderr,
            })),
        ))
    }
}

async fn run_process(
    program: &str,
    cwd: &Path,
    args: &[&str],
    operation: &str,
) -> Result<ProcessOutput, McpError> {
    let working_directory = cwd.to_path_buf();
    let program = program.to_owned();
    let args = args.iter().map(ToString::to_string).collect::<Vec<_>>();

    let output = task::spawn_blocking(move || {
        Command::new(program)
            .args(args)
            .current_dir(working_directory)
            .stdin(Stdio::null())
            .output()
    })
    .await
    .map_err(|err| {
        McpError::internal_error(
            format!("failed to run {operation}"),
            Some(json!({"reason": err.to_string()})),
        )
    })?
    .map_err(|err| {
        McpError::internal_error(
            format!("failed to execute {operation}"),
            Some(json!({"reason": err.to_string()})),
        )
    })?;

    Ok(ProcessOutput {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub(crate) fn resolve_cwd(
    cwd: Option<UnresolvedPath>,
    workspace_root: &Path,
    ignore: &[IgnorePattern],
) -> Result<WorkspacePath, McpError> {
    cwd.map_or_else(
        || Ok(WorkspacePath::root(workspace_root)),
        |cwd| cwd.resolve(workspace_root, ignore),
    )
}

pub(crate) fn resolve_path(
    path: UnresolvedPath,
    cwd: &WorkspacePath,
    workspace_root: &Path,
    ignore: &[IgnorePattern],
) -> Result<WorkspacePath, McpError> {
    path.resolve_from(cwd, workspace_root, ignore)
}
