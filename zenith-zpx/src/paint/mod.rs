//! Deterministic ZPX paint program rendering.

mod render;

pub use render::render_program;
pub(crate) use render::{validate_brush, validate_program, validate_stroke};
