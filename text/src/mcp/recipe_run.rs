//! Runs recipes from an explicitly configured `recipes.toml`, entirely
//! independent of `just_run`/`justfile` (see `just_run.rs`). Both tools
//! can be active at once — this one exists to be adopted gradually, not
//! to replace the other on day one. See ADR 0004.

use super::McpService;

use recipe::{Recipe, RecipeFile};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData as McpError};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::Deserialize;

use std::path::Path;

/// Same truncation policy as `just_run` — a runaway or chatty recipe
/// shouldn't flood the response.
const MAX_OUTPUT_BYTES: usize = 64 * 1024;

/// A recipe name, as sent by a tool call. Not checked for existence at
/// deserialization — the actual proof is the handler's lookup against
/// `self.recipes` (see ADR 0003's proof-of-existence rationale, which
/// applies here the same way, just without a type-level guarantee since
/// `recipe::RecipeName` is publicly constructable for the standalone
/// `recipe` CLI's sake).
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct RecipeName(String);

impl RecipeName {
    fn as_lib(&self) -> recipe::RecipeName {
        recipe::RecipeName::from(self.0.as_str())
    }
}

impl std::fmt::Display for RecipeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Loads `path`. Empty file if it doesn't exist or fails to parse — a
/// broken recipe file should degrade the feature, not crash the server.
/// Callers only invoke this after already checking `path.is_file()` (see
/// `McpServer::serve` and `--recipes-file`), so an empty result here
/// specifically means "failed to parse", not "not configured".
pub fn discover(path: &Path) -> RecipeFile {
    match RecipeFile::load(path) {
        Ok(file) => file,
        Err(e) => {
            tracing::warn!("failed to load {}: {e}", path.display());
            RecipeFile::default()
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RecipeRunInput {
    /// Name of a recipe to run.
    recipe: RecipeName,
    /// Positional arguments, in the recipe's declared parameter order.
    #[serde(default)]
    args: Vec<String>,
}

#[tool_router(router = recipe_run_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(description = "Run a recipe declared in this workspace's configured recipes.toml.")]
    fn recipe_run(
        &self,
        Parameters(input): Parameters<RecipeRunInput>,
    ) -> Result<CallToolResult, McpError> {
        let Some(found) = self.recipes.get(&input.recipe.as_lib()) else {
            return Err(McpError::invalid_params(
                format!("{}: no such recipe", input.recipe),
                None,
            ));
        };

        let output = found
            .execute(&input.args, &self.workspace_root)
            .map_err(|e| McpError::internal_error(format!("failed to run recipe: {e}"), None))?;

        let stdout = truncated(&output.stdout);
        let stderr = truncated(&output.stderr);
        let status = output
            .status
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "terminated by signal".to_owned());

        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "exit status: {status}\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
        ))]))
    }
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

/// Formats one recipe as a line for the tool description's recipe list
/// (see `McpService::list_tools`): `name arg1 arg2: description (args:
/// arg1 — help; arg2 — help)`, with the `description` and `(args: ...)`
/// segments only present when non-empty.
pub fn describe(name: &recipe::RecipeName, recipe: &Recipe) -> String {
    let params = recipe
        .args
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    let mut line = match params.as_str() {
        "" => name.to_string(),
        params => format!("{name} {params}"),
    };
    if !recipe.description.is_empty() {
        line.push_str(&format!(": {}", recipe.description));
    }
    if !recipe.args.is_empty() {
        let help = recipe
            .args
            .iter()
            .map(|(arg, info)| match info.help.as_str() {
                "" => arg.clone(),
                help => format!("{arg} — {help}"),
            })
            .collect::<Vec<_>>()
            .join("; ");
        line.push_str(&format!(" (args: {help})"));
    }
    format!("- {line}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discover_returns_empty_file_for_missing_path() {
        let root = assert_fs::TempDir::new().unwrap();
        assert!(discover(&root.path().join("recipes.toml")).is_empty());
    }

    /// Sanity check against this repository's own `recipes.toml` — not a
    /// generalizable test, but catches a TOML syntax mistake in that
    /// specific file immediately instead of only at server startup.
    #[test]
    fn repository_recipes_toml_parses() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../recipes.toml");
        let file = discover(&path);
        assert!(!file.is_empty(), "expected recipes.toml to declare recipes");
        for name in ["check", "lint", "test", "test-one", "git-commit"] {
            assert!(
                file.get(&recipe::RecipeName::from(name)).is_some(),
                "expected recipe {name:?} to be declared"
            );
        }
    }
}
