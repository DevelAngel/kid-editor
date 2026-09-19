use super::super::workspace_path::UnresolvedPath;
use super::McpService;
use super::shared::{command_result, resolve_cwd};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ErrorData as McpError};
use rmcp::schemars::JsonSchema;
use rmcp::{tool, tool_router};
use serde::Deserialize;
use std::mem;
use thiserror::Error;

#[derive(Debug, Deserialize, JsonSchema)]
enum CommitType {
    #[serde(rename = "build")]
    Build,
    #[serde(rename = "chore")]
    Chore,
    #[serde(rename = "ci")]
    Ci,
    #[serde(rename = "docs")]
    Docs,
    #[serde(rename = "feat")]
    Feat,
    #[serde(rename = "fix")]
    Fix,
    #[serde(rename = "refactor")]
    Refactor,
    #[serde(rename = "style")]
    Style,
    #[serde(rename = "test")]
    Test,
}

impl CommitType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Chore => "chore",
            Self::Ci => "ci",
            Self::Docs => "docs",
            Self::Feat => "feat",
            Self::Fix => "fix",
            Self::Refactor => "refactor",
            Self::Style => "style",
            Self::Test => "test",
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct Input {
    /// Conventional Commit type.
    #[serde(rename = "type")]
    commit_type: CommitType,
    /// Optional scope for the commit.
    #[serde(default)]
    scope: Option<String>,
    /// Commit description.
    description: String,
    /// Commit body paragraphs, each word-wrapped independently.
    body: Vec<String>,
    /// Optional breaking-change note.
    #[serde(default)]
    breaking_change_note: Option<String>,
    /// Amend the previous commit instead of creating a new one.
    #[serde(default)]
    amend: bool,
    /// Optional workspace-relative working directory.
    #[serde(default)]
    cwd: Option<UnresolvedPath>,
}

#[derive(Debug, Error)]
enum CommitMessageError {
    #[error("commit body must not be empty")]
    EmptyBody,
    #[error("commit body must not contain BREAKING CHANGE")]
    BreakingChangeInBody,
    #[error("commit summary is {0} characters long and longer than 50 characters")]
    LongSummary(usize),
    #[error(
        "commit body wraps to {lines} lines ({words} words across {paragraphs} paragraph(s)) and is longer than 12 lines — shorten the paragraphs"
    )]
    TooManyBodyLines {
        lines: usize,
        words: usize,
        paragraphs: usize,
    },
    #[error(
        "breaking change note wraps to {lines} lines ({words} words) and is longer than 4 lines — shorten the note"
    )]
    TooManyBreakingChangeLines { lines: usize, words: usize },
}

fn build_commit_message(params: &Input) -> Result<String, Vec<CommitMessageError>> {
    let mut errors = Vec::new();

    if params
        .body
        .iter()
        .all(|paragraph| paragraph.trim().is_empty())
    {
        errors.push(CommitMessageError::EmptyBody);
    }
    if params
        .body
        .iter()
        .any(|paragraph| paragraph.contains("BREAKING CHANGE"))
    {
        errors.push(CommitMessageError::BreakingChangeInBody);
    }

    let body_lines = wrap_body_lines(&params.body);
    if body_lines.len() > 12 {
        errors.push(CommitMessageError::TooManyBodyLines {
            lines: body_lines.len(),
            words: word_count(&params.body),
            paragraphs: params.body.len(),
        });
    }

    let breaking_change_lines = params
        .breaking_change_note
        .as_deref()
        .map(wrap_breaking_change_note_lines);
    if let Some(lines) = &breaking_change_lines
        && lines.len() > 4
    {
        errors.push(CommitMessageError::TooManyBreakingChangeLines {
            lines: lines.len(),
            words: params
                .breaking_change_note
                .as_deref()
                .map(|note| note.split_whitespace().count())
                .unwrap_or_default(),
        });
    }

    let description = lowercase_first_char(&params.description);
    let scope = params
        .scope
        .as_deref()
        .map(|scope| format!("({scope})"))
        .unwrap_or_default();
    let breaking = if params.breaking_change_note.is_some() {
        "!"
    } else {
        ""
    };
    let commit_type = params.commit_type.as_str();
    let summary = format!("{commit_type}{scope}{breaking}: {description}");

    if summary.chars().count() > 50 {
        errors.push(CommitMessageError::LongSummary(summary.chars().count()));
    }

    if !errors.is_empty() {
        return Err(errors);
    }
    let body = body_lines.join("\n");

    let mut message = format!("{summary}\n\n{body}");
    if let Some(lines) = &breaking_change_lines {
        message.push_str("\n\nBREAKING CHANGE: ");
        message.push_str(&lines.join("\n    "));
    }
    Ok(message)
}

fn word_count(paragraphs: &[String]) -> usize {
    paragraphs
        .iter()
        .map(|paragraph| paragraph.split_whitespace().count())
        .sum()
}

fn wrap_body_lines(paragraphs: &[String]) -> Vec<String> {
    let mut lines = Vec::new();
    for (index, paragraph) in paragraphs.iter().enumerate() {
        if index > 0 {
            lines.push(String::new());
        }
        lines.extend(wrap_paragraph_lines(paragraph, 72, 72));
    }
    lines
}

fn wrap_breaking_change_note_lines(note: &str) -> Vec<String> {
    wrap_paragraph_lines(note, 54, 68)
}

fn wrap_paragraph_lines(text: &str, first_line_max: usize, rest_max: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();

    for word in text.split_whitespace() {
        let max_width = if lines.is_empty() {
            first_line_max
        } else {
            rest_max
        };
        if line.is_empty() {
            line.push_str(word);
        } else if line.chars().count() + 1 + word.chars().count() <= max_width {
            line.push(' ');
            line.push_str(word);
        } else {
            lines.push(mem::take(&mut line));
            line.push_str(word);
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }

    lines
}

fn lowercase_first_char(value: &str) -> String {
    let Some(first) = value.chars().next() else {
        return String::new();
    };
    let Some(word) = value.split_whitespace().next() else {
        return value.to_owned();
    };

    if word.chars().skip(1).any(char::is_uppercase) {
        return value.to_owned();
    }

    let lower = first.to_lowercase().to_string();
    if lower == first.to_string() {
        return value.to_owned();
    }

    format!("{lower}{remaining}", remaining = &value[lower.len()..])
}

#[tool_router(router = git_commit_tool_router, vis = "pub(crate)")]
impl McpService {
    #[tool(
        description = "Creates a Git commit with the given message, optionally amends the previous commit",
        annotations(
            title = "Git Commit",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn git_commit(
        &self,
        Parameters(input): Parameters<Input>,
    ) -> Result<CallToolResult, McpError> {
        let commit_message = match build_commit_message(&input) {
            Ok(message) => message,
            Err(errors) => {
                let text = errors
                    .into_iter()
                    .map(|error| format!("- {error}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                return Ok(CallToolResult::error(vec![ContentBlock::text(text)]));
            }
        };

        let cwd = resolve_cwd(input.cwd, &self.workspace_root, &self.ignore)?;
        if input.amend {
            command_result(
                "git",
                &["commit", "--amend", "-m", &commit_message],
                "git commit (amend)",
                cwd.absolute(),
            )
            .await
        } else {
            command_result(
                "git",
                &["commit", "-m", &commit_message],
                "git commit",
                cwd.absolute(),
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::git::{git, service};
    use assert_fs::TempDir;
    use std::path::Path;
    use std::process::Command;

    #[tokio::test]
    async fn git_commit_creates_a_commit() {
        let dir = TempDir::new().unwrap();
        git(dir.path(), &["init", "--quiet"]);
        std::fs::write(dir.path().join("file.txt"), "content\n").unwrap();
        git(dir.path(), &["add", "file.txt"]);
        service(dir.path())
            .git_commit(Parameters(Input {
                commit_type: CommitType::Fix,
                scope: None,
                description: "Add Git tools".to_owned(),
                body: vec!["Expose Git operations through the editor MCP.".to_owned()],
                breaking_change_note: None,
                amend: false,
                cwd: None,
            }))
            .await
            .unwrap();
        let output = Command::new("git")
            .args(["log", "-1", "--pretty=%s"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "fix: add Git tools"
        );
    }

    #[test]
    fn build_commit_message_reports_multiple_violations() {
        let params = Input {
            commit_type: CommitType::Fix,
            scope: None,
            description: "A very long description that makes the summary too long".to_owned(),
            body: vec!["BREAKING CHANGE".to_owned()],
            breaking_change_note: None,
            amend: false,
            cwd: None,
        };

        let errors = build_commit_message(&params).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.to_string().contains("summary"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.to_string().contains("BREAKING CHANGE"))
        );
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn build_commit_message_normalizes_description_and_breaking_change() {
        let params = Input {
            commit_type: CommitType::Feat,
            scope: Some("session".to_owned()),
            description: "Improve commit handling".to_owned(),
            body: vec!["Handle commit messages centrally.".to_owned()],
            breaking_change_note: Some("The commit input is now structured.".to_owned()),
            amend: false,
            cwd: None,
        };
        assert_eq!(
            build_commit_message(&params).unwrap(),
            "feat(session)!: improve commit handling\n\nHandle commit messages centrally.\n\nBREAKING CHANGE: The commit input is now structured."
        );
    }

    #[test]
    fn lowercase_first_char_preserves_acronyms() {
        assert_eq!(lowercase_first_char("Add"), "add");
        assert_eq!(lowercase_first_char("XML parser"), "XML parser");
    }

    #[allow(dead_code)]
    fn _path_type_is_available(path: &Path) -> &Path {
        path
    }
}
