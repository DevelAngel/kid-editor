use super::McpService;
use super::workspace_path::UnresolvedPath;

use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData as McpError};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::Deserialize;

use std::io::Write;

#[derive(Debug, Deserialize, JsonSchema)]
struct CreateInput {
    path: UnresolvedPath,
    /// Full file content. Rejected if the file already exists.
    file_text: String,
}

#[tool_router(router = create_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(
        description = "Create a new file with the given content. Fails if the path already exists — use fs_replace_lines, fs_insert_lines, or fs_remove_lines to edit an existing file.",
        annotations(
            title = "Create File",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    fn fs_create(
        &self,
        Parameters(input): Parameters<CreateInput>,
    ) -> Result<CallToolResult, McpError> {
        let path = input.path.resolve(&self.workspace_root, &self.ignore)?;
        if path.metadata().is_ok() {
            return Err(McpError::invalid_params(
                format!(
                    "{path}: already exists; use fs_replace_lines, fs_insert_lines, or fs_remove_lines to edit it"
                ),
                None,
            ));
        }
        let write = path.into_write_buffer(self.recipe_toml_protected_path.as_deref())?;
        write
            .open()
            .and_then(|mut file| file.write_all(input.file_text.as_bytes()))
            .map_err(|e| McpError::internal_error(format!("{write}: {e}"), None))?;
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "wrote {write}"
        ))]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use recipe::RecipeFile;
    use std::fs;

    #[test]
    fn creates_new_file() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        svc.fs_create(Parameters(CreateInput {
            path: UnresolvedPath::new("f.txt"),
            file_text: "a\nb\n".to_string(),
        }))
        .unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "a\nb\n");
    }

    #[test]
    fn refuses_to_overwrite_existing_file() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        fs::write(dir.path().join("f.txt"), "original\n").unwrap();
        let result = svc.fs_create(Parameters(CreateInput {
            path: UnresolvedPath::new("f.txt"),
            file_text: "overwritten\n".to_string(),
        }));
        assert!(result.is_err());
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "original\n");
    }
}
