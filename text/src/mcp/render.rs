//! Shared line-numbered rendering, used by `fs_view` and every
//! line-editing tool so both read identically: a header naming the path
//! and line range, followed by numbered lines. Editing tools render
//! this against the file's content *after* the edit, with surrounding
//! context, so the caller can confirm the exact result without a
//! follow-up `fs_view` call — and, in particular, can see whether it
//! selected one line too many or too few.

use std::fmt::Display;

/// Lines of context shown before/after an edit's own range.
pub(super) const EDIT_CONTEXT: usize = 3;

/// Header + numbered `lines[start-1..end]` (1-indexed, inclusive).
/// `header_prefix` is prepended to the path — `"Edited "` for an edit
/// tool, empty for a plain view. Panics if `lines` is empty or the
/// range is invalid; callers must handle an empty file separately (see
/// [`empty_file_notice`]).
pub(super) fn render_excerpt(
    header_prefix: &str,
    path: impl Display,
    lines: &[&str],
    start: usize,
    end: usize,
) -> String {
    let mut out = format!("{header_prefix}{path}, lines {start}-{end}:\n");
    for (i, line) in lines[start - 1..end].iter().enumerate() {
        out.push_str(&format!("{:6}\t{}\n", start + i, line));
    }
    out
}

/// The message shown in place of [`render_excerpt`] when the file has
/// no lines left to show (e.g. after removing everything, or a freshly
/// created empty file).
pub(super) fn empty_file_notice(header_prefix: &str, path: impl Display) -> String {
    format!("{header_prefix}{path}: file is empty\n")
}

/// Expands `[start, start + touched.max(1) - 1]` by [`EDIT_CONTEXT`]
/// lines on each side, clamped to `[1, total_lines]`. `touched` is how
/// many lines the edit's new content occupies starting at `start` (`0`
/// for a pure removal — the line now sitting at `start` is shown
/// instead, for orientation). `total_lines` must be `>= 1`; callers
/// handle the empty-file case via [`empty_file_notice`] before this.
pub(super) fn context_range(start: usize, touched: usize, total_lines: usize) -> (usize, usize) {
    debug_assert!(total_lines > 0, "caller must handle the empty file itself");
    let end = start + touched.max(1) - 1;
    let start = start.saturating_sub(EDIT_CONTEXT).max(1);
    let end = (end + EDIT_CONTEXT).min(total_lines);
    (start, end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_header_and_numbered_lines() {
        let lines = ["a", "b", "c"];
        assert_eq!(
            render_excerpt("", "f.txt", &lines, 2, 3),
            "f.txt, lines 2-3:\n     2\tb\n     3\tc\n"
        );
    }

    #[test]
    fn edit_header_prefix_is_prepended() {
        let lines = ["a"];
        assert_eq!(
            render_excerpt("Edited ", "f.txt", &lines, 1, 1),
            "Edited f.txt, lines 1-1:\n     1\ta\n"
        );
    }

    #[test]
    fn context_range_expands_and_clamps_at_start() {
        assert_eq!(context_range(1, 1, 10), (1, 4));
    }

    #[test]
    fn context_range_expands_and_clamps_at_end() {
        assert_eq!(context_range(10, 1, 10), (7, 10));
    }

    #[test]
    fn context_range_covers_multiline_replacement() {
        assert_eq!(context_range(5, 3, 20), (2, 10));
    }

    #[test]
    fn context_range_of_pure_removal_shows_the_line_now_at_start() {
        assert_eq!(context_range(5, 0, 20), (2, 8));
    }
}
