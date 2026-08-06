//! Runs `just` recipes inside the workspace, without ever letting a
//! client choose *which* `justfile` or *which* directory that means.
//! [`RecipeName`] can only be built by discovery or by `serde`
//! deserializing a tool call — no public way to construct one out of an
//! arbitrary string, so the "does this recipe exist" check can't be
//! skipped. See ADR 0003.

use super::McpService;

use anyhow::Result;
use derive_more::{Deref, Display};
use indexmap::IndexMap;
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

/// Everything discovered about one recipe: its doc comment plus its
/// parameters, if any. A recipe with no parameters has empty `args`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RecipeInfo {
    description: RecipeDescription,
    args: RecipeArgs,
}

impl RecipeInfo {
    pub fn has_desc(&self) -> bool {
        !self.description.is_empty()
    }
    pub fn desc(&self) -> &str {
        &self.description
    }
    pub fn has_args(&self) -> bool {
        !self.args.is_empty()
    }
    pub fn arg_names(&self) -> impl Iterator<Item = &ArgName> {
        self.args.keys()
    }
    pub fn args(&self) -> impl Iterator<Item = (&ArgName, &ArgHelp)> {
        self.args.iter()
    }
}

/// The doc comment `just --show <recipe>` printed above a recipe's
/// signature, if any. Empty when the recipe has none — still listed,
/// just undescribed.
#[derive(Clone, Debug, Default, Deref, Display, Eq, PartialEq)]
pub struct RecipeDescription(String);

/// A recipe's parameters in signature order, name to help text.
pub type RecipeArgs = IndexMap<ArgName, ArgHelp>;

/// A recipe parameter as `just --usage <recipe>` names it under
/// `Arguments:` (e.g. `message`, `[args...]`). Order matters — positional
/// arguments must be reported in signature order — hence [`RecipeArgs`]
/// being an [`IndexMap`] rather than a [`BTreeMap`], which would
/// alphabetize them.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Deref, Display)]
pub struct ArgName(String);

/// The `[arg(..., help = "...")]` text for one parameter, if any.
/// Empty when the parameter has none.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deref, Display)]
pub struct ArgHelp(String);

impl RecipeName {
    /// Empty map if there is no `justfile`, or if `just` isn't available —
    /// either way, the caller ends up offering no recipes, same as
    /// offering no tool at all.
    ///
    /// `--summary` is the source of truth for which recipes exist. Each
    /// name is then described individually via `--show` (recipe's own
    /// source, immune to `--list`'s column-position-dependent format) and
    /// `--usage` (per-parameter help) — two extra calls per recipe, but
    /// negligible for a local `just` invocation.
    pub fn discover(workspace_root: &Path) -> BTreeMap<Self, RecipeInfo> {
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

        names
            .into_keys()
            .map(|name| {
                let description = run_just(&justfile_path, &["--show", name.as_str()])
                    .map(|stdout| parse_recipe_description(&stdout))
                    .unwrap_or_default();
                let args = run_just(&justfile_path, &["--usage", name.as_str()])
                    .map(|stdout| parse_recipe_usage(&stdout))
                    .unwrap_or_default();
                (name, RecipeInfo { description, args })
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

/// `just --show <recipe>` prints the recipe's own source: an optional
/// leading `# ` doc comment line, then optional attribute lines like
/// `[group('name')]`, then the signature and body. The doc comment is
/// always the first line and always starts with `#` — no column-position
/// parsing needed. Empty if the recipe has no doc comment.
fn parse_recipe_description(stdout: &[u8]) -> RecipeDescription {
    String::from_utf8_lossy(stdout)
        .lines()
        .next()
        .and_then(|line| line.strip_prefix('#'))
        .map(|desc| RecipeDescription(desc.trim().to_owned()))
        .unwrap_or_default()
}

/// `just --usage <recipe>` prints a `Usage: ...` line, then — only if
/// the recipe takes parameters — a blank line, `Arguments:`, and one
/// indented `<name> <help>` line per parameter (help empty if the
/// parameter has none). Recipes without parameters have no `Arguments:`
/// section at all, so this returns an empty map for them.
fn parse_recipe_usage(stdout: &[u8]) -> RecipeArgs {
    let text = String::from_utf8_lossy(stdout);
    text.lines()
        .skip_while(|line| line.trim() != "Arguments:")
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .map(|line| {
            let mut name_and_help = line.trim().splitn(2, char::is_whitespace);
            let name = name_and_help.next().unwrap_or_default().to_owned();
            let help = name_and_help.next().unwrap_or_default().trim().to_owned();
            (ArgName(name), ArgHelp(help))
        })
        .collect()
}

#[derive(Debug, Deserialize, JsonSchema)]
struct JustRunInput {
    /// Name of a `just` recipe to run.
    recipe: RecipeName,
    /// Optional arguments if the recipe has some.
    #[serde(default)]
    args: Option<Vec<String>>,
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
            .arg("--yes")
            .arg("--") //< prevents that MCP client injects just options
            .arg(input.recipe.as_str())
            .args(input.args.unwrap_or_default().iter())
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
    fn parses_description_from_leading_comment_line() {
        let stdout = b"# Run cargo check\ncheck:\n    cargo check\n";
        assert_eq!(
            parse_recipe_description(stdout),
            RecipeDescription("Run cargo check".to_owned())
        );
    }

    #[test]
    fn recipe_without_leading_comment_has_empty_description() {
        let stdout = b"lint:\n    cargo clippy\n";
        assert_eq!(
            parse_recipe_description(stdout),
            RecipeDescription::default()
        );
    }

    #[test]
    fn parses_description_when_group_attribute_follows() {
        let stdout = b"# Run all tests\n[group('test')]\ntest:\n    cargo test\n";
        assert_eq!(
            parse_recipe_description(stdout),
            RecipeDescription("Run all tests".to_owned())
        );
    }

    #[test]
    fn parses_args_from_usage_output() {
        let stdout =
            b"Usage: just git-commit message [args...]\n\nArguments:\n  message commit message\n  [args...] more arguments\n";
        let args = parse_recipe_usage(stdout);
        assert_eq!(
            args.into_iter().collect::<Vec<_>>(),
            vec![
                (
                    ArgName("message".to_owned()),
                    ArgHelp("commit message".to_owned())
                ),
                (
                    ArgName("[args...]".to_owned()),
                    ArgHelp("more arguments".to_owned())
                ),
            ]
        );
    }

    #[test]
    fn recipe_without_parameters_has_no_args() {
        let stdout = b"Usage: just check\n";
        assert!(parse_recipe_usage(stdout).is_empty());
    }

    #[test]
    fn arg_without_help_text_has_empty_help() {
        let stdout = b"Usage: just git-add args\n\nArguments:\n  args\n";
        let args = parse_recipe_usage(stdout);
        assert_eq!(
            args.get(&ArgName("args".to_owned())),
            Some(&ArgHelp::default())
        );
    }
}
