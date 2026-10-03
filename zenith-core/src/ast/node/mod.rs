//! Node types for the renderable layer of a `.zen` document.
//!
//! Wiring-only root: the node-layer types are grouped into cohesion-based
//! submodules and re-exported flat here so the crate's public surface
//! (`zenith_core::RectNode`, `ast::node::Anchor`, …) is unchanged.

mod accessors;
mod anchor;
mod common;
mod container;
mod effect;
mod layout_item;
mod leaf;
mod special;

pub use accessors::BoxView;
pub use anchor::{
    Anchor, AnchorEdge, ConnectorAnchor, ConnectorAnchorParseError, anchor_xy, parse_anchor,
    parse_anchor_edge, parse_connector_anchor,
};
pub use common::{Node, ObjectPosition, Point, TextSpan, UnknownProperty, UnknownValue};
pub use container::{
    FrameNode, GroupNode, ProtectedRegion, TableCell, TableColumn, TableNode, TableRow,
};
pub use effect::{LightNode, MeshNode};
pub use layout_item::{
    LayoutAlign, LayoutContainer, LayoutItem, LayoutJustify, LayoutKind, LayoutPosition,
    SizeKeyword,
};
pub use leaf::{
    AnchorKind, ChartNode, ChartSeries, CodeNode, EllipseNode, ImageNode, LineNode, PathAnchor,
    PathNode, PathSubpath, PathSubpathRef, PatternNode, PolygonNode, PolylineNode, RectNode,
    TextNode,
};
pub use special::{
    ConnectorNode, FieldNode, FootnoteNode, InstanceNode, Override, ShapeNode, TocNode, UnknownNode,
};
