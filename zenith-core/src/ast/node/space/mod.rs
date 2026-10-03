//! Coordinate spaces of the node tree: geometry resolution to px, the
//! translation a container applies to its children, and moving a node.

mod child;
mod resolve;
mod translate;

pub use resolve::resolve_geometry_px;
pub use translate::translate_node;
