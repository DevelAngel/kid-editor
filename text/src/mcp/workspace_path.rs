//! No tool may touch a path outside the workspace root, or under an
//! ignored name. [`UnresolvedPath`] is unchecked client input;
//! [`WorkspacePath`] is proof the check passed; [`WriteBuffer`]
//! additionally proves the path isn't the configured `--recipes-file`
//! (ADR 0004).

use glob::{Pattern, PatternError};
use rmcp::model::ErrorData as McpError;
use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

use std::fmt::{self, Display, Formatter};
use std::fs::{self, File, Metadata, OpenOptions, ReadDir};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

/// A single `--ignore`/`--extra-ignore` entry: a glob pattern, optionally
/// anchored to the workspace's top level with a leading `/`; or an exact
/// workspace-relative path (see [`IgnorePattern::exact_path`]), used
/// internally rather than accepted from `--ignore` input.
#[derive(Clone, Debug)]
pub struct IgnorePattern {
    /// `true` if the pattern had a leading `/`: matched only against a
    /// path's first component. Always `true` for an exact path — see
    /// `exact` below.
    anchored: bool,
    glob: Pattern,
    /// Set only by [`IgnorePattern::exact_path`]: protects one specific,
    /// possibly nested file by its exact workspace-relative path rather
    /// than by name. Needed for `--recipes-file`, which — unlike
    /// `justfile` — can be placed and named however the person likes;
    /// a name-based glob would either protect every same-named file
    /// anywhere in the workspace (over-matching) or be unable to target
    /// a specific nested path at all (under-matching). `None` for every
    /// ordinary `--ignore`/`--extra-ignore` entry, which `matches` and
    /// `matches_name_at_depth` continue to handle via `glob` as before.
    exact: Option<PathBuf>,
}

impl IgnorePattern {
    /// Protects exactly `relative` — the workspace-relative path of the
    /// file passed via `--recipes-file`, once resolved and confirmed to
    /// actually lie inside the workspace (see `McpServer::serve`; a
    /// `--recipes-file` outside the workspace is already unreachable
    /// through this server, so nothing here needs to protect it — see
    /// ADR 0004's "More Information" amendment).
    pub(crate) fn exact_path(relative: PathBuf) -> Self {
        Self {
            anchored: true,
            glob: Pattern::new("").expect("empty pattern is valid"),
            exact: Some(relative),
        }
    }

    /// Checks a single directory entry's name at `depth` (0 = direct
    /// child of the root); an anchored pattern only matches at depth 0.
    /// For an exact-path pattern: matches when `name` equals that path's
    /// last component *and* `depth` equals that path's own depth — e.g.
    /// `exact_path("sub/recipes.toml")` (depth 1) hides an entry named
    /// `recipes.toml` at depth 1, regardless of which directory it's
    /// actually in. That's deliberately looser than full-path equality
    /// (which [`matches`](Self::matches) does check, for the paths
    /// tools actually resolve) — this method only sees one path
    /// component at a time as `tree` walks, with no way to know the
    /// full path leading to it, so matching by depth and name is the
    /// closest approximation available. The failure mode is hiding an
    /// unrelated same-named, same-depth file from `tree`'s listing, not
    /// exposing the protected one — the same direction of imprecision
    /// [`is_justfile_like`]/`justfile_patterns` already accept.
    pub(crate) fn matches_name_at_depth(&self, name: &str, depth: usize) -> bool {
        if let Some(exact) = &self.exact {
            return exact.components().count() == depth + 1
                && exact.file_name().and_then(|n| n.to_str()) == Some(name);
        }
        if self.anchored && depth != 0 {
            return false;
        }
        self.glob.matches(name)
    }

    fn matches(&self, relative: &Path) -> bool {
        if let Some(exact) = &self.exact {
            return relative == exact;
        }
        let mut components = relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy());
        if self.anchored {
            components.next().is_some_and(|c| self.glob.matches(&c))
        } else {
            components.any(|c| self.glob.matches(&c))
        }
    }
}

impl Display for IgnorePattern {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if let Some(exact) = &self.exact {
            return write!(f, "{}", exact.display());
        }
        if self.anchored {
            write!(f, "/")?;
        }
        write!(f, "{}", self.glob.as_str())
    }
}

impl FromStr for IgnorePattern {
    type Err = PatternError;

    fn from_str(pattern: &str) -> Result<Self, Self::Err> {
        let (anchored, pattern) = match pattern.strip_prefix('/') {
            Some(rest) => (true, rest),
            None => (false, pattern),
        };
        let glob = Pattern::new(pattern)?;
        Ok(Self {
            anchored,
            glob,
            exact: None,
        })
    }
}

/// A path exactly as a tool received it over the wire: unchecked.
/// [`resolve`](UnresolvedPath::resolve) is the only way to validate it.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct UnresolvedPath(PathBuf); // AI: Never make the PathBuf pub!

impl UnresolvedPath {
    #[cfg(test)]
    pub fn new(unresolved: impl Into<PathBuf>) -> Self {
        Self(unresolved.into())
    }

    /// Checks this path against `workspace_root` and `ignore`.
    pub fn resolve(
        self,
        workspace_root: &Path,
        ignore: &[IgnorePattern],
    ) -> Result<WorkspacePath, McpError> {
        WorkspacePath::new(self, workspace_root, ignore)
    }

    /// Lexically collapses `.` and `..`, without touching the filesystem —
    /// so this also works for paths that don't exist yet (`create`).
    /// Returns `None` if a `..` has nothing left to pop: that means the
    /// path steps above where it started, and silently dropping it (as a
    /// naive stack-based collapse would) would turn a workspace escape
    /// into a path that merely *looks* safe.
    fn normalise(&self) -> Option<PathBuf> {
        let mut out = PathBuf::new();
        for component in self.0.components() {
            match component {
                Component::ParentDir => {
                    if !out.pop() {
                        return None;
                    }
                }
                Component::CurDir => {}
                other => out.push(other.as_os_str()),
            }
        }
        Some(out)
    }
}

/// A path proven to lie inside the workspace root and not under any
/// ignored name. The only way to obtain one is [`WorkspacePath::new`] (or
/// [`UnresolvedPath::resolve`], which just calls it).
#[derive(Debug)]
pub struct WorkspacePath {
    /// Relative to the workspace root, normalized — e.g. `notes/todo.md`,
    /// never `/abs/...` and never containing `..`. Also what [`Display`]
    /// shows: the path in the caller's own vocabulary.
    relative: PathBuf,
    /// `workspace_root` joined with `relative` — computed once, here.
    absolute: PathBuf,
}

impl WorkspacePath {
    /// `workspace_root` must already be canonicalized (the server does
    /// this once at startup — see `McpServer::serve`).
    ///
    /// Three checks, in order: (1) a leading `/` in the client path means
    /// "relative to the workspace", not the real filesystem root — it's
    /// stripped, and anything still absolute after that (a drive letter, a
    /// UNC path, ...) is rejected outright rather than guessed at; (2) `..`
    /// is resolved lexically, and rejected if it has nothing left to pop,
    /// no matter how deep the redirection (`a/../../b` is caught exactly
    /// like `../b`); (3) any path component matching an [`IgnorePattern`]
    /// in `ignore` is rejected the same way a nonexistent path would be —
    /// see [`not_found_or_io`] for why.
    ///
    /// Symlinks are deliberately **not** resolved or checked here: no tool
    /// in this module can create one, so the only way one can exist inside
    /// the workspace is if whoever set the workspace up put it there
    /// themselves — a decision made outside this server, not something a
    /// client can trigger through it.
    pub fn new(
        unresolved: UnresolvedPath,
        workspace_root: &Path,
        ignore: &[IgnorePattern],
    ) -> Result<Self, McpError> {
        // Lexically collapse `.` and `..` in path.
        let Some(unresolved) = unresolved.normalise() else {
            return Err(escapes_workspace(&unresolved.0));
        };

        // Treat a leading `/` as "relative to the workspace",
        // not as the real filesystem root.
        // Anything still absolute afterwards is a form
        // we don't try to interpret.
        let relative = unresolved
            .strip_prefix("/")
            .map(Path::to_path_buf)
            .unwrap_or(unresolved);
        if relative.is_absolute() {
            return Err(escapes_workspace(&relative));
        }

        // No component may match an ignored pattern.
        if ignore.iter().any(|pattern| pattern.matches(&relative)) {
            return Err(McpError::invalid_params(
                format!("{}: no such file or directory", display_relative(&relative)),
                None,
            ));
        }

        let absolute = workspace_root.join(&relative);

        Ok(Self { relative, absolute })
    }

    /// The workspace root itself — what `tree`'s `path: None` means.
    /// `relative` is empty on purpose; not a general-purpose constructor.
    pub(super) fn root(workspace_root: &Path) -> Self {
        Self {
            relative: PathBuf::new(),
            absolute: workspace_root.to_path_buf(),
        }
    }

    #[cfg(test)]
    pub(super) fn absolute(&self) -> &Path {
        &self.absolute
    }

    pub(super) fn metadata(&self) -> io::Result<Metadata> {
        fs::metadata(&self.absolute)
    }

    pub(super) fn read_dir(&self) -> io::Result<ReadDir> {
        fs::read_dir(&self.absolute)
    }

    pub(super) fn read_to_string(&self) -> io::Result<String> {
        fs::read_to_string(&self.absolute)
    }

    /// Fails if `protect_recipe_toml` is `Some(relative)` and this path
    /// is exactly `relative` — the workspace-relative location of the
    /// file passed via `--recipes-file`, already confirmed to lie inside
    /// the workspace (see `McpServer::serve`; `None` covers both "no
    /// recipes file configured" and "configured, but outside the
    /// workspace" — the latter needs no protection here since it's
    /// already unreachable through every tool in this module).
    ///
    /// There is no other way to obtain a [`WriteBuffer`], so this is not
    /// something a tool could forget to check.
    pub(super) fn into_write_buffer(
        self,
        protect_recipe_toml: Option<&Path>,
    ) -> Result<WriteBuffer, McpError> {
        if protect_recipe_toml.is_some_and(|protected| protected == self.relative) {
            return Err(McpError::invalid_params(
                format!(
                    "{self}: the configured recipe file is read-only through this server (see ADR 0004)"
                ),
                None,
            ));
        }
        Ok(WriteBuffer(self))
    }
}

/// A [`WorkspacePath`] proven *not* to be the protected recipe file.
/// Only [`WorkspacePath::into_write_buffer`] produces one.
#[derive(Debug)]
pub struct WriteBuffer(WorkspacePath);

impl WriteBuffer {
    /// Opens the file truncated, creating it and its parent directories as
    /// needed. Returns a plain `std::fs::File` — `std::io::Write` and
    /// friends come straight from the standard library, nothing here
    /// reimplements them; this method only gates which `File` a caller
    /// can ever get.
    pub(super) fn open(&self) -> io::Result<File> {
        if let Some(parent) = self.0.absolute.parent() {
            fs::create_dir_all(parent)?;
        }
        OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.0.absolute)
    }
}

impl Display for WriteBuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

fn escapes_workspace(unresolved: &Path) -> McpError {
    McpError::invalid_params(
        format!("path escapes the workspace: {}", unresolved.display()),
        None,
    )
}

/// `relative.display()`, except an empty path (the workspace root itself —
/// what `.` normalizes to) reads as `.` instead of the empty string.
fn display_relative(relative: &Path) -> String {
    if relative.as_os_str().is_empty() {
        ".".to_owned()
    } else {
        relative.display().to_string()
    }
}

/// Shows the path the way the caller wrote it (relative to the workspace),
/// never the absolute filesystem path — callers should never see a
/// filesystem layout they didn't ask about.
impl Display for WorkspacePath {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", display_relative(&self.relative))
    }
}

/// Turns a `std::io::Error` from an operation on an already-resolved
/// `WorkspacePath` into an `McpError`, reporting "not found" the same way
/// whether the path genuinely never existed or was hidden by the ignore
/// list. The two cases are indistinguishable on purpose: a caller probing
/// for the boundary between "doesn't exist" and "exists but is off
/// limits" should learn nothing from the difference, because there isn't
/// meant to be one.
pub(super) fn not_found_or_io(display_path: impl fmt::Display, e: std::io::Error) -> McpError {
    if e.kind() == std::io::ErrorKind::NotFound {
        McpError::invalid_params(format!("{display_path}: no such file or directory"), None)
    } else {
        McpError::internal_error(format!("{display_path}: {e}"), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use rmcp::model::ErrorCode;
    use std::assert_matches;
    use std::fs;
    use std::io::Write;
    use std::os::unix;

    const EMPTY_IGNORE: &[IgnorePattern] = &[];

    /// Parses each `&str` into an [`IgnorePattern`], panicking on an
    /// invalid glob — fine for tests, where the whole point is that
    /// production code never gets this far with a bad pattern.
    fn ignore_patterns(patterns: &[&str]) -> Vec<IgnorePattern> {
        patterns.iter().map(|p| p.parse().unwrap()).collect()
    }

    impl UnresolvedPath {
        fn resolve_relative(unresolved: impl Into<PathBuf>, root: &Path) -> String {
            UnresolvedPath::new(unresolved)
                .resolve(root, EMPTY_IGNORE)
                .unwrap()
                .to_string()
        }
        fn resolve_workspace(unresolved: impl Into<PathBuf>, root: &Path) -> PathBuf {
            UnresolvedPath::new(unresolved)
                .resolve(root, EMPTY_IGNORE)
                .unwrap()
                .absolute()
                .to_path_buf()
        }
        fn resolve_workspace_with_ignore(
            unresolved: impl Into<PathBuf>,
            root: &Path,
            ignore: &[IgnorePattern],
        ) -> PathBuf {
            UnresolvedPath::new(unresolved)
                .resolve(root, ignore)
                .unwrap()
                .absolute()
                .to_path_buf()
        }
        fn resolve_fails(unresolved: impl Into<PathBuf>, root: &Path) -> McpError {
            UnresolvedPath::new(unresolved)
                .resolve(root, EMPTY_IGNORE)
                .unwrap_err()
        }
        fn resolve_with_irgnore_fails(
            unresolved: impl Into<PathBuf>,
            root: &Path,
            ignore: &[IgnorePattern],
        ) -> McpError {
            UnresolvedPath::new(unresolved)
                .resolve(root, ignore)
                .unwrap_err()
        }
    }

    #[test]
    fn relative_path_stays_in_workspace() {
        const UNRESOLVED: &str = "notes/todo.md";
        let root = TempDir::new().unwrap();
        assert_eq!(
            UnresolvedPath::resolve_workspace(UNRESOLVED, root.path()),
            root.path().join(UNRESOLVED)
        );
    }

    #[test]
    fn treats_absolute_path_as_workspace_relative() {
        const UNRESOLVED: &str = "/notes/todo.md";
        const RESOLVED: &str = "notes/todo.md";
        let root = TempDir::new().unwrap();
        assert_eq!(
            UnresolvedPath::resolve_workspace(UNRESOLVED, root.path()),
            root.path().join(RESOLVED)
        );
    }

    #[test]
    fn treats_front_abnormal_absolute_path_as_workspace_relative() {
        const UNRESOLVED: &str = "///notes/todo.md";
        const RESOLVED: &str = "notes/todo.md";
        let root = TempDir::new().unwrap();
        assert_eq!(
            UnresolvedPath::resolve_workspace(UNRESOLVED, root.path()),
            root.path().join(RESOLVED)
        );
    }

    #[test]
    fn treats_mid_abnormal_absolute_path_as_workspace_relative() {
        const UNRESOLVED: &str = "/notes///todo.md";
        const RESOLVED: &str = "notes/todo.md";
        let root = TempDir::new().unwrap();
        assert_eq!(
            UnresolvedPath::resolve_workspace(UNRESOLVED, root.path()),
            root.path().join(RESOLVED)
        );
    }

    #[test]
    fn rejects_dotdot_escape() {
        const UNRESOLVED: &str = "../outside.txt";
        let root = TempDir::new().unwrap();
        assert_matches!(
            UnresolvedPath::resolve_fails(UNRESOLVED, root.path()),
            McpError {
                code: ErrorCode::INVALID_PARAMS,
                ..
            }
        );
    }

    #[test]
    fn rejects_dotdot_that_stays_lexically_inside_but_traverses_out() {
        // "sub/../../escape" has nowhere left to pop for the second `..`,
        // even though the path starts inside the workspace.
        const UNRESOLVED: &str = "sub/../../escape";
        let root = TempDir::new().unwrap();
        assert_matches!(
            UnresolvedPath::resolve_fails(UNRESOLVED, root.path()),
            McpError {
                code: ErrorCode::INVALID_PARAMS,
                ..
            }
        );
    }

    #[test]
    fn allows_dotdot_that_stays_inside() {
        const UNRESOLVED: &str = "sub/../README.md";
        const RESOLVED: &str = "README.md";
        let root = TempDir::new().unwrap();
        assert_eq!(
            UnresolvedPath::resolve_workspace(UNRESOLVED, root.path()),
            root.path().join(RESOLVED)
        );
    }

    #[test]
    fn rejects_ignored_top_level_entry() {
        let unresolved = &PathBuf::from(".git");
        let ignore = ignore_patterns(&[".git"]);
        let root = TempDir::new().unwrap();
        assert_matches!(
            UnresolvedPath::resolve_with_irgnore_fails(unresolved, root.path(), &ignore),
            McpError {
                code: ErrorCode::INVALID_PARAMS,
                ..
            }
        );
        assert_matches!(
            UnresolvedPath::resolve_with_irgnore_fails(
                unresolved.join("config"),
                root.path(),
                &ignore
            ),
            McpError {
                code: ErrorCode::INVALID_PARAMS,
                ..
            }
        );
    }

    #[test]
    fn rejects_path_nested_under_ignored_directory() {
        let unresolved = &PathBuf::from("target/debug/deep/file.txt");
        let ignore = ignore_patterns(&["target"]);
        let root = TempDir::new().unwrap();
        assert_matches!(
            UnresolvedPath::resolve_with_irgnore_fails(unresolved, root.path(), &ignore),
            McpError {
                code: ErrorCode::INVALID_PARAMS,
                ..
            }
        );
    }

    #[test]
    fn allows_entry_not_matching_ignore_list() {
        const UNRESOLVED: &str = "src/main.rs";
        let ignore = ignore_patterns(&["target"]);
        let root = TempDir::new().unwrap();
        assert_eq!(
            UnresolvedPath::resolve_workspace_with_ignore(UNRESOLVED, root.path(), &ignore),
            root.path().join(UNRESOLVED)
        );
    }

    #[test]
    fn unanchored_glob_pattern_matches_any_depth() {
        let ignore = ignore_patterns(&["*.log"]);
        let root = TempDir::new().unwrap();
        assert_matches!(
            UnresolvedPath::resolve_with_irgnore_fails("debug.log", root.path(), &ignore),
            McpError {
                code: ErrorCode::INVALID_PARAMS,
                ..
            }
        );
        assert_matches!(
            UnresolvedPath::resolve_with_irgnore_fails(
                "nested/deep/debug.log",
                root.path(),
                &ignore
            ),
            McpError {
                code: ErrorCode::INVALID_PARAMS,
                ..
            }
        );
    }

    #[test]
    fn anchored_pattern_only_matches_top_level() {
        let ignore = ignore_patterns(&["/justfile"]);
        let root = TempDir::new().unwrap();
        assert_matches!(
            UnresolvedPath::resolve_with_irgnore_fails("justfile", root.path(), &ignore),
            McpError {
                code: ErrorCode::INVALID_PARAMS,
                ..
            }
        );
        assert_eq!(
            UnresolvedPath::resolve_workspace_with_ignore("sub/justfile", root.path(), &ignore),
            root.path().join("sub/justfile")
        );
    }

    #[test]
    fn anchored_glob_pattern_matches_top_level_only() {
        let ignore = ignore_patterns(&["/*.log"]);
        let root = TempDir::new().unwrap();
        assert_matches!(
            UnresolvedPath::resolve_with_irgnore_fails("debug.log", root.path(), &ignore),
            McpError {
                code: ErrorCode::INVALID_PARAMS,
                ..
            }
        );
        assert_eq!(
            UnresolvedPath::resolve_workspace_with_ignore("nested/debug.log", root.path(), &ignore),
            root.path().join("nested/debug.log")
        );
    }

    #[test]
    fn invalid_glob_pattern_is_rejected_at_parse_time() {
        assert!("[".parse::<IgnorePattern>().is_err());
        assert!("/[".parse::<IgnorePattern>().is_err());
    }

    #[test]
    fn relative_collapses_dot_and_dotdot() {
        const UNRESOLVED: &str = "a/./b/../c";
        const RESOLVED: &str = "a/c";
        let root = TempDir::new().unwrap();
        assert_eq!(
            UnresolvedPath::resolve_workspace(UNRESOLVED, root.path()),
            root.path().join(RESOLVED)
        );
    }

    #[test]
    fn relative_rejects_unresolvable_dotdot() {
        const UNRESOLVED: &str = "../escape";
        let root = TempDir::new().unwrap();
        assert_matches!(
            UnresolvedPath::resolve_fails(UNRESOLVED, root.path()),
            McpError {
                code: ErrorCode::INVALID_PARAMS,
                ..
            }
        );
    }

    #[test]
    fn display_shows_workspace_relative_input_not_absolute_path() {
        const UNRESOLVED: &str = "notes/todo.md";
        let root = TempDir::new().unwrap();
        assert_eq!(
            UnresolvedPath::resolve_relative(UNRESOLVED, root.path()),
            UNRESOLVED
        );
    }

    #[test]
    fn display_of_workspace_root_itself_is_dot() {
        const UNRESOLVED: &str = ".";
        let root = TempDir::new().unwrap();
        assert_eq!(
            UnresolvedPath::resolve_relative(UNRESOLVED, root.path()),
            UNRESOLVED
        );
        assert_eq!(
            UnresolvedPath::resolve_workspace(UNRESOLVED, root.path()),
            root.join(UNRESOLVED)
        );
    }

    #[test]
    fn into_write_buffer_refuses_the_configured_recipe_file() {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("recipes.toml"), "").unwrap();
        let path = UnresolvedPath::new("recipes.toml")
            .resolve(root.path(), EMPTY_IGNORE)
            .unwrap();
        assert_matches!(
            path.into_write_buffer(Some(Path::new("recipes.toml")))
                .unwrap_err(),
            McpError {
                code: ErrorCode::INVALID_PARAMS,
                ..
            }
        );
    }

    #[test]
    fn into_write_buffer_refuses_nested_configured_recipe_file() {
        let root = TempDir::new().unwrap();
        fs::create_dir_all(root.path().join("sub")).unwrap();
        fs::write(root.path().join("sub/recipes.toml"), "").unwrap();
        let path = UnresolvedPath::new("sub/recipes.toml")
            .resolve(root.path(), EMPTY_IGNORE)
            .unwrap();
        assert_matches!(
            path.into_write_buffer(Some(Path::new("sub/recipes.toml")))
                .unwrap_err(),
            McpError {
                code: ErrorCode::INVALID_PARAMS,
                ..
            }
        );
    }

    #[test]
    fn into_write_buffer_only_refuses_the_exact_configured_path() {
        // A same-named file elsewhere in the workspace is not the
        // configured recipe file and stays writable — protection is by
        // exact path, not by filename (see IgnorePattern::exact_path).
        let root = TempDir::new().unwrap();
        fs::create_dir_all(root.path().join("other")).unwrap();
        let path = UnresolvedPath::new("other/recipes.toml")
            .resolve(root.path(), EMPTY_IGNORE)
            .unwrap();
        let write = path
            .into_write_buffer(Some(Path::new("configured/recipes.toml")))
            .unwrap();
        write.open().unwrap().write_all(b"hello").unwrap();
        assert_eq!(
            fs::read_to_string(root.path().join("other/recipes.toml")).unwrap(),
            "hello"
        );
    }

    #[test]
    fn into_write_buffer_allows_recipe_file_when_protection_disabled() {
        // Default state (no `--recipes-file`, or one outside the
        // workspace — already unreachable through every tool here, so
        // nothing needs protecting): nothing left for editing to bypass
        // (ADR 0003, ADR 0004).
        let root = TempDir::new().unwrap();
        let path = UnresolvedPath::new("recipes.toml")
            .resolve(root.path(), EMPTY_IGNORE)
            .unwrap();
        let write = path.into_write_buffer(None).unwrap();
        write
            .open()
            .unwrap()
            .write_all(b"[recipe.check]\n")
            .unwrap();
        assert_eq!(
            fs::read_to_string(root.path().join("recipes.toml")).unwrap(),
            "[recipe.check]\n"
        );
    }

    #[test]
    fn exact_path_pattern_matches_only_that_path() {
        let pattern = IgnorePattern::exact_path(PathBuf::from("sub/recipes.toml"));
        assert!(pattern.matches(Path::new("sub/recipes.toml")));
        assert!(!pattern.matches(Path::new("recipes.toml")));
        assert!(!pattern.matches(Path::new("other/recipes.toml")));
    }

    #[test]
    fn exact_path_pattern_matches_name_at_its_own_depth() {
        let pattern = IgnorePattern::exact_path(PathBuf::from("sub/recipes.toml"));
        assert!(pattern.matches_name_at_depth("recipes.toml", 1));
        assert!(!pattern.matches_name_at_depth("recipes.toml", 0));
        assert!(!pattern.matches_name_at_depth("other.toml", 1));
    }

    #[test]
    fn into_write_buffer_allows_other_files() {
        let root = TempDir::new().unwrap();
        let path = UnresolvedPath::new("notes.md")
            .resolve(root.path(), EMPTY_IGNORE)
            .unwrap();
        let write = path
            .into_write_buffer(Some(Path::new("recipes.toml")))
            .unwrap();
        write.open().unwrap().write_all(b"hello").unwrap();
        assert_eq!(
            fs::read_to_string(root.path().join("notes.md")).unwrap(),
            "hello"
        );
    }

    #[test]
    fn write_buffer_creates_parent_directories() {
        let root = TempDir::new().unwrap();
        let path = UnresolvedPath::new("deep/nested/notes.md")
            .resolve(root.path(), EMPTY_IGNORE)
            .unwrap();
        let write = path
            .into_write_buffer(Some(Path::new("recipes.toml")))
            .unwrap();
        write.open().unwrap().write_all(b"hello").unwrap();
        assert_eq!(
            fs::read_to_string(root.path().join("deep/nested/notes.md")).unwrap(),
            "hello"
        );
    }

    #[test]
    fn write_buffer_truncates_existing_content() {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("notes.md"), "old content, much longer").unwrap();
        let path = UnresolvedPath::new("notes.md")
            .resolve(root.path(), EMPTY_IGNORE)
            .unwrap();
        let write = path
            .into_write_buffer(Some(Path::new("recipes.toml")))
            .unwrap();
        write.open().unwrap().write_all(b"new").unwrap();
        assert_eq!(
            fs::read_to_string(root.path().join("notes.md")).unwrap(),
            "new"
        );
    }

    #[cfg_attr(unix, test)]
    fn doesnt_follow_symlink_inside_workspace() {
        const INSIDE_FILE: &str = "secret.txt";
        const UNRESOLVED: &str = "link.txt";
        let root = TempDir::new().unwrap();
        fs::write(root.path().join(INSIDE_FILE), "top secret").unwrap();
        unix::fs::symlink(root.path().join("secret.txt"), root.path().join(UNRESOLVED)).unwrap();
        assert_eq!(
            fs::read_to_string(root.path().join(UNRESOLVED)).unwrap(),
            fs::read_to_string(root.path().join(INSIDE_FILE)).unwrap()
        );

        // Note: The symlink is not visible to the MCP client and
        //       the creation of symlink is not supported (or planned).
        //       Thus, it is okay to handle them as normal files.
        assert_eq!(
            UnresolvedPath::resolve_relative(UNRESOLVED, root.path()),
            UNRESOLVED
        );
        assert_eq!(
            UnresolvedPath::resolve_workspace(UNRESOLVED, root.path()),
            root.path().join(UNRESOLVED)
        );
        assert_eq!(
            UnresolvedPath::resolve_relative(INSIDE_FILE, root.path()),
            INSIDE_FILE
        );
        assert_eq!(
            UnresolvedPath::resolve_workspace(INSIDE_FILE, root.path()),
            root.path().join(INSIDE_FILE)
        );
    }

    #[cfg_attr(unix, test)]
    fn doesnt_follow_symlink_outside_workspace() {
        const OUTSIDE_FILE: &str = "secret.txt";
        let outside = TempDir::with_prefix("outside-").unwrap();
        fs::write(outside.path().join(OUTSIDE_FILE), "top secret").unwrap();

        const UNRESOLVED: &str = "link.txt";
        let root = TempDir::with_prefix("workspace-").unwrap();
        unix::fs::symlink(
            // original
            outside.path().join("secret.txt"),
            // limk
            root.path().join(UNRESOLVED),
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(root.path().join(UNRESOLVED)).unwrap(),
            fs::read_to_string(outside.path().join(OUTSIDE_FILE)).unwrap()
        );

        // Note: The symlink is not visible to the MCP client and
        //       the creation of symlink is not supported (or planned).
        //       Thus, it is okay to handle them as normal files.
        assert_eq!(
            UnresolvedPath::resolve_relative(UNRESOLVED, root.path()),
            UNRESOLVED
        );
        assert_eq!(
            UnresolvedPath::resolve_workspace(UNRESOLVED, root.path()),
            root.path().join(UNRESOLVED)
        );
        assert_eq!(
            UnresolvedPath::resolve_relative(outside.path().join(OUTSIDE_FILE), root.path()),
            outside.strip_prefix("/").unwrap().join(OUTSIDE_FILE)
        );
        assert_eq!(
            UnresolvedPath::resolve_workspace(outside.path().join(OUTSIDE_FILE), root.path()),
            root.path()
                .join(outside.strip_prefix("/").unwrap())
                .join(OUTSIDE_FILE)
        );
    }
}
