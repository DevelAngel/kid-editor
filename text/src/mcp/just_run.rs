//! Runs `just` recipes inside the workspace, without ever letting a
//! client choose *which* `justfile` or *which* directory that means.
//! [`RecipeName`] can only be built by discovery or by `serde`
//! deserializing a tool call — no public way to construct one out of an
//! arbitrary string, so the "does this recipe exist" check can't be
//! skipped. See ADR 0003.

use super::McpService;

use anyhow::Result;
use derive_more::{Deref, Display};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData as McpError};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::Deserialize;

use std::collections::HashSet;
use std::path::Path;
use std::process::{Command, Output};

/// The output of a `just` recipe is truncated to this many bytes each for
/// stdout and stderr, so a runaway or chatty recipe can't flood the
/// response.
const MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// A recipe `just --summary` reported for this workspace's `justfile`.
/// Deliberately not `String`: holding one *is* the proof it exists, the
/// same way `WorkspacePath` is the proof a path was checked.
#[derive(Clone, Debug, Deref, Display, Eq, PartialEq, Hash, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct RecipeName(String);

impl RecipeName {
    /// Empty set if there is no `justfile`, or if `just` isn't available —
    /// either way, the caller ends up offering no recipes, same as
    /// offering no tool at all.
    pub fn discover(workspace_root: &Path) -> HashSet<Self> {
        let justfile_path = workspace_root.join("justfile");
        if !justfile_path.is_file() {
            return HashSet::new();
        }

        let output = Command::new("just")
            .arg("--summary")
            .arg("--unsorted")
            .arg("--no-aliases")
            .arg("--justfile")
            .arg(&justfile_path)
            .output();

        match output {
            Ok(output) if output.status.success() => parse_recipe_list(&output.stdout),
            Ok(output) => {
                tracing::debug!(
                    "`just --summary` failed, just_run tool disabled: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                HashSet::new()
            }
            Err(e) => {
                tracing::debug!("could not run `just --summary`, just_run tool disabled: {e}");
                HashSet::new()
            }
        }
    }
}

/// `just --summary --unsorted --no-aliases` prints recipe names separated
/// by plain whitespace on one line — no per-line parsing needed.
fn parse_recipe_list(stdout: &[u8]) -> HashSet<RecipeName> {
    String::from_utf8_lossy(stdout)
        .split_ascii_whitespace()
        .map(|name| RecipeName(name.to_owned()))
        .collect()
}

#[derive(Debug, Deserialize, JsonSchema)]
struct JustRunInput {
    /// Name of a `just` recipe to run.
    recipe: RecipeName,
}

#[tool_router(router = just_run_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(
        description = "Run a `just` recipe from the workspace's justfile (e.g. `check`, `test`, `lint`). Only available when the workspace has a justfile; only recipes it defines can be run."
    )]
    fn just_run(
        &self,
        Parameters(input): Parameters<JustRunInput>,
    ) -> Result<CallToolResult, McpError> {
        if !self.just_recipes.contains(&input.recipe) {
            return Err(McpError::invalid_params(
                format!("{}: no such just recipe", input.recipe),
                None,
            ));
        }

        let output = Command::new("just")
            .arg("--justfile")
            .arg(self.workspace_root.join("justfile"))
            .arg("--working-directory")
            .arg(&self.workspace_root)
            .arg(input.recipe.as_str())
            .output()
            .map_err(|e| McpError::internal_error(format!("failed to run `just`: {e}"), None))?;

        Ok(CallToolResult::success(vec![ContentBlock::text(
            render_output(&output),
        )]))
    }
}

fn render_output(output: &Output) -> String {
    let stdout = truncated(&output.stdout);
    let stderr = truncated(&output.stderr);
    let status = output
        .status
        .code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "terminated by signal".to_owned());

    format!("exit status: {status}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}")
}

fn truncated(bytes: &[u8]) -> String {
    if bytes.len() <= MAX_OUTPUT_BYTES {
        String::from_utf8_lossy(bytes).into_owned()
    } else {
        format!(
            "{}\n... truncated ({} bytes total) ...",
            String::from_utf8_lossy(&bytes[..MAX_OUTPUT_BYTES]),
            bytes.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_recipe_names_from_just_list_output() {
        let stdout = b"check test lint\n";
        let recipes = parse_recipe_list(stdout);
        assert_eq!(
            recipes,
            HashSet::from([
                RecipeName("check".to_owned()),
                RecipeName("test".to_owned()),
                RecipeName("lint".to_owned()),
            ])
        );
    }

    #[test]
    fn ignores_blank_and_unindented_lines() {
        let stdout = b"check\n";
        let recipes = parse_recipe_list(stdout);
        assert_eq!(recipes, HashSet::from([RecipeName("check".to_owned())]));
    }

    #[test]
    fn discover_returns_empty_set_without_justfile() {
        let root = assert_fs::TempDir::new().unwrap();
        assert!(RecipeName::discover(root.path()).is_empty());
    }
}
