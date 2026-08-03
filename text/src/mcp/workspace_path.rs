//! The one law of this server: **no tool may touch a path outside the
//! workspace root, or a path under an ignored name.** This module is the
//! only place that law is allowed to be implemented. Every other module
//! that touches `std::fs` receives a [`WorkspacePath`] and nothing else —
//! never a raw string, never a `PathBuf` it computed itself.
//!
//! Two types carry the law:
//!
//! - [`UnresolvedPath`] is what a tool's `#[derive(Deserialize)]` input
//!   struct actually contains. It wraps the raw path exactly as the client
//!   sent it — unchecked, and deliberately hard to misuse: it has no
//!   `AsRef<Path>`, no `Display` usable for a file operation, nothing that
//!   lets a tool reach the filesystem with it directly. The only thing you
//!   can do with one is call [`UnresolvedPath::resolve`].
//! - [`WorkspacePath`] is the proof that [`UnresolvedPath::resolve`] (or,
//!   equivalently, [`WorkspacePath::new`]) succeeded. There is no other
//!   way to construct one — no `From`, no public field. Holding a
//!   `WorkspacePath` *is* holding the guarantee; there's nothing left for
//!   a tool to check, and nothing for it to forget to check.
//!
//! **Do not deconstruct a `WorkspacePath` into its parts.** Something like
//! `(path.as_path().to_path_buf(), path.to_string())` hands out a plain
//! `PathBuf` and a plain `String` that no longer carry any proof of
//! anything — they're indistinguishable, at the type level, from a path
//! nobody ever checked. Keep the `WorkspacePath` itself in scope and use
//! [`WorkspacePath::as_path`] (for `std::fs` calls) or its `Display` impl
//! (for messages) at the point of use.

use rmcp::model::ErrorData as McpError;
use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

use std::fmt::{self, Display, Formatter};
use std::path::{Component, Path, PathBuf};

/// A path exactly as a tool received it over the wire: unchecked, and
/// deliberately inert. It carries no method that reaches the filesystem —
/// [`resolve`](UnresolvedPath::resolve) is the only door out, and it's a
/// fallible one.
///
/// The inner field is `pub(crate)` only so unit tests in sibling modules
/// (`view.rs`, `tree.rs`, ...) can build fixtures directly; production
/// code never has a reason to construct one by hand, since every instance
/// that matters arrives via `serde` deserializing a tool call.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct UnresolvedPath(PathBuf); // AI: Never make the PathBuf pub!

impl UnresolvedPath {
    #[cfg(test)]
    pub fn new(unresolved: impl Into<PathBuf>) -> Self {
        Self(unresolved.into())
    }

    /// Checks this path against `workspace_root` and `ignore` and, if it
    /// passes, returns the [`WorkspacePath`] that's the only way to act on
    /// it. See [`WorkspacePath::new`] for exactly what "passes" means.
    pub fn resolve(
        self,
        workspace_root: &Path,
        ignore: &[String],
    ) -> Result<WorkspacePath, McpError> {
        WorkspacePath::new(self, workspace_root, ignore)
    }

    /// Lexically collapses `.` and `..` components, without touching the
    /// filesystem — so this also works for paths that don't exist yet (e.g.
    /// for `create`). Returns `None` if a `..` has nothing left to pop: unlike
    /// a naive stack-based collapse that would just drop such a `..` silently,
    /// that case means the path tries to step above where it started, and
    /// silently dropping it would turn a workspace escape into a path that
    /// merely *looks* safe.
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
/// [`UnresolvedPath::resolve`], which just calls it). No `From`, no public
/// field, no way to build one that skipped the check.
#[derive(Debug)]
pub struct WorkspacePath {
    /// The path relative to the workspace root, after normalization —
    /// e.g. `notes/todo.md`, never `/abs/...` and never containing `..`.
    /// This is also what [`Display`] shows: the path in the vocabulary the
    /// caller used, not an absolute filesystem path they never typed.
    relative: PathBuf,
    /// `workspace_root` joined with `relative` — the absolute path that's
    /// actually safe to hand to `std::fs`. Computed once, here, and never
    /// recomputed anywhere else.
    absolute: PathBuf,
}

impl WorkspacePath {
    /// Resolves `unresolved` against `workspace_root`, which callers must
    /// already have canonicalized (the server does this once at startup —
    /// see `McpServer::serve`).
    ///
    /// The check has three parts, in order:
    ///
    /// 1. **Escape via an absolute path.** A client-supplied path is
    ///    always relative to the workspace, never to the real filesystem —
    ///    `/src/main.rs` means `<workspace>/src/main.rs`, not the real
    ///    `/src/main.rs`. A single leading `/` is stripped to express
    ///    that. Anything still absolute after that strip (a Windows drive
    ///    letter, a UNC path, ...) is rejected outright rather than
    ///    guessed at.
    /// 2. **Escape via `..`.** The path is normalized lexically — `.` and
    ///    `..` resolved without touching the filesystem, so this also
    ///    works for paths that don't exist yet (`create`). If a `..` has
    ///    nothing left to pop — the path tries to step above wherever it
    ///    started — that's a traversal attempt and it's rejected, no
    ///    matter how deep the redirection (`a/../../b` is caught exactly
    ///    like `../b`).
    /// 3. **An ignored name anywhere in the path.** If any component
    ///    matches an entry in `ignore` (`.git`, `target`, ...), the path is
    ///    rejected the same way a nonexistent path would be — see
    ///    [`not_found_or_io`] for why that's deliberate.
    ///
    /// Symlinks are deliberately **not** resolved or checked here. No tool
    /// in this module can create one — `create`/`str_replace`/`insert`
    /// only ever write regular file content — so the only way a symlink
    /// can exist inside the workspace is if whoever set the workspace up
    /// put it there themselves, before the server ever ran. That's a prior
    /// decision made outside this server, not something a client can
    /// trigger through it, so it isn't this function's job to second-guess
    /// it.
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

    /// The validated, absolute path — safe to hand to any `std::fs` call.
    pub fn as_path(&self) -> &Path {
        &self.absolute
    }
}

/// Refuses `path` if it names the workspace's `justfile`, independent of
/// the ignore list. See ADR 0003: `just_run` executes recipes from this
/// file, so nothing that can change file contents may touch it — but
/// `view`/`tree` must keep reading it, since that's how an agent learns
/// which recipes exist. Callers that write to the filesystem call this
/// right after resolving the path; `view.rs` and `tree.rs` never call it.
pub(super) fn refuse_justfile_write(path: &WorkspacePath) -> Result<(), McpError> {
    if path.relative == Path::new("justfile") {
        return Err(McpError::invalid_params(
            format!("{path}: justfile is read-only through this server (see ADR 0003)"),
            None,
        ));
    }
    Ok(())
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
                .as_path()
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
                .as_path()
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
