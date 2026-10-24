//! Detect sibling nodes written on one line without a `;` between them.
//!
//! KDL ends a node at a newline or a `;`. Two nodes on one line with no `;`
//! between them read as one node:
//!
//! - After a child block (`text id="a" { span "A" } text id="b" …`), the
//!   parser reports an unclosed child block. [`sibling_line_hint`] names the
//!   line and the fix.
//! - Without a child block (`rect id="a" rect id="b"`), the second node name
//!   becomes an argument of the first, and its properties overwrite the
//!   first node's. [`merged_sibling`] reports it, because the parse loses
//!   the first node without a word.

use kdl::{KdlDocument, KdlNode, KdlValue};

use crate::schema::node_kinds;
use crate::util::line_col;

/// The fix both reports name.
const FIX: &str = "End each node with `;` before the next one on the same line, for example \
                   `text id=\"a\" { span \"A\" }; text id=\"b\" { span \"B\" }`, or put each node \
                   on its own line.";

/// A hint naming the first line where a node follows a closed child block
/// on the same line, or `None` when no line does. The scan knows only
/// double-quoted strings and `//` comments.
pub(crate) fn sibling_line_hint(text: &str) -> Option<String> {
    let (index, _) = text
        .lines()
        .enumerate()
        .find(|(_, line)| node_after_block(line))?;
    Some(format!(
        "hint: line {} has sibling nodes on one line. {FIX}",
        index + 1
    ))
}

/// `true` when `line` has a `}` followed by the start of a node name.
fn node_after_block(line: &str) -> bool {
    let mut in_string = false;
    let mut escaped = false;
    let mut after_close = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
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
        if c.is_whitespace() {
            continue;
        }
        if after_close && c.is_alphabetic() {
            return true;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            return false;
        }
        after_close = c == '}';
        if c == '"' {
            in_string = true;
        }
    }
    false
}

/// The byte offset and message of the first node that holds a merged
/// sibling: a bare argument that names a node kind, followed by a property
/// the node already set. `None` when no node does.
pub(crate) fn merged_sibling(doc: &KdlDocument, text: &str) -> Option<(usize, String)> {
    doc.nodes()
        .iter()
        .find_map(|node| merged_in_node(node, text))
}

fn merged_in_node(node: &KdlNode, text: &str) -> Option<(usize, String)> {
    let mut seen: Vec<&str> = Vec::new();
    let mut kind: Option<&str> = None;
    for entry in node.entries() {
        match entry.name() {
            None => {
                if kind.is_none()
                    && let KdlValue::String(word) = entry.value()
                    && node_kinds().contains(&word.as_str())
                {
                    kind = Some(word.as_str());
                }
            }
            Some(name) => {
                let name = name.value();
                if let Some(kind) = kind
                    && seen.contains(&name)
                {
                    let offset = entry.span().offset();
                    let line = line_col(text, offset.min(text.len())).map_or(1, |(l, _)| l);
                    let message = format!(
                        "KDL parse error at line {line}: node `{}` sets `{name}` again after the \
                         word `{kind}`. Two sibling nodes are on one line, and the second one \
                         would overwrite the first.\n  hint: {FIX}",
                        node.name().value()
                    );
                    return Some((offset, message));
                }
                seen.push(name);
            }
        }
    }
    node.children()
        .and_then(|children| merged_sibling(children, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_names_the_line_with_a_node_after_a_block() {
        let src = "a {\n  text id=\"a\" { span \"A\" } text id=\"b\" { span \"B\" }\n}\n";
        let hint = sibling_line_hint(src).expect("hint");
        assert!(hint.contains("line 2"), "{hint}");
        assert!(hint.contains("`;`"), "{hint}");
    }

    #[test]
    fn no_hint_for_closing_braces_strings_or_comments() {
        assert_eq!(sibling_line_hint("a {\n  b { c 1 } }\n}\n"), None);
        assert_eq!(sibling_line_hint("a \"} text\"\n"), None);
        assert_eq!(sibling_line_hint("a { b 1 } // } text\n"), None);
        assert_eq!(sibling_line_hint("a { b 1 }; c 2\n"), None);
    }

    #[test]
    fn merged_rects_are_reported_and_a_semicolon_fixes_them() {
        let src = "g { rect id=\"a\" x=1 rect id=\"b\" x=2 }\n";
        let doc: KdlDocument = src.parse().expect("kdl parses");
        let (offset, message) = merged_sibling(&doc, src).expect("merged");
        assert!(
            message.contains("sets `id` again after the word `rect`"),
            "{message}"
        );
        assert_eq!(&src[offset..offset + 2], "id");

        let fixed = "g { rect id=\"a\" x=1; rect id=\"b\" x=2 }\n";
        let doc: KdlDocument = fixed.parse().expect("kdl parses");
        assert_eq!(merged_sibling(&doc, fixed), None);
        assert_eq!(doc.nodes()[0].children().map(|c| c.nodes().len()), Some(2));
    }

    #[test]
    fn a_text_span_with_a_kind_word_is_fine() {
        let src = "text id=\"t\" { span \"rect\" fill=\"x\" }\n";
        let doc: KdlDocument = src.parse().expect("kdl parses");
        assert_eq!(merged_sibling(&doc, src), None);
    }
}
