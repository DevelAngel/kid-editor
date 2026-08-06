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

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output};

/// The output of a `just` recipe is truncated to this many bytes each for
/// stdout and stderr, so a runaway or chatty recipe can't flood the
/// response.
const MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// A recipe `just --summary` reported for this workspace's `justfile`.
/// Deliberately not `String`: holding one *is* the proof it exists, the
/// same way `WorkspacePath` is the proof a path was checked.
#[derive(
    Clone, Debug, Deref, Display, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct RecipeName(String);

/// The doc comment `just --list` printed above a recipe, if any. Empty
/// when the recipe has none — still listed, just undescribed.
#[derive(Clone, Debug, Default, Deref, Display, Eq, PartialEq)]
pub struct RecipeDescription(String);

impl RecipeName {
    /// Empty map if there is no `justfile`, or if `just` isn't available —
    /// either way, the caller ends up offering no recipes, same as
    /// offering no tool at all.
    ///
    /// `--summary` is the source of truth for which recipes exist;
    /// `--list` only annotates that set with descriptions, so a recipe
    /// `--list` doesn't describe still ends up in the map, just with an
    /// empty [`RecipeDescription`].
    pub fn discover(workspace_root: &Path) -> BTreeMap<Self, RecipeDescription> {
        let justfile_path = workspace_root.join("justfile");
        if !justfile_path.is_file() {
            return BTreeMap::new();
        }

        let names = run_just(&justfile_path, &["--summary", "--unsorted", "--no-aliases"])
            .map(|stdout| parse_recipe_list(&stdout))
            .unwrap_or_default();
        if names.is_empty() {
            return BTreeMap::new();
        }

        let mut descriptions = run_just(&justfile_path, &["--list", "--unsorted", "--no-aliases"])
            .map(|stdout| parse_recipe_descriptions(&stdout))
            .unwrap_or_default();

        names
            .into_keys()
            .map(|name| {
                let description = descriptions.remove(&name).unwrap_or_default();
                (name, description)
            })
            .collect()
    }
}

/// Runs `just` with the given args against `justfile_path`, returning
/// stdout on success. Failures are logged and swallowed — a broken `just`
/// invocation should degrade the feature, not crash the server.
fn run_just(justfile_path: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("just")
        .args(args)
        .arg("--justfile")
        .arg(justfile_path)
        .output();

    match output {
        Ok(output) if output.status.success() => Some(output.stdout),
        Ok(output) => {
            tracing::debug!(
                "`just {args:?}` failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            None
        }
        Err(e) => {
            tracing::debug!("could not run `just {args:?}`: {e}");
            None
        }
    }
}

/// `just --summary --unsorted --no-aliases` prints recipe names separated
/// by plain whitespace on one line — no per-line parsing needed.
fn parse_recipe_list(stdout: &[u8]) -> BTreeMap<RecipeName, ()> {
    String::from_utf8_lossy(stdout)
        .split_ascii_whitespace()
        .map(|name| (RecipeName(name.to_owned()), ()))
        .collect()
}

/// `just --list --unsorted --no-aliases` prints one indented line per
/// recipe: the name (plus any parameters), then optionally `# ` followed
/// by its doc comment. The header line ("Available recipes:") and any
/// line without a recognizable leading name are skipped.
fn parse_recipe_descriptions(stdout: &[u8]) -> BTreeMap<RecipeName, RecipeDescription> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| {
            let mut name_and_rest = line.trim().splitn(2, char::is_whitespace);
            let name = name_and_rest.next()?.to_owned();
            let description = line
                .split_once('#')
                .map(|(_, desc)| desc.trim().to_owned())
                .unwrap_or_default();
            Some((RecipeName(name), RecipeDescription(description)))
        })
        .collect()
}

#[derive(Debug, Deserialize, JsonSchema)]
struct JustRunInput {
    /// Name of a `just` recipe to run.
    recipe: RecipeName,
}

#[tool_router(router = just_run_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(description = "Run a `just` recipe.")]
    fn just_run(
        &self,
        Parameters(input): Parameters<JustRunInput>,
    ) -> Result<CallToolResult, McpError> {
        if !self.just_recipes.contains_key(&input.recipe) {
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
            .arg("--one")
            .arg("--") //< prevents that MCP client injects just options
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
            recipes.into_keys().collect::<Vec<_>>(),
            vec![
                RecipeName("check".to_owned()),
                RecipeName("lint".to_owned()),
                RecipeName("test".to_owned()),
            ]
        );
    }

    #[test]
    fn ignores_blank_and_unindented_lines() {
        let stdout = b"check\n";
        let recipes = parse_recipe_list(stdout);
        assert_eq!(
            recipes.into_keys().collect::<Vec<_>>(),
            vec![RecipeName("check".to_owned())]
        );
    }

    #[test]
    fn discover_returns_empty_set_without_justfile() {
        let root = assert_fs::TempDir::new().unwrap();
        assert!(RecipeName::discover(root.path()).is_empty());
    }

    #[test]
    fn parses_description_after_hash() {
        let stdout = b"Available recipes:\n    check   # Run cargo check\n";
        let descriptions = parse_recipe_descriptions(stdout);
        assert_eq!(
            descriptions.get(&RecipeName("check".to_owned())),
            Some(&RecipeDescription("Run cargo check".to_owned()))
        );
    }

    #[test]
    fn recipe_without_hash_has_empty_description() {
        let stdout = b"Available recipes:\n    lint\n";
        let descriptions = parse_recipe_descriptions(stdout);
        assert_eq!(
            descriptions.get(&RecipeName("lint".to_owned())),
            Some(&RecipeDescription::default())
        );
    }

    #[test]
    fn strips_parameters_from_recipe_name_before_hash() {
        let stdout = b"    test *ARGS   # Run tests\n";
        let descriptions = parse_recipe_descriptions(stdout);
        assert_eq!(
            descriptions.get(&RecipeName("test".to_owned())),
            Some(&RecipeDescription("Run tests".to_owned()))
        );
    }
}
