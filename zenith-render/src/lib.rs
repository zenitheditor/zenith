//! CPU PNG reference renderer for Zenith.
//!
//! Owns the raster backend adapter trait (tiny-skia is the first engine),
//! deterministic PNG production from a scene display list, SVG and raster
//! image decode, glyph rasterization, and enforcement of all raster-time
//! determinism rules. Backend types never appear in the public API.

mod backend;
mod error;
mod pdf;
mod render;
mod svg_style;
mod tiny_skia;

pub use backend::{RasterBackend, RasterImage};
pub use error::RenderError;
pub use pdf::{PdfOptions, render_pdf, render_pdf_multi, render_pdf_multi_with, render_pdf_with};
pub use render::{composite_spread, render_image, render_png, render_spread_png};
pub use tiny_skia::TinySkiaBackend;
