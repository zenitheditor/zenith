//! [`Node::anchor_view`]: the anchor placement fields of a node.

use crate::ast::value::{Dimension, PropertyValue};

use super::Node;

/// Borrowed anchor placement fields of an anchor-bearing node, returned by
/// [`Node::anchor_view`]. Field meanings match
/// [`RectNode`](super::RectNode).
#[derive(Debug, Clone, Copy)]
pub struct AnchorView<'a> {
    pub id: &'a str,
    pub anchor: Option<&'a str>,
    pub anchor_zone: Option<&'a str>,
    pub anchor_sibling: Option<&'a str>,
    pub anchor_parent: Option<bool>,
    pub anchor_edge: Option<&'a str>,
    pub anchor_gap: Option<&'a Dimension>,
    pub x: Option<&'a PropertyValue>,
    pub y: Option<&'a PropertyValue>,
    pub w: Option<&'a PropertyValue>,
    pub h: Option<&'a PropertyValue>,
}

/// Build an [`AnchorView`] from a node struct with the shared anchor fields.
macro_rules! view {
    ($n:expr) => {
        AnchorView {
            id: $n.id.as_str(),
            anchor: $n.anchor.as_deref(),
            anchor_zone: $n.anchor_zone.as_deref(),
            anchor_sibling: $n.anchor_sibling.as_deref(),
            anchor_parent: $n.anchor_parent,
            anchor_edge: $n.anchor_edge.as_deref(),
            anchor_gap: $n.anchor_gap.as_ref(),
            x: $n.x.as_ref(),
            y: $n.y.as_ref(),
            w: $n.w.as_ref(),
            h: $n.h.as_ref(),
        }
    };
}

impl Node {
    /// The anchor placement fields, or `None` for kinds that never carry an
    /// `anchor` (line, connector, polygon, polyline, path, footnote,
    /// instance, light, mesh, unknown).
    pub fn anchor_view(&self) -> Option<AnchorView<'_>> {
        let v = match self {
            Node::Rect(n) => view!(n),
            Node::Ellipse(n) => view!(n),
            Node::Text(n) => view!(n),
            Node::Code(n) => view!(n),
            Node::Image(n) => view!(n),
            Node::Frame(n) => view!(n),
            Node::Group(n) => view!(n),
            Node::Shape(n) => view!(n),
            Node::Table(n) => view!(n),
            Node::Field(n) => view!(n),
            Node::Toc(n) => view!(n),
            Node::Pattern(n) => view!(n),
            Node::Chart(n) => view!(n),
            Node::Line(_)
            | Node::Connector(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Footnote(_)
            | Node::Instance(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => return None,
        };
        Some(v)
    }
}
