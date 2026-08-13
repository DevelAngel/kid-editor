//! Exposes one MCP tool per recipe from an explicitly configured
//! `recipes.toml`, instead of a single generic `recipe_run(name, args)`.
//! Per-recipe tools let an MCP client grant or deny each recipe
//! individually — a client that only trusts `check` and `test` can
//! allow those two and leave everything else, including `git-commit`,
//! unapproved. A single generic tool can't express that: approving it
//! approves every recipe behind it at once. See ADR 0005.
//!
//! This can't go through `#[tool_router]` like every other tool in this
//! module tree — that macro builds its router from static `#[tool]`
//! annotations at compile time, and the set of recipes (hence the set of
//! tools) is only known once `recipes.toml` is read at startup. Tools
//! are therefore built and dispatched by hand here, called directly from
//! `McpService::list_tools`/`call_tool` rather than through
//! `tool_router`.

use recipe::{Recipe, RecipeFile, RecipeName};

use rmcp::model::{
    CallToolResult, ContentBlock, ErrorData as McpError, JsonObject, Tool, ToolAnnotations,
};
use serde_json::{Map, Value, json};

use std::path::Path;
use std::sync::Arc;

/// The prefix every generated tool name gets, so a recipe can never
/// collide with one of this server's fixed tool names (`view`, `create`,
/// `just_run`, ...) — even a recipe literally named `view` becomes
/// `recipe_view`, not `view`.
const TOOL_PREFIX: &str = "recipe_";

/// Same truncation policy as every other command-running tool here — a
/// runaway or chatty recipe shouldn't flood the response.
const MAX_OUTPUT_BYTES: usize = 64 * 1024;

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

/// MCP tool names allow `[A-Za-z0-9_]`; recipe names use `-` (see
/// `recipes.toml`). Deterministic and mechanical, not meant to be
/// reversed — `find` below matches by recomputing this from each known
/// recipe rather than parsing a tool name back into one.
fn tool_name(name: &RecipeName) -> String {
    format!("{TOOL_PREFIX}{}", name.as_str().replace('-', "_"))
}

/// One [`Tool`] per recipe in `recipes`, each with a hand-built
/// object schema: one required string property per declared parameter,
/// in the recipe's own order, described by that parameter's `help`.
pub fn tools(recipes: &RecipeFile) -> Vec<Tool> {
    recipes
        .iter()
        .map(|(name, recipe)| {
            let properties: Map<String, Value> = recipe
                .args
                .iter()
                .map(|(arg, info)| {
                    let schema = match info.help.as_str() {
                        "" => json!({"type": "string"}),
                        help => json!({"type": "string", "description": help}),
                    };
                    (arg.clone(), schema)
                })
                .collect();
            let required: Vec<Value> = recipe
                .args
                .keys()
                .map(|arg| Value::String(arg.clone()))
                .collect();
            let schema = json!({
                "type": "object",
                "properties": properties,
                "required": required,
            });
            let Value::Object(schema) = schema else {
                unreachable!("json!({{...}}) with object literal always produces Value::Object");
            };
            Tool::new(tool_name(name), description(recipe), Arc::new(schema))
                .with_annotations(annotations(recipe))
        })
        .collect()
}

/// Maps a recipe's own optional annotation fields onto rmcp's
/// `ToolAnnotations` — every field stays `None` (rmcp's own default)
/// unless `recipes.toml` sets it explicitly. See `RecipeAnnotations`.
/// `ToolAnnotations` is `#[non_exhaustive]`, hence the `..Default::default()`.
fn annotations(recipe: &Recipe) -> ToolAnnotations {
    let mut result = ToolAnnotations::default();
    result.title = recipe.annotations.title.clone();
    result.read_only_hint = recipe.annotations.read_only;
    result.destructive_hint = recipe.annotations.destructive;
    result.idempotent_hint = recipe.annotations.idempotent;
    result.open_world_hint = recipe.annotations.open_world;
    result
}

/// One line, for the tool's own `description` — what it runs and what
/// it takes, since the recipe's name alone (now the tool's name) no
/// longer needs restating the way the old `recipe_run` list did.
fn description(recipe: &Recipe) -> String {
    let mut line = if recipe.description.is_empty() {
        "Run this recipe.".to_owned()
    } else {
        recipe.description.clone()
    };
    if !recipe.args.is_empty() {
        let params = recipe
            .args
            .iter()
            .map(|(arg, info)| match info.help.as_str() {
                "" => arg.clone(),
                help => format!("{arg} ({help})"),
            })
            .collect::<Vec<_>>()
            .join(", ");
        line.push_str(&format!(" Parameters: {params}."));
    }
    line
}

/// Runs the recipe behind `tool_name`, if any is currently offered.
/// `Ok(None)` (not an error) means `tool_name` isn't one of ours — the
/// caller (`McpService::call_tool`) falls through to `tool_router` for
/// every other tool, so an unrecognized name here isn't necessarily a
/// dead end.
pub fn call(
    recipes: &RecipeFile,
    tool_name_requested: &str,
    arguments: Option<&JsonObject>,
    workspace_root: &Path,
) -> Option<Result<CallToolResult, McpError>> {
    let (_, recipe) = recipes
        .iter()
        .find(|(name, _)| tool_name(name) == tool_name_requested)?;

    Some(run(recipe, arguments, workspace_root))
}

fn run(
    recipe: &Recipe,
    arguments: Option<&JsonObject>,
    workspace_root: &Path,
) -> Result<CallToolResult, McpError> {
    let mut provided = Vec::with_capacity(recipe.args.len());
    for arg in recipe.args.keys() {
        let value = arguments
            .and_then(|args| args.get(arg))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                McpError::invalid_params(format!("missing required argument `{arg}`"), None)
            })?;
        provided.push(value.to_owned());
    }

    let output = recipe
        .execute(&provided, workspace_root)
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
                file.get(&RecipeName::from(name)).is_some(),
                "expected recipe {name:?} to be declared"
            );
        }
    }

    #[test]
    fn tool_name_replaces_hyphens_with_underscores() {
        assert_eq!(
            tool_name(&RecipeName::from("git-commit")),
            "recipe_git_commit"
        );
    }

    #[test]
    fn tools_prefixes_and_carries_one_property_per_arg() {
        let file =
            discover(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../recipes.toml"));
        let generated = tools(&file);
        let commit = generated
            .iter()
            .find(|t| t.name == "recipe_git_commit")
            .expect("expected a recipe_git_commit tool");
        assert_eq!(
            commit.input_schema.get("required"),
            Some(&Value::Array(vec![Value::String("message".to_owned())]))
        );
    }

    #[test]
    fn call_runs_matching_recipe_and_none_for_unknown_name() {
        let toml_text = "[recipe.check]\nrun = [\"echo\", \"hi\"]\n";
        let file: RecipeFile = toml::from_str(toml_text).unwrap();
        let root = assert_fs::TempDir::new().unwrap();

        assert!(call(&file, "recipe_check", None, root.path()).is_some());
        assert!(call(&file, "recipe_missing", None, root.path()).is_none());
    }
}
