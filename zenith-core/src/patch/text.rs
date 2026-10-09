//! Byte-range edits over the source text, plus the line-layout reads they need.

use super::error::{PatchError, PatchErrorCode};

/// The indentation step of the canonical formatter.
const CANON_UNIT: &str = "  ";

/// One replacement of `start..end` in the source by `text`. An insertion has
/// `start == end`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Edit {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) text: String,
}

impl Edit {
    pub(super) fn replace(start: usize, end: usize, text: impl Into<String>) -> Self {
        Self {
            start,
            end,
            text: text.into(),
        }
    }

    pub(super) fn insert(at: usize, text: impl Into<String>) -> Self {
        Self::replace(at, at, text)
    }
}

/// Apply `edits` to `src`. Insertions at one offset keep their creation order.
/// Overlapping edits are an error.
pub(super) fn apply_edits(src: &str, mut edits: Vec<Edit>) -> Result<String, PatchError> {
    // Stable sort: equal keys keep creation order.
    edits.sort_by_key(|e| (e.start, e.end));
    let mut out = String::with_capacity(src.len());
    let mut cursor = 0;
    for edit in &edits {
        if edit.start < cursor || edit.end < edit.start {
            return Err(layout_error(format!(
                "edits overlap at byte {}",
                edit.start
            )));
        }
        out.push_str(slice(src, cursor, edit.start)?);
        out.push_str(&edit.text);
        cursor = edit.end;
    }
    out.push_str(slice(src, cursor, src.len())?);
    Ok(out)
}

/// `src[start..end]`, or an error when the range is not on char boundaries.
pub(super) fn slice(src: &str, start: usize, end: usize) -> Result<&str, PatchError> {
    src.get(start..end).ok_or_else(|| {
        PatchError::new(
            PatchErrorCode::UnalignedSource,
            format!("byte range {start}..{end} is outside the text"),
        )
    })
}

pub(super) fn layout_error(message: impl Into<String>) -> PatchError {
    PatchError::new(PatchErrorCode::UnsupportedLayout, message)
}

/// The line ending the source uses: `\r\n` when any line has one.
pub(super) fn line_ending(src: &str) -> &'static str {
    if src.contains("\r\n") { "\r\n" } else { "\n" }
}

/// The source's indentation step: the leading whitespace of the first
/// indented line that holds a node. Falls back to two spaces.
pub(super) fn indent_unit(src: &str) -> String {
    for line in src.lines() {
        let ws = leading_ws(line);
        let rest = line.get(ws.len()..).unwrap_or("").trim_end();
        if !ws.is_empty() && !rest.is_empty() && !rest.starts_with("//") && !rest.starts_with("/*")
        {
            return ws.to_owned();
        }
    }
    CANON_UNIT.to_owned()
}

/// The spaces and tabs that open `line`.
pub(super) fn leading_ws(line: &str) -> &str {
    let n = line.len() - line.trim_start_matches([' ', '\t']).len();
    line.get(..n).unwrap_or("")
}

/// Byte offset of the start of the line that holds `pos`.
pub(super) fn line_start(src: &str, pos: usize) -> usize {
    src.get(..pos)
        .and_then(|head| head.rfind('\n'))
        .map_or(0, |i| i + 1)
}

/// Byte offset of the `\n` that ends the line holding `pos`, if any.
pub(super) fn line_end(src: &str, pos: usize) -> Option<usize> {
    src.get(pos..)
        .and_then(|tail| tail.find('\n'))
        .map(|i| pos + i)
}

/// The indentation of the line that holds `pos`.
pub(super) fn indent_at(src: &str, pos: usize) -> &str {
    let start = line_start(src, pos);
    leading_ws(src.get(start..).unwrap_or(""))
}

/// `true` when only spaces and tabs precede `pos` on its line.
pub(super) fn starts_line(src: &str, pos: usize) -> bool {
    let start = line_start(src, pos);
    src.get(start..pos)
        .is_some_and(|head| head.chars().all(|c| c == ' ' || c == '\t'))
}

/// `true` when the text after a node, up to the line end, closes the line:
/// whitespace, an optional `;`, and an optional `//` comment.
pub(super) fn closes_line(rest: &str) -> bool {
    let rest = rest.trim_start_matches([' ', '\t']);
    let rest = rest.strip_prefix(';').unwrap_or(rest);
    let rest = rest.trim_start_matches([' ', '\t']);
    rest.is_empty() || rest == "\r" || rest.starts_with("//")
}

/// Start of the comments attached above the line that starts at `line`.
///
/// Attachment rule: a comment belongs to the node below it when it fills its
/// own lines and sits directly above the node or above another attached
/// comment. That covers `//` lines and a `/* … */` block whose `/*` opens
/// its line and whose `*/` closes its line. A blank line, a node line, or a
/// block opener line stops the walk. The node's trailing same-line comment
/// sits on the node's own line, so it belongs to the node too.
pub(super) fn attach_start(src: &str, line: usize) -> usize {
    let mut start = line;
    while let Some(prev) = start.checked_sub(1).map(|p| line_start(src, p)) {
        let text = src.get(prev..start).unwrap_or("").trim();
        if text.starts_with("//") {
            start = prev;
            continue;
        }
        if text.ends_with("*/")
            && let Some(open) = block_comment_open(src, start)
        {
            start = open;
            continue;
        }
        break;
    }
    start
}

/// Line start of the `/*` that opens the block comment ending on the line
/// before `line`, when that `/*` starts its line.
fn block_comment_open(src: &str, line: usize) -> Option<usize> {
    let head = src.get(..line)?;
    let close = head.rfind("*/")?;
    let open = head.get(..close)?.rfind("/*")?;
    starts_line(src, open).then(|| line_start(src, open))
}

/// `true` when `line` holds only whitespace.
pub(super) fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

/// The line that ends just before line start `at`: its start and text.
pub(super) fn line_before(src: &str, at: usize) -> Option<(usize, &str)> {
    let start = line_start(src, at.checked_sub(1)?);
    Some((start, src.get(start..at)?))
}

/// The line that starts at `at`: the offset past its `\n` and its text.
pub(super) fn line_from(src: &str, at: usize) -> Option<(usize, &str)> {
    let end = line_end(src, at).map_or(src.len(), |e| e + 1);
    if end <= at {
        return None;
    }
    Some((end, src.get(at..end)?))
}

/// Move whole source lines to a new indentation.
///
/// Each line drops `from` and gets `to`. A line indented less than `from`
/// (a shallower comment) gets `to` in place of its own indentation. Blank
/// lines become empty. Lines join with `eol`, and the result ends in `eol`.
pub(super) fn move_lines(
    text: &str,
    from: &str,
    to: &str,
    eol: &str,
) -> Result<String, PatchError> {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let line = line.trim_end_matches('\n').trim_end_matches('\r');
        if is_blank(line) {
            out.push_str(eol);
            continue;
        }
        let rest = match line.strip_prefix(from) {
            Some(rest) => rest,
            None => {
                let ws = leading_ws(line);
                if !from.starts_with(ws) {
                    return Err(layout_error(
                        "a moved line is indented with other whitespace than its node",
                    ));
                }
                line.get(ws.len()..).unwrap_or("")
            }
        };
        out.push_str(to);
        out.push_str(rest);
        out.push_str(eol);
    }
    Ok(out)
}

/// Re-indent canonical node text for a new site in the source.
///
/// `text` starts at the node name, so its first line has no indentation.
/// Each later line drops the canonical `base` indentation and gets `target`
/// plus one source `unit` per canonical nesting level. Lines join with `eol`.
pub(super) fn reindent(
    text: &str,
    base: &str,
    target: &str,
    unit: &str,
    eol: &str,
) -> Result<String, PatchError> {
    let mut out = String::with_capacity(text.len());
    for (i, line) in text.split('\n').enumerate() {
        if i == 0 {
            out.push_str(line);
            continue;
        }
        out.push_str(eol);
        if line.is_empty() {
            continue;
        }
        let rel = line
            .strip_prefix(base)
            .ok_or_else(|| layout_error("canonical text line is not indented under its node"))?;
        let spaces = leading_ws(rel);
        if spaces.contains('\t') || !spaces.len().is_multiple_of(CANON_UNIT.len()) {
            return Err(layout_error("canonical text line has an odd indentation"));
        }
        out.push_str(target);
        for _ in 0..spaces.len() / CANON_UNIT.len() {
            out.push_str(unit);
        }
        out.push_str(rel.get(spaces.len()..).unwrap_or(""));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_apply_in_offset_order() {
        let out = apply_edits(
            "abcdef",
            vec![
                Edit::replace(4, 5, "E"),
                Edit::insert(1, "1"),
                Edit::insert(1, "2"),
            ],
        )
        .expect("apply");
        assert_eq!(out, "a12bcdEf");
    }

    #[test]
    fn overlapping_edits_are_rejected() {
        let err = apply_edits(
            "abcdef",
            vec![Edit::replace(1, 4, "x"), Edit::replace(2, 3, "y")],
        )
        .expect_err("overlap");
        assert_eq!(err.code, PatchErrorCode::UnsupportedLayout);
    }

    #[test]
    fn insertion_after_replacement_end_is_allowed() {
        let out = apply_edits("abc", vec![Edit::insert(2, "+"), Edit::replace(1, 2, "B")])
            .expect("apply");
        assert_eq!(out, "aB+c");
    }

    #[test]
    fn line_ending_detects_crlf() {
        assert_eq!(line_ending("a\r\nb\r\n"), "\r\n");
        assert_eq!(line_ending("a\nb\n"), "\n");
    }

    #[test]
    fn indent_unit_reads_first_indented_node_line() {
        assert_eq!(indent_unit("zenith {\n\t// c\n\tproject id=\"p\"\n}"), "\t");
        assert_eq!(indent_unit("zenith {\n    a\n}"), "    ");
        assert_eq!(indent_unit("zenith"), "  ");
    }

    #[test]
    fn line_helpers() {
        let src = "ab\n  cd\nef";
        assert_eq!(line_start(src, 5), 3);
        assert_eq!(line_end(src, 5), Some(7));
        assert_eq!(line_end(src, 9), None);
        assert_eq!(indent_at(src, 6), "  ");
        assert!(starts_line(src, 5));
        assert!(!starts_line(src, 6));
    }

    #[test]
    fn closes_line_accepts_terminators_and_comments() {
        assert!(closes_line(""));
        assert!(closes_line("  ;"));
        assert!(closes_line(" // note"));
        assert!(closes_line("\r"));
        assert!(!closes_line(" }"));
        assert!(!closes_line(" /* c */"));
    }

    #[test]
    fn reindent_maps_canonical_levels_to_source_unit() {
        let text = "text id=\"t\" {\n      span \"a\"\n    }";
        let out = reindent(text, "    ", "\t\t", "\t", "\r\n").expect("reindent");
        assert_eq!(out, "text id=\"t\" {\r\n\t\t\tspan \"a\"\r\n\t\t}");
    }

    #[test]
    fn attach_start_takes_contiguous_comment_lines() {
        let src = "a\n\n// one\n  // two\nnode\n";
        let node = src.find("node").expect("node");
        assert_eq!(attach_start(src, node), src.find("// one").expect("c"));
    }

    #[test]
    fn attach_start_stops_at_a_blank_line_and_a_node() {
        let src = "// far\n\nnode\n";
        let node = src.find("node").expect("node");
        assert_eq!(attach_start(src, node), node);
        let src = "a // trailing\nnode\n";
        let node = src.find("node").expect("node");
        assert_eq!(attach_start(src, node), node);
    }

    #[test]
    fn attach_start_takes_a_whole_line_block_comment() {
        let src = "a\n  /* one\n     two */\n  node\n";
        let node = line_start(src, src.find("node").expect("node"));
        assert_eq!(
            attach_start(src, node),
            line_start(src, src.find("/*").expect("c"))
        );
        let src = "a /* tail\n */\nnode\n";
        let node = src.find("node").expect("node");
        assert_eq!(attach_start(src, node), node);
    }

    #[test]
    fn neighbour_lines() {
        let src = "a\nbb\ncc";
        assert_eq!(line_before(src, 2), Some((0, "a\n")));
        assert_eq!(line_before(src, 0), None);
        assert_eq!(line_from(src, 2), Some((5, "bb\n")));
        assert_eq!(line_from(src, 5), Some((7, "cc")));
        assert_eq!(line_from(src, 7), None);
    }

    #[test]
    fn move_lines_reindents_and_keeps_relative_depth() {
        let text = "    // c\n    a {\n      b\n\n    }\n";
        let out = move_lines(text, "    ", "\t", "\r\n").expect("move");
        assert_eq!(out, "\t// c\r\n\ta {\r\n\t  b\r\n\r\n\t}\r\n");
    }

    #[test]
    fn move_lines_rejects_foreign_whitespace() {
        let err = move_lines("    a\n\tb\n", "    ", "  ", "\n").expect_err("tab");
        assert_eq!(err.code, PatchErrorCode::UnsupportedLayout);
    }

    #[test]
    fn reindent_rejects_lines_outside_the_base() {
        let err = reindent("a {\n  b\n}", "    ", "", "  ", "\n").expect_err("outside");
        assert_eq!(err.code, PatchErrorCode::UnsupportedLayout);
    }
}
