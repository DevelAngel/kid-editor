use super::McpService;
use super::line_address::{JoinLines, LineRange};
use super::render::{context_range, empty_file_notice, render_excerpt};
use super::workspace_path::{UnresolvedPath, not_found_or_io};

use anyhow::Result;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData as McpError};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::Deserialize;

use std::io::Write;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoveLinesInput {
    path: UnresolvedPath,
    /// 1-indexed, inclusive start of the range to remove; negative
    /// numbers count from the end of the file (-1 = last line), like
    /// `tail`
    start_line: i64,
    /// 1-indexed, inclusive end of the range to remove; same negative
    /// addressing as `start_line`
    end_line: i64,
}

#[tool_router(router = remove_lines_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(
        description = "Remove a line range (negative counts from the end, like tail)",
        annotations(
            title = "Remove Lines",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    fn fs_remove_lines(
        &self,
        Parameters(input): Parameters<RemoveLinesInput>,
    ) -> Result<CallToolResult, McpError> {
        let path = input.path.resolve(&self.workspace_root, &self.ignore)?;
        let content = path
            .read_to_string()
            .map_err(|e| not_found_or_io(&path, e))?;

        let mut lines: Vec<&str> = content.lines().collect();
        let (start, end) = LineRange::new(input.start_line, input.end_line)
            .resolve(lines.len())
            .map_err(|msg| McpError::invalid_params(msg, None))?;
        lines.drain((start - 1)..end);
        let updated = lines.rejoin(&content);

        let write = path.into_write_buffer(self.recipe_toml_protected_path.as_deref())?;
        write
            .open()
            .and_then(|mut file| file.write_all(updated.as_bytes()))
            .map_err(|e| McpError::internal_error(format!("{write}: {e}"), None))?;

        let updated_lines: Vec<&str> = updated.lines().collect();
        let text = if updated_lines.is_empty() {
            empty_file_notice("Edited ", &write)
        } else {
            let (ctx_start, ctx_end) = context_range(start, 0, updated_lines.len());
            render_excerpt("Edited ", &write, &updated_lines, ctx_start, ctx_end)
        };
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use recipe::RecipeFile;
    use std::fs;

    #[test]
    fn removes_single_line() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        fs::write(dir.path().join("f.txt"), "a\nb\nc\n").unwrap();
        svc.fs_remove_lines(Parameters(RemoveLinesInput {
            path: UnresolvedPath::new("f.txt"),
            start_line: 2,
            end_line: 2,
        }))
        .unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "a\nc\n");
    }

    #[test]
    fn removes_range() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        fs::write(dir.path().join("f.txt"), "a\nb\nc\nd\n").unwrap();
        svc.fs_remove_lines(Parameters(RemoveLinesInput {
            path: UnresolvedPath::new("f.txt"),
            start_line: 2,
            end_line: 3,
        }))
        .unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "a\nd\n");
    }

    #[test]
    fn negative_end_counts_from_end() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        fs::write(dir.path().join("f.txt"), "a\nb\nc\nd\n").unwrap();
        svc.fs_remove_lines(Parameters(RemoveLinesInput {
            path: UnresolvedPath::new("f.txt"),
            start_line: 2,
            end_line: -1,
        }))
        .unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "a\n");
    }

    #[test]
    fn removing_all_lines_leaves_empty_file() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        fs::write(dir.path().join("f.txt"), "a\nb\n").unwrap();
        svc.fs_remove_lines(Parameters(RemoveLinesInput {
            path: UnresolvedPath::new("f.txt"),
            start_line: 1,
            end_line: -1,
        }))
        .unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "");
    }

    #[test]
    fn inverted_range_is_rejected() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        fs::write(dir.path().join("f.txt"), "a\nb\nc\n").unwrap();
        let result = svc.fs_remove_lines(Parameters(RemoveLinesInput {
            path: UnresolvedPath::new("f.txt"),
            start_line: 3,
            end_line: 1,
        }));
        assert!(result.is_err());
    }

    #[test]
    fn out_of_range_is_rejected() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        fs::write(dir.path().join("f.txt"), "a\nb\n").unwrap();
        let result = svc.fs_remove_lines(Parameters(RemoveLinesInput {
            path: UnresolvedPath::new("f.txt"),
            start_line: 1,
            end_line: 5,
        }));
        assert!(result.is_err());
    }
}
