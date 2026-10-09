//! Pure gesture geometry. Wiring only.
//!
//! - `vector` — points, vectors, and affine linear parts.
//! - `resize` — grips and the resize that keeps the opposite grip fixed.

mod resize;
mod vector;

pub(crate) use resize::{Drag, Grip, Rect, resize};
pub(crate) use vector::{Pt, add, angle_deg, det, linear, linear_part, sub, unmap_vector};
