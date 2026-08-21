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
pub struct ReplaceLinesInput {
    path: UnresolvedPath,
    /// 1-indexed, inclusive start of the range to replace; negative
    /// numbers count from the end of the file (-1 = last line), like
    /// `tail`
    start_line: i64,
    /// 1-indexed, inclusive end of the range to replace; same negative
    /// addressing as `start_line`
    end_line: i64,
    /// Text replacing the whole line range
    #[serde(default)]
    new_str: String,
}

#[tool_router(router = replace_lines_tool_router, vis = "pub(super)")]
impl McpService {
    #[tool(
        description = "Replace a line range with new text (negative counts from the end, like tail)",
        annotations(
            title = "Replace Lines",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    fn fs_replace_lines(
        &self,
        Parameters(input): Parameters<ReplaceLinesInput>,
    ) -> Result<CallToolResult, McpError> {
        let path = input.path.resolve(&self.workspace_root, &self.ignore)?;
        let content = path
            .read_to_string()
            .map_err(|e| not_found_or_io(&path, e))?;

        let mut lines: Vec<&str> = content.lines().collect();
        let (start, end) = LineRange::new(input.start_line, input.end_line)
            .resolve(lines.len())
            .map_err(|msg| McpError::invalid_params(msg, None))?;
        // A single element, not `new_str.lines()`: `new_str` may itself
        // contain raw newlines, which `join` preserves verbatim either
        // way — splitting it first would only cost an allocation. An
        // empty `new_str` splices in zero elements, not one empty line,
        // so replacing with nothing behaves like removing the range.
        let replacement: &[&str] = if input.new_str.is_empty() {
            &[]
        } else {
            &[input.new_str.as_str()]
        };
        lines.splice((start - 1)..end, replacement.iter().copied());
        let updated = lines.rejoin(&content);

        let write = path.into_write_buffer(self.recipe_toml_protected_path.as_deref())?;
        write
            .open()
            .and_then(|mut file| file.write_all(updated.as_bytes()))
            .map_err(|e| McpError::internal_error(format!("{write}: {e}"), None))?;

        let touched = input.new_str.lines().count();
        let updated_lines: Vec<&str> = updated.lines().collect();
        let text = if updated_lines.is_empty() {
            empty_file_notice("Edited ", &write)
        } else {
            let (ctx_start, ctx_end) = context_range(start, touched, updated_lines.len());
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
    fn replaces_single_line() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        fs::write(dir.path().join("f.txt"), "a\nb\nc\n").unwrap();
        svc.fs_replace_lines(Parameters(ReplaceLinesInput {
            path: UnresolvedPath::new("f.txt"),
            start_line: 2,
            end_line: 2,
            new_str: "x".into(),
        }))
        .unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "a\nx\nc\n");
    }

    #[test]
    fn replaces_range_with_multiline_text() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        fs::write(dir.path().join("f.txt"), "a\nb\nc\nd\n").unwrap();
        svc.fs_replace_lines(Parameters(ReplaceLinesInput {
            path: UnresolvedPath::new("f.txt"),
            start_line: 2,
            end_line: 3,
            new_str: "x\ny".into(),
        }))
        .unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "a\nx\ny\nd\n");
    }

    #[test]
    fn negative_line_counts_from_end() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        fs::write(dir.path().join("f.txt"), "a\nb\nc\n").unwrap();
        svc.fs_replace_lines(Parameters(ReplaceLinesInput {
            path: UnresolvedPath::new("f.txt"),
            start_line: -1,
            end_line: -1,
            new_str: "x".into(),
        }))
        .unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "a\nb\nx\n");
    }

    #[test]
    fn empty_new_str_removes_the_range() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        fs::write(dir.path().join("f.txt"), "a\nb\nc\n").unwrap();
        svc.fs_replace_lines(Parameters(ReplaceLinesInput {
            path: UnresolvedPath::new("f.txt"),
            start_line: 2,
            end_line: 2,
            new_str: String::new(),
        }))
        .unwrap();
        let content = fs::read_to_string(dir.path().join("f.txt")).unwrap();
        assert_eq!(content, "a\nc\n");
    }

    #[test]
    fn inverted_range_is_rejected() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        fs::write(dir.path().join("f.txt"), "a\nb\nc\n").unwrap();
        let result = svc.fs_replace_lines(Parameters(ReplaceLinesInput {
            path: UnresolvedPath::new("f.txt"),
            start_line: 3,
            end_line: 1,
            new_str: "x".into(),
        }));
        assert!(result.is_err());
    }

    #[test]
    fn out_of_range_is_rejected() {
        let dir = TempDir::new().unwrap();
        let svc = McpService::new(dir.to_path_buf(), vec![], RecipeFile::default(), None);
        fs::write(dir.path().join("f.txt"), "a\nb\n").unwrap();
        let result = svc.fs_replace_lines(Parameters(ReplaceLinesInput {
            path: UnresolvedPath::new("f.txt"),
            start_line: 1,
            end_line: 5,
            new_str: "x".into(),
        }));
        assert!(result.is_err());
    }
}
