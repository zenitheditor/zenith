//! Deterministic vector geometry math for Zenith.

pub mod bezier;
pub mod bounds;
pub mod error;
pub mod guide;
pub mod intersection;
pub mod offset;
pub mod outline;
pub mod path;
pub mod point;
pub mod polyline;
pub mod transform;
mod validation;

pub use bezier::{CubicBezier, CubicBezierProjection, project_onto_cubic_bezier};
pub use bounds::RectBounds;
pub use error::GeometryError;
pub use guide::{ConstructionGuide, modular_guides, polar_guides, ratio_guides};
pub use intersection::{
    IntersectionPoint, LineIntersection, SegmentIntersection, intersect_lines, intersect_segments,
};
pub use offset::{
    OffsetRailJoin, SegmentOffset, join_adjacent_segment_offsets, offset_open_polyline_segments,
    offset_segment,
};
pub use outline::{
    OpenPolylineCap, OpenPolylineJoin, OpenPolylineOutline, OpenPolylineOutlinePolicy,
    outline_open_polyline,
};
pub use path::{
    PathAnchor, PathGeometry, PathJoinVectors, PathProjection, PathSegment, PathTopology,
};
pub use point::{Point2, SegmentProjection};
pub use polyline::{PolylineProjection, project_onto_polyline, simplify_polyline};
pub use transform::AffineTransform;
