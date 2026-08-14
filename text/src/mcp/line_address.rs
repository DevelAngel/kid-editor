//! Shared line-addressing for tools that operate on line numbers rather
//! than exact text — `fs_insert_lines`, `fs_remove_lines`,
//! `fs_replace_lines`. Negative numbers count from the end of the file,
//! mirroring `head`/`tail` (`-1` = last line, `-2` = second-to-last, ...).

/// Resolves a possibly negative, 1-indexed line number against
/// `total_lines`. Positive numbers address from the start (`1` = first
/// line); negative numbers address from the end (`-1` = last line).
/// `0` and out-of-range numbers are rejected. Requires `total_lines >= 1`
/// — callers that must support an empty file (only `fs_insert_lines`
/// does) handle that case themselves before calling this.
pub(super) fn resolve_line(line: i64, total_lines: usize) -> Result<usize, String> {
    debug_assert!(total_lines > 0, "caller must handle the empty file itself");
    let total = total_lines as i64;
    let resolved = match line {
        0 => {
            return Err(
                "line number 0 is not valid; use 1 for the first line or -1 for the last".into(),
            );
        }
        l if l > 0 => l,
        l => total + l + 1,
    };
    if resolved < 1 || resolved > total {
        return Err(format!(
            "line {line} is out of range for a file with {total_lines} line(s)"
        ));
    }
    Ok(resolved as usize)
}

/// Resolves a `start_line..=end_line` range (both possibly negative, see
/// [`resolve_line`]) into a 1-indexed, inclusive `(start, end)` pair with
/// `start <= end`.
pub(super) fn resolve_range(
    start_line: i64,
    end_line: i64,
    total_lines: usize,
) -> Result<(usize, usize), String> {
    let start = resolve_line(start_line, total_lines)?;
    let end = resolve_line(end_line, total_lines)?;
    if start > end {
        return Err(format!(
            "start_line ({start_line}, resolves to line {start}) comes after \
             end_line ({end_line}, resolves to line {end})"
        ));
    }
    Ok((start, end))
}

/// Joins `lines` back into file content, restoring a trailing newline
/// only if `original_content` had one and `lines` isn't empty (an empty
/// file has no trailing newline to restore).
pub(super) fn join_lines(lines: &[&str], original_content: &str) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let mut joined = lines.join("\n");
    if original_content.ends_with('\n') {
        joined.push('\n');
    }
    joined
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_line_number_is_unchanged() {
        assert_eq!(resolve_line(1, 5), Ok(1));
        assert_eq!(resolve_line(5, 5), Ok(5));
    }

    #[test]
    fn negative_line_number_counts_from_end() {
        assert_eq!(resolve_line(-1, 5), Ok(5));
        assert_eq!(resolve_line(-5, 5), Ok(1));
    }

    #[test]
    fn zero_line_number_is_rejected() {
        assert!(resolve_line(0, 5).is_err());
    }

    #[test]
    fn out_of_range_positive_is_rejected() {
        assert!(resolve_line(6, 5).is_err());
    }

    #[test]
    fn out_of_range_negative_is_rejected() {
        assert!(resolve_line(-6, 5).is_err());
    }

    #[test]
    fn range_resolves_both_ends() {
        assert_eq!(resolve_range(2, 4, 5), Ok((2, 4)));
        assert_eq!(resolve_range(2, -1, 5), Ok((2, 5)));
    }

    #[test]
    fn inverted_range_is_rejected() {
        assert!(resolve_range(4, 2, 5).is_err());
    }

    #[test]
    fn join_lines_restores_trailing_newline_when_original_had_one() {
        assert_eq!(join_lines(&["a", "b"], "a\nb\n"), "a\nb\n");
        assert_eq!(join_lines(&["a", "b"], "a\nb"), "a\nb");
    }

    #[test]
    fn join_lines_of_empty_slice_is_empty_string() {
        assert_eq!(join_lines(&[], "a\nb\n"), "");
    }
}
