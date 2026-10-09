//! Byte ranges of KDL nodes and entries in the text they were parsed from.

use kdl::{KdlEntry, KdlNode};

use super::super::error::PatchError;
use super::super::text::{closes_line, layout_error, line_end, slice};

/// Byte range of `node` in the text it was parsed from.
pub(in crate::patch) fn node_range(node: &KdlNode) -> (usize, usize) {
    let span = node.span();
    (span.offset(), span.offset() + span.len())
}

/// Byte range of `entry` in the text it was parsed from.
pub(in crate::patch) fn entry_range(entry: &KdlEntry) -> (usize, usize) {
    let span = entry.span();
    (span.offset(), span.offset() + span.len())
}

/// Byte offset just past the node name.
pub(in crate::patch) fn name_end(node: &KdlNode) -> usize {
    let span = node.name().span();
    span.offset() + span.len()
}

/// The node's child nodes, empty when it has no child block.
pub(super) fn children_of(node: &KdlNode) -> &[KdlNode] {
    node.children().map_or(&[], |d| d.nodes())
}

/// Byte offset just past the node's last entry, or its name when it has none.
pub(super) fn entries_end(node: &KdlNode) -> usize {
    node.entries()
        .last()
        .map_or_else(|| name_end(node), |e| entry_range(e).1)
}

/// Byte offset just past the `\n` that ends the line holding `pos`, when the
/// rest of that line only closes it.
pub(super) fn closing_line_end(src: &str, pos: usize) -> Result<usize, PatchError> {
    let end = line_end(src, pos).ok_or_else(|| layout_error("the node ends the file"))?;
    if !closes_line(slice(src, pos, end)?) {
        return Err(layout_error("other text follows the node on its line"));
    }
    Ok(end + 1)
}

/// Byte offsets of the `{` and `}` of `node`'s child block in `src`, when
/// only whitespace separates the last entry from the `{` and the `}` ends
/// the node.
pub(super) fn block_braces(src: &str, node: &KdlNode) -> Result<(usize, usize), PatchError> {
    let (start, end) = node_range(node);
    let head_end = entries_end(node);
    let rest = slice(src, head_end, end)?;
    let open_rel = rest
        .find(|c: char| !c.is_whitespace())
        .ok_or_else(|| layout_error("no child block opener"))?;
    if rest.get(open_rel..open_rel + 1) != Some("{") {
        return Err(layout_error(
            "text sits between the entries and the child block",
        ));
    }
    let close = start + slice(src, start, end)?.trim_end().len();
    let close = close
        .checked_sub(1)
        .ok_or_else(|| layout_error("no child block closer"))?;
    if src.get(close..close + 1) != Some("}") {
        return Err(layout_error("the child block is followed by other text"));
    }
    Ok((head_end + open_rel, close))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kdl::KdlDocument;

    fn nodes(src: &str) -> KdlDocument {
        src.parse().expect("kdl")
    }

    #[test]
    fn entries_end_falls_back_to_the_name() {
        let doc = nodes("styles {}\nrect id=\"r\" x=1");
        assert_eq!(entries_end(&doc.nodes()[0]), "styles".len());
        let rect_line = "styles {}\n".len();
        assert_eq!(
            entries_end(&doc.nodes()[1]),
            rect_line + "rect id=\"r\" x=1".len()
        );
    }

    #[test]
    fn block_braces_finds_open_and_close() {
        let src = "frame id=\"f\" {\n  rect\n}";
        let doc = nodes(src);
        let (open, close) = block_braces(src, &doc.nodes()[0]).expect("braces");
        assert_eq!(&src[open..=open], "{");
        assert_eq!(close, src.len() - 1);
    }

    #[test]
    fn block_braces_rejects_a_missing_block() {
        let src = "rect id=\"r\"";
        let doc = nodes(src);
        assert!(block_braces(src, &doc.nodes()[0]).is_err());
    }
}
