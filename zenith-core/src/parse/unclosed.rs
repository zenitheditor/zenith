//! Locate the likely missing `}` of an unclosed block.
//!
//! The kdl crate reports an unclosed child block at its opening `{`, which can
//! be hundreds of lines above the real mistake. Canonical `.zen` indents each
//! child one step deeper than its parent, so a line that is not a `}` and sits
//! at or left of the innermost open block's indent shows where the `}` was
//! forgotten. When the braces balance, [`bad_line_hint`] parses each line on its
//! own and names the first one that fails, such as a stray `)`.
//!
//! Both scanners read a line at a time and know only double-quoted strings.
//! Block comments, raw strings, and multi-line strings can mislead them, so a
//! hint is a pointer to check, not a proof.

/// A block still open: the 1-based line of its `{` and that line's indent.
struct Open {
    line: usize,
    indent: usize,
}

/// A hint for an unclosed block, or `None` when every block closes.
///
/// The block still open at the end of the file is certain: it comes from the
/// brace count alone. The line before which its `}` is missing is a guess from
/// indentation, so it is named only when every block's first child sits deeper
/// than the block's own line. Otherwise the hint names the open block and says
/// no more.
pub(crate) fn missing_close_hint(text: &str) -> Option<String> {
    let mut stack: Vec<Open> = Vec::new();
    // The first line at or left of the innermost open block that is not a `}`.
    let mut guess: Option<(usize, usize)> = None;
    let mut consistent = true;
    // The indent of the line that opened a block, until its first child line.
    let mut awaiting_child: Option<usize> = None;
    for (index, raw) in text.lines().enumerate() {
        let line_no = index + 1;
        let trimmed = raw.trim_start();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        let indent = raw.len() - trimmed.len();
        let closes = trimmed.starts_with('}');
        if let Some(opener) = awaiting_child.take()
            && !closes
            && indent <= opener
        {
            consistent = false;
        }
        if guess.is_none()
            && let Some(open) = stack.last()
            && indent <= open.indent
            && !closes
        {
            guess = Some((open.line, line_no));
        }
        let before = stack.len();
        scan_braces(trimmed, |opened| {
            if opened {
                stack.push(Open {
                    line: line_no,
                    indent,
                });
            } else {
                stack.pop();
            }
        });
        if stack.len() > before {
            awaiting_child = Some(indent);
        }
    }
    let open = stack.last()?;
    Some(match guess {
        Some((opener, before)) if consistent => format!(
            "hint: the block opened on line {opener} is still open at line {before}; a `}}` is likely missing before line {before}."
        ),
        _ => format!(
            "hint: the block opened on line {} is never closed; add a `}}` where it ends.",
            open.line
        ),
    })
}

/// Call `on_brace(true)` for each `{` and `on_brace(false)` for each `}` in
/// `line`, skipping those inside a double-quoted string.
fn scan_braces(line: &str, mut on_brace: impl FnMut(bool)) {
    let mut in_string = false;
    let mut escaped = false;
    for c in line.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => on_brace(true),
            '}' => on_brace(false),
            _ => {}
        }
    }
}

/// Braces opened minus braces closed on `line`, ignoring those in strings.
fn net_braces(line: &str) -> i32 {
    let mut net = 0;
    scan_braces(line, |opened| net += if opened { 1 } else { -1 });
    net
}

/// A hint naming the first line that does not parse as a node on its own, with
/// the parser's message, or `None` when every line parses. A line ending in
/// `\` joins the next one. Each opened block is closed for the test.
pub(crate) fn bad_line_hint(text: &str) -> Option<String> {
    let mut joined = String::new();
    let mut first_line = 0;
    for (index, raw) in text.lines().enumerate() {
        let trimmed = raw.trim();
        if joined.is_empty() {
            if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('}') {
                continue;
            }
            first_line = index + 1;
        }
        joined.push_str(raw);
        joined.push('\n');
        if trimmed.ends_with('\\') {
            continue;
        }
        let mut probe = joined.clone();
        for _ in 0..net_braces(&joined).max(0) {
            probe.push_str("}\n");
        }
        if net_braces(&joined) >= 0
            && let Err(e) = probe.parse::<kdl::KdlDocument>()
        {
            let why = e
                .diagnostics
                .first()
                .and_then(|d| d.message.clone())
                .unwrap_or_else(|| "invalid KDL".to_owned());
            return Some(format!(
                "hint: line {first_line} does not parse on its own ({why}); check it for a stray `)` or `\"`, or a value with no `=`."
            ));
        }
        joined.clear();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_line_where_the_close_is_missing() {
        let src = "a {\n  b {\n    c 1\n  d 2\n}\n";
        let hint = missing_close_hint(src).expect("hint");
        assert!(hint.contains("opened on line 2"), "{hint}");
        assert!(hint.contains("before line 4"), "{hint}");
    }

    #[test]
    fn inconsistent_indentation_names_the_open_block_only() {
        let src = "page {\n  frame {\n  text id=\"t\" { span \"x\" }\n  }\n";
        let hint = missing_close_hint(src).expect("hint");
        assert!(
            hint.contains("opened on line 1") && hint.contains("never closed"),
            "{hint}"
        );
        let flat = missing_close_hint("a {\nb {\nc 1\n}\n").expect("hint");
        assert!(
            flat.contains("opened on line 1") && flat.contains("never closed"),
            "{flat}"
        );
    }

    #[test]
    fn a_missing_close_at_the_end_names_the_opener() {
        let hint = missing_close_hint("a {\n  b 1\n").expect("hint");
        assert!(
            hint.contains("opened on line 1") && hint.contains("never closed"),
            "{hint}"
        );
    }

    #[test]
    fn braces_in_strings_do_not_count() {
        assert_eq!(missing_close_hint("a {\n  b \"}{{\"\n}\n"), None);
    }

    #[test]
    fn a_stray_paren_is_named_by_line() {
        let src = "page {\n  rect id=\"a\" w=(px)10\n  text id=\"b\" h=(px)80) fill=(token)\"x\" { span \"hi\" }\n}\n";
        assert_eq!(missing_close_hint(src), None);
        let hint = bad_line_hint(src).expect("hint");
        assert!(hint.contains("line 3"), "{hint}");
    }

    #[test]
    fn valid_lines_give_no_line_hint() {
        assert_eq!(bad_line_hint("a {\n  b 1\n  c \\\n    d=2\n}\n"), None);
    }

    #[test]
    fn balanced_input_has_no_hint() {
        assert_eq!(missing_close_hint("a {\n  b {\n  }\n}\n"), None);
    }
}
