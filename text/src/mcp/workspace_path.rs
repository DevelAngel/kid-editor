//! The one law of this server: **no tool may touch a path outside the
//! workspace root, or a path under an ignored name.** This module is the
//! only place that law is implemented; every other module receives a
//! [`WorkspacePath`] and never a raw string or self-built `PathBuf`.
//!
//! - [`UnresolvedPath`] is client input, unchecked. The only thing you can
//!   do with one is call [`UnresolvedPath::resolve`].
//! - [`WorkspacePath`] is the proof that check passed — no `From`, no
//!   public field, no way to construct one that skipped it.
//! - [`WriteBuffer`] additionally proves the path isn't the workspace's
//!   `justfile` (ADR 0003). Only [`WorkspacePath::into_write_buffer`]
//!   produces one, and it hands out no raw path — a tool that never
//!   converts to a `WriteBuffer` has no way to write anything at all.
//!
//! Don't deconstruct a `WorkspacePath`/`WriteBuffer` into its parts — a
//! loose `PathBuf` carries none of this proof. Use its accessors or
//! `Display` at the point of use instead.

use rmcp::model::ErrorData as McpError;
use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

use std::fmt::{self, Display, Formatter};
use std::fs::{self, File, Metadata, OpenOptions, ReadDir};
use std::io;
use std::path::{Component, Path, PathBuf};

/// A path exactly as a tool received it over the wire: unchecked, and
/// deliberately inert. [`resolve`](UnresolvedPath::resolve) is the only
/// door out, and it's a fallible one.
///
/// The inner field is `pub(crate)` only so tests in sibling modules can
/// build fixtures directly; production code always gets one via `serde`.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct UnresolvedPath(PathBuf); // AI: Never make the PathBuf pub!

impl UnresolvedPath {
    #[cfg(test)]
    pub fn new(unresolved: impl Into<PathBuf>) -> Self {
        Self(unresolved.into())
    }

    /// Checks this path against `workspace_root` and `ignore`. See
    /// [`WorkspacePath::new`] for what "passes" means.
    pub fn resolve(
        self,
        workspace_root: &Path,
        ignore: &[String],
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
    /// like `../b`); (3) any path component matching `ignore` is rejected
    /// the same way a nonexistent path would be — see [`not_found_or_io`]
    /// for why.
    ///
    /// Symlinks are deliberately **not** resolved or checked here: no tool
    /// in this module can create one, so the only way one can exist inside
    /// the workspace is if whoever set the workspace up put it there
    /// themselves — a decision made outside this server, not something a
    /// client can trigger through it.
    pub fn new(
        unresolved: UnresolvedPath,
        workspace_root: &Path,
        ignore: &[String],
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

        // No component may match an ignored name.
        if relative
            .components()
            .any(|c| ignore.iter().any(|name| name.as_str() == c.as_os_str()))
        {
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

    /// Fails if this path names the workspace's `justfile` (ADR 0003).
    /// There is no other way to obtain a [`WriteBuffer`], so this is not
    /// something a tool could forget to check.
    pub(super) fn into_write_buffer(self) -> Result<WriteBuffer, McpError> {
        if self.relative == Path::new("justfile") {
            return Err(McpError::invalid_params(
                format!("{self}: justfile is read-only through this server (see ADR 0003)"),
                None,
            ));
        }
        Ok(WriteBuffer(self))
    }
}

/// A [`WorkspacePath`] proven *not* to name the workspace's `justfile`.
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

    const EMPTY_IGNORE: &[String] = &[];

    // fn resolve(input: &str, root: &Path, ignore: &[String]) -> Result<WorkspacePath, McpError> {
    //     WorkspacePath::new(PathBuf::from(input), root, ignore)
    // }

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
            ignore: &[String],
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
            ignore: &[String],
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
        let ignore = vec![".git".to_string()];
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
        let ignore = vec!["target".to_string()];
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
        let ignore = vec!["target".to_string()];
        let root = TempDir::new().unwrap();
        assert_eq!(
            UnresolvedPath::resolve_workspace_with_ignore(UNRESOLVED, root.path(), &ignore),
            root.path().join(UNRESOLVED)
        );
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
    fn into_write_buffer_refuses_the_justfile() {
        let root = TempDir::new().unwrap();
        fs::write(root.path().join("justfile"), "check:\n\tcargo check\n").unwrap();
        let path = UnresolvedPath::new("justfile")
            .resolve(root.path(), EMPTY_IGNORE)
            .unwrap();
        assert_matches!(
            path.into_write_buffer().unwrap_err(),
            McpError {
                code: ErrorCode::INVALID_PARAMS,
                ..
            }
        );
    }

    #[test]
    fn into_write_buffer_allows_other_files() {
        let root = TempDir::new().unwrap();
        let path = UnresolvedPath::new("notes.md")
            .resolve(root.path(), EMPTY_IGNORE)
            .unwrap();
        let write = path.into_write_buffer().unwrap();
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
        let write = path.into_write_buffer().unwrap();
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
        let write = path.into_write_buffer().unwrap();
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
