use super::McpService;

use anyhow::Result;
use rmcp::model::ErrorData as McpError;

use std::path::{Component, Path, PathBuf};

impl McpService {
    /// Resolves `input` (relative or absolute) against the workspace root.
    /// Rejects anything that would escape it — via `..` segments or, for
    /// paths that already exist, via a symlink pointing outside — and
    /// anything under an ignored name (see `ignore`), which is treated as
    /// nonexistent rather than merely hidden: no tool can see, read, or
    /// write it, the same as if it were outside the workspace.
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

        if let Ok(relative) = normalized.strip_prefix(&self.workspace_root)
            && relative
                .components()
                .any(|c| self.ignore.iter().any(|i| i.as_str() == c.as_os_str()))
        {
            return Err(McpError::invalid_params(
                format!("{input}: no such file or directory"),
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
        let svc = McpService::new(dir.to_path_buf(), vec![]);
        let resolved = svc.resolve("notes/todo.md").unwrap();
        assert_eq!(resolved, dir.path().join("notes/todo.md"));
    }

    #[test]
    fn resolve_absolute_path_inside_workspace_is_allowed() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![]);
        let abs = dir.path().join("file.txt");
        let resolved = svc.resolve(abs.to_str().unwrap()).unwrap();
        assert_eq!(resolved, abs);
    }

    #[test]
    fn resolve_rejects_dotdot_escape() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![]);
        assert!(svc.resolve("../outside.txt").is_err());
    }

    #[test]
    fn resolve_rejects_absolute_path_outside_workspace() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![]);
        assert!(svc.resolve("/etc/passwd").is_err());
    }

    #[test]
    fn resolve_rejects_dotdot_that_stays_lexically_inside_but_traverses_out() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![]);
        // "sub/../../escape" normalizes to a path outside the root even
        // though it starts inside it.
        assert!(svc.resolve("sub/../../escape").is_err());
    }

    #[test]
    fn resolve_rejects_ignored_top_level_entry() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![".git".to_string()]);
        assert!(svc.resolve(".git").is_err());
        assert!(svc.resolve(".git/config").is_err());
    }

    #[test]
    fn resolve_rejects_path_nested_under_ignored_directory() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec!["target".to_string()]);
        assert!(svc.resolve("target/debug/deep/file.txt").is_err());
    }

    #[test]
    fn resolve_allows_entry_not_matching_ignore_list() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec!["target".to_string()]);
        assert!(svc.resolve("src/main.rs").is_ok());
    }

    #[test]
    fn normalize_collapses_dot_and_dotdot() {
        let p = normalize_lexically(Path::new("/a/./b/../c"));
        assert_eq!(p, Path::new("/a/c"));
    }
}
