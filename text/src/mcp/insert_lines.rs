use super::McpService;
use super::line_address::{JoinLines, LineAddress};
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
#[serde(rename_all = "snake_case")]
pub enum Position {
    Before,
    After,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct InsertLinesInput {
    path: UnresolvedPath,
    /// 1-indexed line number to insert relative to; negative numbers
    /// count from the end of the file (-1 = last line), like `tail`.
    /// Ignored if the file is empty.
    line: i64,
    /// Whether to insert before or after `line`
    position: Position,
    new_str: String,
}

#[tool_router(router = insert_lines_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(
        description = "Insert text before or after a given line number",
        annotations(
            title = "Insert Lines",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    fn fs_insert_lines(
        &self,
        Parameters(input): Parameters<InsertLinesInput>,
    ) -> Result<CallToolResult, McpError> {
        let path = input.path.resolve(&self.workspace_root, &self.ignore)?;
        let content = path
            .read_to_string()
            .map_err(|e| not_found_or_io(&path, e))?;

        let mut lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();
        // An empty file has exactly one place to insert into: there is
        // no earlier or later line to be before/after, so `line` and
        // `position` don't apply.
        let anchor = if total_lines == 0 {
            0
        } else {
            let resolved = LineAddress::new(input.line)
                .resolve(total_lines)
                .map_err(|msg| McpError::invalid_params(msg, None))?;
            match input.position {
                Position::Before => resolved - 1,
                Position::After => resolved,
            }
        };
        lines.insert(anchor, input.new_str.as_str());
        let updated = lines.rejoin(&content);

        let write = path.into_write_buffer(self.recipe_toml_protected_path.as_deref())?;
        write
            .open()
            .and_then(|mut file| file.write_all(updated.as_bytes()))
            .map_err(|e| McpError::internal_error(format!("{write}: {e}"), None))?;

        let new_start = anchor + 1;
        let touched = input.new_str.lines().count().max(1);
        let updated_lines: Vec<&str> = updated.lines().collect();
        let text = if updated_lines.is_empty() {
            empty_file_notice("Edited ", &write)
        } else {
            let (ctx_start, ctx_end) = context_range(new_start, touched, updated_lines.len());
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
    fn inserts_after_given_line() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        fs::write(dir.path().join("f.txt"), "a\nb\n").unwrap();
        svc.fs_insert_lines(Parameters(InsertLinesInput {
            path: UnresolvedPath::new("f.txt"),
            line: 1,
            position: Position::After,
            new_str: "x".into(),
        }))
        .unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "a\nx\nb\n");
    }

    #[test]
    fn inserts_before_given_line() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        fs::write(dir.path().join("f.txt"), "a\nb\n").unwrap();
        svc.fs_insert_lines(Parameters(InsertLinesInput {
            path: UnresolvedPath::new("f.txt"),
            line: 2,
            position: Position::Before,
            new_str: "x".into(),
        }))
        .unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "a\nx\nb\n");
    }

    #[test]
    fn negative_line_counts_from_end() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        fs::write(dir.path().join("f.txt"), "a\nb\nc\n").unwrap();
        svc.fs_insert_lines(Parameters(InsertLinesInput {
            path: UnresolvedPath::new("f.txt"),
            line: -1,
            position: Position::After,
            new_str: "x".into(),
        }))
        .unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "a\nb\nc\nx\n");
    }

    #[test]
    fn inserts_into_empty_file() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        fs::write(dir.path().join("f.txt"), "").unwrap();
        svc.fs_insert_lines(Parameters(InsertLinesInput {
            path: UnresolvedPath::new("f.txt"),
            line: 1,
            position: Position::After,
            new_str: "x".into(),
        }))
        .unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "x");
    }

    #[test]
    fn out_of_range_line_is_rejected() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        fs::write(dir.path().join("f.txt"), "a\nb\n").unwrap();
        let result = svc.fs_insert_lines(Parameters(InsertLinesInput {
            path: UnresolvedPath::new("f.txt"),
            line: 5,
            position: Position::After,
            new_str: "x".into(),
        }));
        assert!(result.is_err());
    }
}
