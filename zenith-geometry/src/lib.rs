//! Deterministic vector geometry math for Zenith.

pub mod bezier;
pub mod bounds;
pub mod contour;
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
pub use contour::{
    ClosedPolyline, ClosedPolylineRelation, ClosedPolylineWinding, PointLocation,
    classify_closed_polyline_relation, collect_raw_closed_polyline_intersections,
};
pub use error::GeometryError;
pub use guide::{ConstructionGuide, modular_guides, polar_guides, ratio_guides};
pub use intersection::{
    IntersectionPoint, LineIntersection, PolylineIntersection, SegmentIntersection,
    collect_open_polyline_intersections, intersect_lines, intersect_segments,
};
pub use offset::{
    OffsetRailJoin, SegmentOffset, join_adjacent_segment_offsets, offset_open_polyline_segments,
    offset_segment,
};
pub use outline::{
    ClosedPolylineJoin, ClosedPolylineOutline, ClosedPolylineOutlinePolicy, OpenPolylineCap,
    OpenPolylineJoin, OpenPolylineOutline, OpenPolylineOutlinePolicy, PathOutline,
    outline_closed_polyline, outline_open_polyline, outline_path_geometry,
};
pub use path::{
    PathAnchor, PathGeometry, PathJoinVectors, PathProjection, PathSegment, PathTopology,
};
pub use point::{Point2, SegmentProjection};
pub use polyline::{PolylineProjection, project_onto_polyline, simplify_polyline};
pub use transform::AffineTransform;
