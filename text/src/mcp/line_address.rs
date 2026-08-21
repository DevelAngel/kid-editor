//! Shared line-addressing for tools that operate on line numbers rather
//! than exact text — `fs_insert_lines`, `fs_remove_lines`,
//! `fs_replace_line`. Negative numbers count from the end of the file,
//! mirroring `head`/`tail` (`-1` = last line, `-2` = second-to-last, ...).

/// A possibly negative, 1-indexed line number as received from a client:
/// positive counts from the start (`1` = first line), negative counts
/// from the end (`-1` = last line), mirroring `head`/`tail`.
#[derive(Debug, Clone, Copy)]
pub(super) struct LineAddress(i64);

impl LineAddress {
    pub(super) fn new(line: i64) -> Self {
        Self(line)
    }

    /// Resolves this address against `total_lines`. `0` and out-of-range
    /// numbers are rejected. Requires `total_lines >= 1` — callers that
    /// must support an empty file (only `fs_insert_lines` does) handle
    /// that case themselves before calling this.
    pub(super) fn resolve(self, total_lines: usize) -> Result<usize, String> {
        debug_assert!(total_lines > 0, "caller must handle the empty file itself");
        let Self(line) = self;
        let total = total_lines as i64;
        let resolved = match line {
            0 => {
                return Err(
                    "line number 0 is not valid; use 1 for the first line or -1 for the last"
                        .into(),
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
}

/// A `start..=end` line range, both ends a possibly negative
/// [`LineAddress`].
#[derive(Debug, Clone, Copy)]
pub(super) struct LineRange {
    start: LineAddress,
    end: LineAddress,
}

impl LineRange {
    pub(super) fn new(start_line: i64, end_line: i64) -> Self {
        Self {
            start: LineAddress::new(start_line),
            end: LineAddress::new(end_line),
        }
    }

    /// Resolves both ends against `total_lines` into a 1-indexed,
    /// inclusive `(start, end)` pair with `start <= end`.
    pub(super) fn resolve(self, total_lines: usize) -> Result<(usize, usize), String> {
        let Self { start, end } = self;
        let (start_line, end_line) = (start.0, end.0);
        let start = start.resolve(total_lines)?;
        let end = end.resolve(total_lines)?;
        if start > end {
            return Err(format!(
                "start_line ({start_line}, resolves to line {start}) comes after \
                 end_line ({end_line}, resolves to line {end})"
            ));
        }
        Ok((start, end))
    }
}

/// Rejoining a line slice back into file content — an extension on
/// `[&str]` rather than a free function, so call sites read as
/// `lines.rejoin(&content)`.
pub(super) trait JoinLines {
    /// Restores a trailing newline only if `original_content` had one
    /// and `self` isn't empty (an empty file has no trailing newline to
    /// restore).
    fn rejoin(&self, original_content: &str) -> String;
}

impl JoinLines for [&str] {
    fn rejoin(&self, original_content: &str) -> String {
        if self.is_empty() {
            return String::new();
        }
        let mut joined = self.join("\n");
        if original_content.ends_with('\n') {
            joined.push('\n');
        }
        joined
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_line_number_is_unchanged() {
        assert_eq!(LineAddress::new(1).resolve(5), Ok(1));
        assert_eq!(LineAddress::new(5).resolve(5), Ok(5));
    }

    #[test]
    fn negative_line_number_counts_from_end() {
        assert_eq!(LineAddress::new(-1).resolve(5), Ok(5));
        assert_eq!(LineAddress::new(-5).resolve(5), Ok(1));
    }

    #[test]
    fn zero_line_number_is_rejected() {
        assert!(LineAddress::new(0).resolve(5).is_err());
    }

    #[test]
    fn out_of_range_positive_is_rejected() {
        assert!(LineAddress::new(6).resolve(5).is_err());
    }

    #[test]
    fn out_of_range_negative_is_rejected() {
        assert!(LineAddress::new(-6).resolve(5).is_err());
    }

    #[test]
    fn range_resolves_both_ends() {
        assert_eq!(LineRange::new(2, 4).resolve(5), Ok((2, 4)));
        assert_eq!(LineRange::new(2, -1).resolve(5), Ok((2, 5)));
    }

    #[test]
    fn inverted_range_is_rejected() {
        assert!(LineRange::new(4, 2).resolve(5).is_err());
    }

    #[test]
    fn rejoin_restores_trailing_newline_when_original_had_one() {
        assert_eq!(["a", "b"].rejoin("a\nb\n"), "a\nb\n");
        assert_eq!(["a", "b"].rejoin("a\nb"), "a\nb");
    }

    #[test]
    fn rejoin_of_empty_slice_is_empty_string() {
        let lines: [&str; 0] = [];
        assert_eq!(lines.rejoin("a\nb\n"), "");
    }
}
