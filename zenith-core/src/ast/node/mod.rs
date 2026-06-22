//! Node types for the renderable layer of a `.zen` document.
//!
//! Wiring-only root: the node-layer types are grouped into cohesion-based
//! submodules and re-exported flat here so the crate's public surface
//! (`zenith_core::RectNode`, `ast::node::Anchor`, …) is unchanged.

mod anchor;
mod common;
mod container;
mod leaf;
mod special;

pub use anchor::{Anchor, anchor_xy, parse_anchor};
pub use common::{Node, ObjectPosition, Point, TextSpan, UnknownProperty, UnknownValue};
pub use container::{FrameNode, GroupNode, TableCell, TableColumn, TableNode, TableRow};
pub use leaf::{
    CodeNode, EllipseNode, ImageNode, LineNode, PatternNode, PolygonNode, PolylineNode, RectNode,
    TextNode,
};
pub use special::{
    ConnectorNode, FieldNode, FootnoteNode, InstanceNode, Override, ShapeNode, TocNode, UnknownNode,
};
