use super::SearchMatch;
use crate::mcp::workspace_path::{IgnorePattern, WorkspacePath, not_found_or_io};

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use rmcp::model::ErrorData as McpError;

/// Recursively fuzzy-matches every line under a [`WorkspacePath`] against
/// a query, honoring the same ignore rules as `fs_tree`. Results are
/// ranked by nucleo's relevance score, best match first; lines that
/// don't match at all are dropped rather than scored zero.
pub(super) struct FuzzySearch {
    pattern: Pattern,
    matcher: Matcher,
}

impl FuzzySearch {
    pub(super) fn new(query: &str) -> Self {
        Self {
            pattern: Pattern::parse(query, CaseMatching::Smart, Normalization::Smart),
            matcher: Matcher::new(Config::DEFAULT),
        }
    }

    /// Searches `root` — a file or a directory, walked recursively — and
    /// returns every match ranked best-first by relevance score.
    pub(super) fn run(
        &mut self,
        root: &WorkspacePath,
        ignore: &[IgnorePattern],
    ) -> Result<Vec<SearchMatch>, McpError> {
        let metadata = root.metadata().map_err(|e| not_found_or_io(root, e))?;
        let mut scored = Vec::new();
        if metadata.is_dir() {
            self.walk_dir(root, 0, ignore, &mut scored)?;
        } else {
            self.search_file(root, &mut scored);
        }
        scored.sort_by(|(a, a_score), (b, b_score)| {
            b_score
                .cmp(a_score)
                .then(a.file.cmp(&b.file))
                .then(a.line.cmp(&b.line))
        });
        Ok(scored.into_iter().map(|(m, _)| m).collect())
    }

    /// Depth-first walk mirroring `WorkspaceSearch::walk_dir`'s ignore
    /// handling; kept separate for now, unified once fuzzy and exact
    /// share one walker.
    fn walk_dir(
        &mut self,
        dir: &WorkspacePath,
        depth: usize,
        ignore: &[IgnorePattern],
        scored: &mut Vec<(SearchMatch, u32)>,
    ) -> Result<(), McpError> {
        let entries = dir.read_dir().map_err(|e| not_found_or_io(dir, e))?;
        let mut entries: Vec<_> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                !ignore
                    .iter()
                    .any(|pattern| pattern.matches_name_at_depth(&name, depth))
            })
            .collect();
        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            let child = dir.child(&entry.file_name());
            if entry.path().is_dir() {
                self.walk_dir(&child, depth + 1, ignore, scored)?;
            } else {
                self.search_file(&child, scored);
            }
        }
        Ok(())
    }

    /// Scores one file's lines, appending matches to `scored`. Files
    /// that aren't valid UTF-8 (typically binary) are silently skipped.
    fn search_file(&mut self, path: &WorkspacePath, scored: &mut Vec<(SearchMatch, u32)>) {
        let Ok(content) = path.read_to_string() else {
            return;
        };
        let relative = path.relative().to_path_buf();
        let mut buf = Vec::new();
        for (index, line) in content.lines().enumerate() {
            buf.clear();
            let haystack = Utf32Str::new(line, &mut buf);
            if let Some(score) = self.pattern.score(haystack, &mut self.matcher) {
                scored.push((
                    SearchMatch {
                        file: relative.clone(),
                        line: (index + 1) as u64,
                        text: line.trim_end().to_owned(),
                    },
                    score,
                ));
            }
        }
    }
}
