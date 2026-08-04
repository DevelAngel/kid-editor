use super::McpService;
use super::workspace_path::{UnresolvedPath, not_found_or_io};

use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData as McpError};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::Deserialize;

use std::io::Write;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StrReplaceInput {
    path: UnresolvedPath,
    /// Exact text to replace — must occur exactly once in the file
    old_str: String,
    /// Replacement text
    #[serde(default)]
    new_str: String,
}

#[tool_router(router = str_replace_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(description = "Replace an exact, unique occurrence of old_str with new_str in a file")]
    fn str_replace(
        &self,
        Parameters(input): Parameters<StrReplaceInput>,
    ) -> Result<CallToolResult, McpError> {
        let path = input.path.resolve(&self.workspace_root, &self.ignore)?;
        let content = path
            .read_to_string()
            .map_err(|e| not_found_or_io(&path, e))?;

        let occurrences = content.matches(input.old_str.as_str()).count();
        if occurrences == 0 {
            return Err(McpError::invalid_params(
                format!("old_str not found in {path}"),
                None,
            ));
        }
        if occurrences > 1 {
            return Err(McpError::invalid_params(
                format!(
                    "old_str occurs {occurrences} times in {path} — include more surrounding context to make it unique"
                ),
                None,
            ));
        }

        let updated = content.replacen(&input.old_str, &input.new_str, 1);
        let write = path.into_write_buffer()?;
        write
            .open()
            .and_then(|mut file| file.write_all(updated.as_bytes()))
            .map_err(|e| McpError::internal_error(format!("{write}: {e}"), None))?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "replaced 1 occurrence in {write}"
        ))]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use std::collections::BTreeMap;
    use std::fs;

    #[test]
    fn str_replace_requires_unique_match() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], BTreeMap::new());
        fs::write(dir.path().join("f.txt"), "foo\nfoo\n").unwrap();
        let result = svc.str_replace(Parameters(StrReplaceInput {
            path: UnresolvedPath::new("f.txt"),
            old_str: "foo".into(),
            new_str: "bar".into(),
        }));
        assert!(result.is_err());
    }

    #[test]
    fn str_replace_replaces_unique_match() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], BTreeMap::new());
        fs::write(dir.path().join("f.txt"), "foo\nbaz\n").unwrap();
        svc.str_replace(Parameters(StrReplaceInput {
            path: UnresolvedPath::new("f.txt"),
            old_str: "foo".into(),
            new_str: "bar".into(),
        }))
        .unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "bar\nbaz\n");
    }
}
