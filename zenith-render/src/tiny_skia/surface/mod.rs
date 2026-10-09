//! The render surface: the device-pixel window of the page that a render
//! writes, and the draw calls that place geometry on it.
//!
//! A full-page render uses `Surface::page`. A region render uses a window.
//! Every quantity in the render loop stays in full-page device space: the
//! transform stack, the clip stack, and the effect specs. Only these draw
//! calls move geometry onto the surface. On the full page they make the same
//! tiny-skia call as before, so full-page output is unchanged.
//!
//! Wiring only:
//! - `window` — `Surface` and where a draw runs (`Placement`).
//! - `scratch` — pixel and coverage copies between a scratch buffer and the
//!   surface.
//! - `draw` — pixmap fills, strokes, rects, and images.
//! - `mask` — coverage-mask fills.

mod draw;
mod mask;
mod scratch;
mod window;

pub(in crate::tiny_skia) use draw::{draw_pixmap, fill_device_rect, fill_path, stroke_path};
pub(in crate::tiny_skia) use mask::{mask_fill_path, mask_intersect_path};
pub(in crate::tiny_skia) use window::Surface;
