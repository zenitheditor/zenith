//! Deterministic vector geometry math for Zenith.

pub mod bezier;
pub mod bounds;
pub mod error;
pub mod point;
pub mod polyline;
mod validation;

pub use bezier::{CubicBezier, CubicBezierProjection, project_onto_cubic_bezier};
pub use bounds::RectBounds;
pub use error::GeometryError;
pub use point::{Point2, SegmentProjection};
pub use polyline::{PolylineProjection, project_onto_polyline, simplify_polyline};
