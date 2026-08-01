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
