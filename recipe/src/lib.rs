//! A minimal, shell-free command-runner: recipes are declared in a TOML
//! file as a name, a description, an ordered list of named parameters,
//! and the literal argv to execute. No recipe can invoke another, import
//! another file, or run through a shell — see the project's ADR 0004 for
//! why those omissions are deliberate.

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

/// Whether and at what severity a parameter's value is logged by
/// [`Recipe::log_lines`]. Independent of [`ArgKind`], which only affects
/// formatting, not visibility.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ArgLogLevel {
    #[default]
    Hidden,
    Debug,
    Info,
}

/// Formatting hint for a parameter's value when logged. `Value` (the
/// default) renders as `name=value` on one line. `Path` renders the same
/// way, plus a best-effort workspace-membership check (see
/// `log_lines`) — informational only, not an access decision. `Text`
/// renders as an indented multi-line block instead, so embedded newlines
/// show up as real line breaks rather than an escaped `\n`.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ArgKind {
    #[default]
    Value,
    Path,
    Text,
}

/// One named parameter a recipe accepts, substituted into `run` at the
/// placeholder `{name}`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct RecipeArg {
    #[serde(default)]
    pub help: String,
    #[serde(default)]
    pub level: ArgLogLevel,
    #[serde(default, rename = "type")]
    pub kind: ArgKind,
}

/// MCP `ToolAnnotations` hints for a recipe's generated tool — unset
/// fields fall through to rmcp's own default.
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
/// for `execute`'s positional `provided` slice, not just for display —
/// the CLI itself maps its named `--<param>` flags onto that order.
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

    /// Builds one log line per severity actually used among this
    /// recipe's `level`-tagged args, in this recipe's own parameter
    /// order. Almost always zero or one line; a recipe mixing `debug`
    /// and `info` args produces both, kept separate so each line keeps
    /// its own severity rather than being merged under the wrong one.
    /// Args left at the default `level = "hidden"` never appear.
    ///
    /// `workspace_root` is only used for `kind = "path"` args, to add a
    /// best-effort "is this actually inside the workspace" note — this
    /// is informational for the log, not an access check (see
    /// `text`'s `workspace_path` module for the real one).
    pub fn log_lines(
        &self,
        provided: &[String],
        workspace_root: &Path,
    ) -> Vec<(ArgLogLevel, String)> {
        let mut debug_parts = Vec::new();
        let mut info_parts = Vec::new();
        for ((name, arg), value) in self.args.iter().zip(provided) {
            let formatted = match arg.level {
                ArgLogLevel::Hidden => continue,
                ArgLogLevel::Debug | ArgLogLevel::Info => {
                    format_arg(name, arg.kind, value, workspace_root)
                }
            };
            match arg.level {
                ArgLogLevel::Debug => debug_parts.push(formatted),
                ArgLogLevel::Info => info_parts.push(formatted),
                ArgLogLevel::Hidden => unreachable!("filtered out above"),
            }
        }

        [
            (ArgLogLevel::Debug, debug_parts),
            (ArgLogLevel::Info, info_parts),
        ]
        .into_iter()
        .filter(|(_, parts)| !parts.is_empty())
        .map(|(level, parts)| (level, join_formatted(&parts)))
        .collect()
    }
}

/// One formatted parameter, pending assembly into a full log line by
/// [`join_formatted`]. `Inline` sits on the line's shared `, `-joined
/// segment; `Block` (from `kind = "text"`) gets its own multi-line
/// segment so a value's embedded newlines stay real line breaks.
enum Formatted {
    Inline(String),
    Block { name: String, indented: String },
}

fn format_arg(name: &str, kind: ArgKind, value: &str, workspace_root: &Path) -> Formatted {
    match kind {
        ArgKind::Value => Formatted::Inline(format!("{name}={value}")),
        ArgKind::Path => Formatted::Inline(format!(
            "{name}={value}{}",
            path_note(value, workspace_root)
        )),
        ArgKind::Text => {
            let indented = value
                .lines()
                .map(|line| format!("    {line}"))
                .collect::<Vec<_>>()
                .join("\n");
            Formatted::Block {
                name: name.to_owned(),
                indented,
            }
        }
    }
}

/// Best-effort, informational only — never used to grant or deny
/// access. `value` is resolved the same way a plain filesystem path
/// would be (absolute values are taken as-is, relative ones joined onto
/// `workspace_root`), then canonicalized so `..`/symlinks are actually
/// followed rather than merely inspected lexically. A path that doesn't
/// exist yet (e.g. a `create` target) can't be canonicalized, hence
/// "unresolved" rather than a false "outside".
fn path_note(value: &str, workspace_root: &Path) -> &'static str {
    let Ok(canonical_root) = workspace_root.canonicalize() else {
        return " (unresolved)";
    };
    match workspace_root.join(value).canonicalize() {
        Ok(canonical) if canonical.starts_with(&canonical_root) => "",
        Ok(_) => " (outside workspace)",
        Err(_) => " (unresolved)",
    }
}

/// Joins one severity's formatted args into a single log line: all
/// `Inline` parts share one `, `-joined segment first, then each `Block`
/// follows as its own `name:`-headed segment.
fn join_formatted(parts: &[Formatted]) -> String {
    let mut segments = Vec::new();

    let inline = parts
        .iter()
        .filter_map(|p| match p {
            Formatted::Inline(s) => Some(s.as_str()),
            Formatted::Block { .. } => None,
        })
        .collect::<Vec<_>>()
        .join(", ");
    if !inline.is_empty() {
        segments.push(inline);
    }

    for part in parts {
        if let Formatted::Block { name, indented } = part {
            segments.push(format!("{name}:\n{indented}"));
        }
    }

    segments.join("\n")
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
    pub fn load(path: &Path) -> Result<Self, LoadError> {
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
    fn substitutes_named_placeholder() {
        let recipe = Recipe {
            description: String::new(),
            args: IndexMap::from([(
                "name".to_owned(),
                RecipeArg {
                    help: String::new(),
                    ..Default::default()
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
                    ..Default::default()
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
                    ..Default::default()
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

    fn arg(level: ArgLogLevel, kind: ArgKind) -> RecipeArg {
        RecipeArg {
            help: String::new(),
            level,
            kind,
        }
    }

    #[test]
    fn hidden_args_never_appear_in_log_lines() {
        let recipe = Recipe {
            description: String::new(),
            args: IndexMap::from([(
                "secret".to_owned(),
                arg(ArgLogLevel::Hidden, ArgKind::Value),
            )]),
            run: vec!["echo".to_owned()],
            annotations: RecipeAnnotations::default(),
        };
        let root = assert_fs::TempDir::new().unwrap();
        assert!(
            recipe
                .log_lines(&["s3cr3t".to_owned()], root.path())
                .is_empty()
        );
    }

    #[test]
    fn info_level_value_arg_logs_as_key_equals_value() {
        let recipe = Recipe {
            description: String::new(),
            args: IndexMap::from([("project".to_owned(), arg(ArgLogLevel::Info, ArgKind::Value))]),
            run: vec!["echo".to_owned()],
            annotations: RecipeAnnotations::default(),
        };
        let root = assert_fs::TempDir::new().unwrap();
        let lines = recipe.log_lines(&["kid-editor".to_owned()], root.path());
        assert_eq!(
            lines,
            vec![(ArgLogLevel::Info, "project=kid-editor".to_owned())]
        );
    }

    #[test]
    fn debug_and_info_args_produce_separate_lines() {
        let recipe = Recipe {
            description: String::new(),
            args: IndexMap::from([
                ("number".to_owned(), arg(ArgLogLevel::Info, ArgKind::Value)),
                ("body".to_owned(), arg(ArgLogLevel::Debug, ArgKind::Value)),
            ]),
            run: vec!["echo".to_owned()],
            annotations: RecipeAnnotations::default(),
        };
        let root = assert_fs::TempDir::new().unwrap();
        let lines = recipe.log_lines(&["42".to_owned(), "hi".to_owned()], root.path());
        assert_eq!(
            lines,
            vec![
                (ArgLogLevel::Debug, "body=hi".to_owned()),
                (ArgLogLevel::Info, "number=42".to_owned()),
            ]
        );
    }

    #[test]
    fn text_kind_preserves_newlines_as_indented_block() {
        let recipe = Recipe {
            description: String::new(),
            args: IndexMap::from([("message".to_owned(), arg(ArgLogLevel::Info, ArgKind::Text))]),
            run: vec!["echo".to_owned()],
            annotations: RecipeAnnotations::default(),
        };
        let root = assert_fs::TempDir::new().unwrap();
        let lines = recipe.log_lines(&["line one\nline two".to_owned()], root.path());
        assert_eq!(
            lines,
            vec![(
                ArgLogLevel::Info,
                "message:\n    line one\n    line two".to_owned()
            )]
        );
    }

    #[test]
    fn mixing_value_and_text_kind_at_same_level_joins_inline_then_block() {
        let recipe = Recipe {
            description: String::new(),
            args: IndexMap::from([
                ("number".to_owned(), arg(ArgLogLevel::Info, ArgKind::Value)),
                ("body".to_owned(), arg(ArgLogLevel::Info, ArgKind::Text)),
            ]),
            run: vec!["echo".to_owned()],
            annotations: RecipeAnnotations::default(),
        };
        let root = assert_fs::TempDir::new().unwrap();
        let lines = recipe.log_lines(&["42".to_owned(), "a\nb".to_owned()], root.path());
        assert_eq!(
            lines,
            vec![(
                ArgLogLevel::Info,
                "number=42\nbody:\n    a\n    b".to_owned()
            )]
        );
    }

    #[test]
    fn path_kind_inside_workspace_has_no_extra_note() {
        let recipe = Recipe {
            description: String::new(),
            args: IndexMap::from([("path".to_owned(), arg(ArgLogLevel::Info, ArgKind::Path))]),
            run: vec!["echo".to_owned()],
            annotations: RecipeAnnotations::default(),
        };
        let root = assert_fs::TempDir::new().unwrap();
        std::fs::write(root.path().join("notes.md"), "hi").unwrap();
        let lines = recipe.log_lines(&["notes.md".to_owned()], root.path());
        assert_eq!(lines, vec![(ArgLogLevel::Info, "path=notes.md".to_owned())]);
    }

    #[test]
    fn path_kind_outside_workspace_is_flagged() {
        let recipe = Recipe {
            description: String::new(),
            args: IndexMap::from([("path".to_owned(), arg(ArgLogLevel::Info, ArgKind::Path))]),
            run: vec!["echo".to_owned()],
            annotations: RecipeAnnotations::default(),
        };
        let root = assert_fs::TempDir::new().unwrap();
        let outside = assert_fs::TempDir::new().unwrap();
        let outside_file = outside.path().join("secret.txt");
        std::fs::write(&outside_file, "top secret").unwrap();
        let lines = recipe.log_lines(&[outside_file.display().to_string()], root.path());
        assert_eq!(
            lines,
            vec![(
                ArgLogLevel::Info,
                format!("path={} (outside workspace)", outside_file.display())
            )]
        );
    }

    #[test]
    fn path_kind_nonexistent_target_is_unresolved() {
        let recipe = Recipe {
            description: String::new(),
            args: IndexMap::from([("path".to_owned(), arg(ArgLogLevel::Info, ArgKind::Path))]),
            run: vec!["echo".to_owned()],
            annotations: RecipeAnnotations::default(),
        };
        let root = assert_fs::TempDir::new().unwrap();
        let lines = recipe.log_lines(&["not-yet-created.md".to_owned()], root.path());
        assert_eq!(
            lines,
            vec![(
                ArgLogLevel::Info,
                "path=not-yet-created.md (unresolved)".to_owned()
            )]
        );
    }
}
