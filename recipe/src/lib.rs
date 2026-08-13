//! A minimal, shell-free command-runner: recipes are declared in a TOML
//! file as a name, a description, an ordered list of named parameters,
//! and the literal argv to execute. No recipe can invoke another, import
//! another file, or run through a shell — see the project's ADR 0004 for
//! why those omissions are deliberate.

pub mod cli;

use indexmap::IndexMap;
use serde::Deserialize;

use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};
use std::path::Path;
use std::process::{Command, Output};

/// A recipe's name, as declared by the `[recipe.<name>]` table key.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[serde(transparent)]
pub struct RecipeName(String);

impl RecipeName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for RecipeName {
    fn from(name: &str) -> Self {
        Self(name.to_owned())
    }
}

impl Display for RecipeName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One named parameter a recipe accepts, substituted into `run` at the
/// placeholder `{name}`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct RecipeArg {
    #[serde(default)]
    pub help: String,
}

/// MCP `ToolAnnotations` hints for a recipe's generated tool. Every
/// field is `Option<bool>`/`Option<String>` and defaults to `None` when
/// absent from `recipes.toml` — an unset field leaves rmcp's own
/// default in place rather than forcing every existing recipe to
/// declare one. See `text/src/mcp/recipe_run.rs`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct RecipeAnnotations {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub read_only: Option<bool>,
    #[serde(default)]
    pub destructive: Option<bool>,
    #[serde(default)]
    pub idempotent: Option<bool>,
    #[serde(default)]
    pub open_world: Option<bool>,
}

/// A single recipe: what it does, what it takes, and the literal argv to
/// run. `args` is an [`IndexMap`] because parameter order is meaningful
/// for positional CLI usage (`recipe run test-one my_test`), not just
/// for display.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct Recipe {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub args: IndexMap<String, RecipeArg>,
    pub run: Vec<String>,
    #[serde(default)]
    pub annotations: RecipeAnnotations,
}

impl Recipe {
    /// Builds and runs this recipe's `run` argv directly — no shell — in
    /// `cwd`, substituting `{name}` placeholders in each argv element
    /// with the corresponding positional value from `provided`.
    ///
    /// `provided` must have exactly as many entries as `self.args`;
    /// anything else is a caller error, not a partial match.
    pub fn execute(&self, provided: &[String], cwd: &Path) -> Result<Output, ExecError> {
        if provided.len() != self.args.len() {
            return Err(ExecError::ArgCountMismatch {
                expected: self.args.len(),
                got: provided.len(),
            });
        }

        let substitutions: Vec<(&str, &str)> = self
            .args
            .keys()
            .map(String::as_str)
            .zip(provided.iter().map(String::as_str))
            .collect();

        let argv: Vec<String> = self
            .run
            .iter()
            .map(|part| substitute(part, &substitutions))
            .collect();

        let Some((program, rest)) = argv.split_first() else {
            return Err(ExecError::EmptyRun);
        };

        Command::new(program)
            .args(rest)
            .current_dir(cwd)
            .output()
            .map_err(|source| ExecError::Spawn {
                program: program.clone(),
                source,
            })
    }
}

/// Replaces every `{name}` occurrence in `part` with its substitution.
/// A `part` with no placeholder is returned unchanged.
fn substitute(part: &str, substitutions: &[(&str, &str)]) -> String {
    let mut out = part.to_owned();
    for (name, value) in substitutions {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

#[derive(Debug)]
pub enum ExecError {
    ArgCountMismatch {
        expected: usize,
        got: usize,
    },
    EmptyRun,
    Spawn {
        program: String,
        source: std::io::Error,
    },
}

impl Display for ExecError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArgCountMismatch { expected, got } => {
                write!(f, "expected {expected} argument(s), got {got}")
            }
            Self::EmptyRun => write!(f, "recipe's `run` is empty"),
            Self::Spawn { program, source } => write!(f, "failed to run `{program}`: {source}"),
        }
    }
}

impl std::error::Error for ExecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// All recipes declared in one `recipes.toml`, keyed by name. TOML's
/// `[recipe.<name>]` syntax nests every recipe one level under the key
/// `recipe`, so this can't be `#[serde(transparent)]` over the map
/// directly — the `recipe` field name below is what makes
/// `[recipe.check]` land in the map rather than being rejected as an
/// unknown top-level key.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct RecipeFile {
    #[serde(default)]
    recipe: BTreeMap<RecipeName, Recipe>,
}

impl RecipeFile {
    /// Empty file if `path` doesn't exist — same convention as the
    /// `just`-based discovery this replaces: no recipe file means no
    /// recipes, not an error.
    pub fn load(path: &Path) -> Result<Self, LoadError> {
        if !path.is_file() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path).map_err(LoadError::Read)?;
        toml::from_str(&text).map_err(LoadError::Parse)
    }

    pub fn get(&self, name: &RecipeName) -> Option<&Recipe> {
        self.recipe.get(name)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&RecipeName, &Recipe)> {
        self.recipe.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.recipe.is_empty()
    }
}

#[derive(Debug)]
pub enum LoadError {
    Read(std::io::Error),
    Parse(toml::de::Error),
}

impl Display for LoadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(e) => write!(f, "failed to read recipe file: {e}"),
            Self::Parse(e) => write!(f, "failed to parse recipe file: {e}"),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(e) => Some(e),
            Self::Parse(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    fn recipe_file(toml: &str) -> RecipeFile {
        toml::from_str(toml).unwrap()
    }

    #[test]
    fn parses_recipe_without_args() {
        let file = recipe_file(indoc! {r#"
            [recipe.check]
            description = "Full workspace build check"
            run = ["cargo", "check", "--all-targets"]
        "#});
        let recipe = file.get(&RecipeName("check".to_owned())).unwrap();
        assert_eq!(recipe.description, "Full workspace build check");
        assert!(recipe.args.is_empty());
        assert_eq!(recipe.run, vec!["cargo", "check", "--all-targets"]);
    }

    #[test]
    fn parses_recipe_with_named_arg() {
        let file = recipe_file(indoc! {r#"
            [recipe.test-one]
            description = "Run a single test by name"
            run = ["cargo", "test", "--", "{name}"]

            [recipe.test-one.args.name]
            help = "test name to run"
        "#});
        let recipe = file.get(&RecipeName("test-one".to_owned())).unwrap();
        assert_eq!(recipe.args.keys().collect::<Vec<_>>(), vec!["name"]);
        assert_eq!(recipe.args["name"].help, "test name to run");
    }

    #[test]
    fn load_returns_empty_file_when_missing() {
        let root = assert_fs::TempDir::new().unwrap();
        let file = RecipeFile::load(&root.path().join("recipes.toml")).unwrap();
        assert!(file.is_empty());
    }

    #[test]
    fn substitutes_named_placeholder() {
        let recipe = Recipe {
            description: String::new(),
            args: IndexMap::from([(
                "name".to_owned(),
                RecipeArg {
                    help: String::new(),
                },
            )]),
            run: vec!["echo".to_owned(), "{name}".to_owned()],
            annotations: RecipeAnnotations::default(),
        };
        let root = assert_fs::TempDir::new().unwrap();
        let output = recipe.execute(&["hello".to_owned()], root.path()).unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
    }

    #[test]
    fn substitutes_same_placeholder_used_twice() {
        let recipe = Recipe {
            description: String::new(),
            args: IndexMap::from([(
                "msg".to_owned(),
                RecipeArg {
                    help: String::new(),
                },
            )]),
            run: vec!["echo".to_owned(), "{msg}-{msg}".to_owned()],
            annotations: RecipeAnnotations::default(),
        };
        let root = assert_fs::TempDir::new().unwrap();
        let output = recipe.execute(&["a".to_owned()], root.path()).unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "a-a");
    }

    #[test]
    fn argv_element_without_placeholder_is_unchanged() {
        let recipe = Recipe {
            description: String::new(),
            args: IndexMap::new(),
            run: vec!["echo".to_owned(), "fixed".to_owned()],
            annotations: RecipeAnnotations::default(),
        };
        let root = assert_fs::TempDir::new().unwrap();
        let output = recipe.execute(&[], root.path()).unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "fixed");
    }

    #[test]
    fn rejects_wrong_argument_count() {
        let recipe = Recipe {
            description: String::new(),
            args: IndexMap::from([(
                "name".to_owned(),
                RecipeArg {
                    help: String::new(),
                },
            )]),
            run: vec!["echo".to_owned(), "{name}".to_owned()],
            annotations: RecipeAnnotations::default(),
        };
        let root = assert_fs::TempDir::new().unwrap();
        assert!(matches!(
            recipe.execute(&[], root.path()),
            Err(ExecError::ArgCountMismatch {
                expected: 1,
                got: 0
            })
        ));
    }

    #[test]
    fn rejects_empty_run() {
        let recipe = Recipe {
            description: String::new(),
            args: IndexMap::new(),
            run: vec![],
            annotations: RecipeAnnotations::default(),
        };
        let root = assert_fs::TempDir::new().unwrap();
        assert!(matches!(
            recipe.execute(&[], root.path()),
            Err(ExecError::EmptyRun)
        ));
    }
}
