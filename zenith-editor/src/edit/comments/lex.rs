//! The comments of a KDL text: `//` line comments, `/* … */` block
//! comments (nested), and `/-` slashdashed nodes, entries, and child
//! blocks.
//!
//! The scan skips quoted, multi-line, and raw strings, so comment markers
//! inside them do not count. KDL identifiers cannot hold `/`, so every
//! `/` outside a string starts a comment. The scan works on bytes: every
//! byte it compares is ASCII, so it never splits a UTF-8 character.

/// One comment: its byte range in the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Comment {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

/// Every comment of `text`, in source order. A comment nested in a
/// slashdashed element is part of that element, not a comment of its own.
/// Malformed text (an unclosed string or comment) runs to the end.
pub(crate) fn comments(text: &str) -> Vec<Comment> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    // `true` where a node can start: the text start, or after a line
    // break, `;`, `{`, or `}` with only whitespace and comments since.
    let mut node_start = true;
    while let Some(&b) = bytes.get(i) {
        match b {
            b'"' | b'#' => {
                i = skip_string(bytes, i);
                node_start = false;
            }
            b'/' => match bytes.get(i + 1) {
                Some(b'/') => {
                    let end = line_comment_end(bytes, i);
                    out.push(Comment { start: i, end });
                    i = end;
                }
                Some(b'*') => {
                    let end = block_comment_end(bytes, i);
                    out.push(Comment { start: i, end });
                    i = end;
                }
                Some(b'-') => {
                    let end = slashdash_end(bytes, i, node_start);
                    out.push(Comment { start: i, end });
                    i = end;
                }
                Some(_) | None => {
                    i += 1;
                    node_start = false;
                }
            },
            b'\n' | b';' | b'{' | b'}' => {
                i += 1;
                node_start = true;
            }
            b' ' | b'\t' | b'\r' => i += 1,
            _ => {
                i += 1;
                node_start = false;
            }
        }
    }
    out
}

/// One past a `//` comment: the line break stays outside.
fn line_comment_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while let Some(&b) = bytes.get(i) {
        if b == b'\n' {
            return i;
        }
        i += 1;
    }
    bytes.len()
}

/// One past the `*/` that closes the block comment at `start`, counting
/// nested `/* … */`.
fn block_comment_end(bytes: &[u8], start: usize) -> usize {
    let mut depth = 0usize;
    let mut i = start;
    while let Some(&b) = bytes.get(i) {
        let next = bytes.get(i + 1).copied();
        if b == b'/' && next == Some(b'*') {
            depth += 1;
            i += 2;
        } else if b == b'*' && next == Some(b'/') {
            depth = depth.saturating_sub(1);
            i += 2;
            if depth == 0 {
                return i;
            }
        } else {
            i += 1;
        }
    }
    bytes.len()
}

/// One past the string at `start` (a `"`, or the `#` of a raw string or a
/// keyword such as `#true`).
fn skip_string(bytes: &[u8], start: usize) -> usize {
    let hashes = bytes
        .get(start..)
        .map_or(0, |rest| rest.iter().take_while(|&&b| b == b'#').count());
    let quote = start + hashes;
    if bytes.get(quote) != Some(&b'"') {
        // A keyword: `#true`, `#null`, `#inf`, …
        return quote.max(start + 1);
    }
    let triple = bytes.get(quote..quote + 3) == Some(b"\"\"\"".as_slice());
    let open = if triple { 3 } else { 1 };
    let mut i = quote + open;
    while let Some(&b) = bytes.get(i) {
        if hashes == 0 && b == b'\\' {
            i += 2;
            continue;
        }
        if b == b'"' {
            let quotes_ok = !triple || bytes.get(i..i + 3) == Some(b"\"\"\"".as_slice());
            let close = i + open;
            let hashes_ok = bytes
                .get(close..close + hashes)
                .is_some_and(|h| h.iter().all(|&b| b == b'#'));
            if quotes_ok && hashes_ok {
                return close + hashes;
            }
        }
        i += 1;
    }
    bytes.len()
}

/// One past the element the `/-` at `start` comments out: a child block,
/// a whole node (when `node_start`), or one entry.
fn slashdash_end(bytes: &[u8], start: usize, node_start: bool) -> usize {
    let mut i = start + 2;
    while let Some(&b) = bytes.get(i) {
        match b {
            b' ' | b'\t' | b'\r' | b'\n' => i += 1,
            _ => break,
        }
    }
    match bytes.get(i) {
        Some(b'{') => block_end(bytes, i),
        Some(_) if node_start => node_end(bytes, i),
        Some(_) => entry_end(bytes, i),
        None => bytes.len(),
    }
}

/// One past the `}` that closes the `{` at `start`.
fn block_end(bytes: &[u8], start: usize) -> usize {
    let mut depth = 0usize;
    let mut i = start;
    while let Some(&b) = bytes.get(i) {
        match b {
            b'"' | b'#' => {
                i = skip_string(bytes, i);
                continue;
            }
            b'/' if matches!(bytes.get(i + 1), Some(b'/')) => {
                i = line_comment_end(bytes, i);
                continue;
            }
            b'/' if matches!(bytes.get(i + 1), Some(b'*')) => {
                i = block_comment_end(bytes, i);
                continue;
            }
            b'{' => depth += 1,
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    bytes.len()
}

/// One past the node at `start`: its line, its child block, or up to a
/// `;` or the `}` of its parent.
fn node_end(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while let Some(&b) = bytes.get(i) {
        match b {
            b'"' | b'#' => i = skip_string(bytes, i),
            b'/' if matches!(bytes.get(i + 1), Some(b'/')) => return trim_end(bytes, start, i),
            b'/' if matches!(bytes.get(i + 1), Some(b'*')) => i = block_comment_end(bytes, i),
            b'{' => return block_end(bytes, i),
            b'\n' | b';' | b'}' => return trim_end(bytes, start, i),
            _ => i += 1,
        }
    }
    trim_end(bytes, start, bytes.len())
}

/// One past the entry at `start`: up to whitespace, `;`, or a brace,
/// outside strings and type parentheses.
fn entry_end(bytes: &[u8], start: usize) -> usize {
    let mut depth = 0usize;
    let mut i = start;
    while let Some(&b) = bytes.get(i) {
        match b {
            b'"' | b'#' => {
                i = skip_string(bytes, i);
                continue;
            }
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b' ' | b'\t' | b'\r' | b'\n' | b';' | b'{' | b'}' if depth == 0 => return i,
            _ => {}
        }
        i += 1;
    }
    bytes.len()
}

/// `end`, moved back over trailing spaces to no earlier than `start`.
fn trim_end(bytes: &[u8], start: usize, end: usize) -> usize {
    let mut e = end;
    while e > start && matches!(bytes.get(e - 1), Some(b' ' | b'\t' | b'\r')) {
        e -= 1;
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(src: &str) -> Vec<&str> {
        comments(src)
            .into_iter()
            .filter_map(|c| src.get(c.start..c.end))
            .collect()
    }

    #[test]
    fn finds_every_comment_kind() {
        let src = "// head\nrect x= /* unit */ (px)10 // note\n/-rect id=\"gone\" {\n  a\n}\nb /-y=1 z=2\nc /-{\n  d\n}\n/* a /* nested */ b */ e\n";
        assert_eq!(
            texts(src),
            vec![
                "// head",
                "/* unit */",
                "// note",
                "/-rect id=\"gone\" {\n  a\n}",
                "/-y=1",
                "/-{\n  d\n}",
                "/* a /* nested */ b */",
            ]
        );
    }

    #[test]
    fn skips_markers_inside_strings() {
        let src = "a \"// no\" #\"/* raw \"# \"\"\"\n/- multi\n\"\"\" b=\"\\\"//\" // yes\n";
        assert_eq!(texts(src), vec!["// yes"]);
        assert_eq!(texts("a #true // k"), vec!["// k"]);
    }

    #[test]
    fn a_slashdashed_node_ends_at_its_line_or_semicolon() {
        assert_eq!(texts("/-a b=1 // c\nd"), vec!["/-a b=1", "// c"]);
        assert_eq!(texts("x { /-a; b }"), vec!["/-a"]);
        assert_eq!(texts("x { /-a }"), vec!["/-a"]);
        assert_eq!(texts("/- \"unclosed"), vec!["/- \"unclosed"]);
    }
}
