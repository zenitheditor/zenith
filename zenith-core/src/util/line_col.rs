//! Byte offset to 1-based line and column.

/// Return the 1-based `(line, column)` of byte `offset` in `text`.
///
/// The column counts characters, not bytes, so it matches an editor column.
/// Returns `None` when `offset` is past the end of `text`. An offset equal to
/// `text.len()` is valid and points just after the last character.
pub fn line_col(text: &str, offset: usize) -> Option<(usize, usize)> {
    let before = text.as_bytes().get(..offset)?;
    let line_start = before
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |i| i + 1);
    let line = before.iter().filter(|&&b| b == b'\n').count() + 1;
    let col = before.get(line_start..).map_or(0, |tail| {
        tail.iter().filter(|&&b| (b & 0xC0) != 0x80).count()
    }) + 1;
    Some((line, col))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_byte_is_line_one_col_one() {
        assert_eq!(line_col("abc", 0), Some((1, 1)));
        assert_eq!(line_col("hello world", 5), Some((1, 6)));
    }

    #[test]
    fn second_line_starts_at_column_one() {
        assert_eq!(line_col("foo\nbar", 4), Some((2, 1)));
        assert_eq!(line_col("foo\nbar", 6), Some((2, 3)));
    }

    #[test]
    fn counts_lines_and_chars() {
        let src = "ab\ncdé\nxy";
        assert_eq!(line_col(src, 3), Some((2, 1)));
        // `é` is two bytes; `\n` after it is the 4th char of line 2.
        assert_eq!(line_col(src, 7), Some((2, 4)));
        assert_eq!(line_col(src, 8), Some((3, 1)));
    }

    #[test]
    fn column_is_chars_not_bytes() {
        // `é` is 2 bytes and `日` is 3 bytes, so byte offset 6 is char column 4.
        let src = "é日x!";
        assert_eq!(line_col(src, 6), Some((1, 4)));
        assert_eq!(src.len(), 7);
        assert_eq!(line_col(src, 7), Some((1, 5)));
    }

    #[test]
    fn end_of_source_is_valid_and_past_end_is_none() {
        assert_eq!(line_col("ab", 2), Some((1, 3)));
        assert_eq!(line_col("ab", 3), None);
        assert_eq!(line_col("ab", 999), None);
    }

    #[test]
    fn empty_string() {
        assert_eq!(line_col("", 0), Some((1, 1)));
        assert_eq!(line_col("", 5), None);
    }
}
