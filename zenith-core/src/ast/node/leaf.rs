//! Leaf node structs: shapes and text-bearing primitives that have no child
//! `Node`s of their own (rect, line, ellipse, image, text, code, polygon,
//! polyline, pattern, chart).
//!
//! Submodules: `shapes` (image/rect/line/ellipse), `text` (text/code),
//! `paths` (polygon/polyline/path + anchor/subpath types), `graphics`
//! (pattern/chart).

mod graphics;
mod paths;
mod shapes;
mod text;

pub use graphics::{ChartNode, ChartSeries, PatternNode};
pub use paths::{
    AnchorKind, PathAnchor, PathNode, PathSubpath, PathSubpathRef, PolygonNode, PolylineNode,
};
pub use shapes::{EllipseNode, ImageNode, LineNode, RectNode};
pub use text::{CodeNode, TextNode};
