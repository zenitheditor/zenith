//! Mutable access to the anchor placement attributes and the `x` / `y` of
//! an anchor-bearing node.

use zenith_core::{Dimension, Node, PropertyValue};

/// Mutable references to the six anchor attributes and `x` / `y` of one
/// node.
pub(in crate::engine) struct AnchorFieldsMut<'a> {
    pub anchor: &'a mut Option<String>,
    pub zone: &'a mut Option<String>,
    pub sibling: &'a mut Option<String>,
    pub parent: &'a mut Option<bool>,
    pub edge: &'a mut Option<String>,
    pub gap: &'a mut Option<Dimension>,
    pub x: &'a mut Option<PropertyValue>,
    pub y: &'a mut Option<PropertyValue>,
}

/// Build an [`AnchorFieldsMut`] from a node struct with the shared fields.
macro_rules! fields {
    ($n:expr) => {
        AnchorFieldsMut {
            anchor: &mut $n.anchor,
            zone: &mut $n.anchor_zone,
            sibling: &mut $n.anchor_sibling,
            parent: &mut $n.anchor_parent,
            edge: &mut $n.anchor_edge,
            gap: &mut $n.anchor_gap,
            x: &mut $n.x,
            y: &mut $n.y,
        }
    };
}

/// The anchor fields of `node`, or `None` for the kinds that carry no
/// anchor: the same kinds as [`Node::anchor_view`].
pub(in crate::engine) fn anchor_fields_mut(node: &mut Node) -> Option<AnchorFieldsMut<'_>> {
    let f = match node {
        Node::Rect(n) => fields!(n),
        Node::Ellipse(n) => fields!(n),
        Node::Text(n) => fields!(n),
        Node::Code(n) => fields!(n),
        Node::Image(n) => fields!(n),
        Node::Frame(n) => fields!(n),
        Node::Group(n) => fields!(n),
        Node::Shape(n) => fields!(n),
        Node::Table(n) => fields!(n),
        Node::Field(n) => fields!(n),
        Node::Toc(n) => fields!(n),
        Node::Pattern(n) => fields!(n),
        Node::Chart(n) => fields!(n),
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
    Some(f)
}

impl AnchorFieldsMut<'_> {
    /// Remove all six anchor attributes.
    pub(super) fn clear(&mut self) {
        *self.anchor = None;
        *self.zone = None;
        *self.sibling = None;
        *self.parent = None;
        *self.edge = None;
        *self.gap = None;
    }
}
