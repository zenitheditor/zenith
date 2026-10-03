//! Detection of auto-layout frames in a node tree.
//!
//! The scene engine lowers every `row` / `column` / `grid` frame to absolute
//! geometry before compile. Validation skips geometry checks on a page that
//! holds such a frame, because its authored geometry is not final there.

use super::{LayoutKind, Node};

/// `true` when any node in `nodes`, at any depth, is a frame whose layout
/// positions its children (`row`, `column`, or `grid`).
///
/// The walk descends frames, groups, table cells, unknown containers, and
/// pattern motifs.
pub fn subtree_uses_layout(nodes: &[Node]) -> bool {
    nodes.iter().any(node_uses_layout)
}

fn node_uses_layout(node: &Node) -> bool {
    match node {
        Node::Frame(f) => {
            f.layout
                .as_ref()
                .is_some_and(LayoutKind::positions_children)
                || subtree_uses_layout(&f.children)
        }
        Node::Group(g) => subtree_uses_layout(&g.children),
        Node::Unknown(u) => subtree_uses_layout(&u.children),
        Node::Table(t) => t
            .rows
            .iter()
            .any(|row| row.cells.iter().any(|c| subtree_uses_layout(&c.children))),
        Node::Pattern(p) => node_uses_layout(&p.motif),
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Line(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Image(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Footnote(_)
        | Node::Shape(_)
        | Node::Connector(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{KdlAdapter, KdlSource};

    fn page_children(body: &str) -> Vec<Node> {
        let src = format!(
            "zenith version=1 {{\n  document id=\"d\" {{\n    page id=\"pg\" w=(px)100 h=(px)100 {{\n{body}\n    }}\n  }}\n}}\n"
        );
        let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
        doc.body.pages.into_iter().next().expect("page").children
    }

    #[test]
    fn detects_nested_layout_frames_only() {
        assert!(!subtree_uses_layout(&page_children(
            r#"      frame id="f" x=(px)0 y=(px)0 w=(px)10 h=(px)10 { rect id="r" }"#
        )));
        assert!(!subtree_uses_layout(&page_children(
            r#"      frame id="f" x=(px)0 y=(px)0 w=(px)10 h=(px)10 layout="absolute""#
        )));
        assert!(subtree_uses_layout(&page_children(
            r#"      group id="g" { frame id="f" x=(px)0 y=(px)0 layout="row" }"#
        )));
        assert!(subtree_uses_layout(&page_children(
            r#"      frame id="f" x=(px)0 y=(px)0 w=(px)10 h=(px)10 layout="grid""#
        )));
    }
}
