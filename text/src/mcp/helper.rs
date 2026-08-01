use super::McpService;

use anyhow::Result;
use rmcp::model::ErrorData as McpError;

use std::path::{Component, Path, PathBuf};

impl McpService {
    /// Resolves `input` (relative or absolute) against the workspace root
    /// and rejects anything that would escape it — via `..` segments or,
    /// for paths that already exist, via a symlink pointing outside.
    pub(super) fn resolve(&self, input: &str) -> Result<PathBuf, McpError> {
        let candidate = Path::new(input);
        let joined = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            self.workspace_root.join(candidate)
        };
        let normalized = normalize_lexically(&joined);

        if !normalized.starts_with(&self.workspace_root) {
            return Err(McpError::invalid_params(
                format!("path escapes the workspace: {input}"),
                None,
            ));
        }

        // If the path already exists, canonicalize (resolving symlinks) and
        // re-check — a symlink inside the workspace can still point outside it.
        if let Ok(canonical) = normalized.canonicalize()
            && !canonical.starts_with(&self.workspace_root)
        {
            return Err(McpError::invalid_params(
                format!("path escapes the workspace: {input}"),
                None,
            ));
        }

        Ok(normalized)
    }
}

/// Normalizes `.` and `..` components without touching the filesystem, so
/// this also works for paths that don't exist yet (e.g. for `create`).
fn normalize_lexically(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use std::path::Path;

    #[test]
    fn resolve_relative_path_stays_in_workspace() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf());
        let resolved = svc.resolve("notes/todo.md").unwrap();
        assert_eq!(resolved, dir.path().join("notes/todo.md"));
    }

    #[test]
    fn resolve_absolute_path_inside_workspace_is_allowed() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf());
        let abs = dir.path().join("file.txt");
        let resolved = svc.resolve(abs.to_str().unwrap()).unwrap();
        assert_eq!(resolved, abs);
    }

    #[test]
    fn resolve_rejects_dotdot_escape() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf());
        assert!(svc.resolve("../outside.txt").is_err());
    }

    #[test]
    fn resolve_rejects_absolute_path_outside_workspace() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf());
        assert!(svc.resolve("/etc/passwd").is_err());
    }

    #[test]
    fn resolve_rejects_dotdot_that_stays_lexically_inside_but_traverses_out() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf());
        // "sub/../../escape" normalizes to a path outside the root even
        // though it starts inside it.
        assert!(svc.resolve("sub/../../escape").is_err());
    }

    #[test]
    fn normalize_collapses_dot_and_dotdot() {
        let p = normalize_lexically(Path::new("/a/./b/../c"));
        assert_eq!(p, Path::new("/a/c"));
    }
}
