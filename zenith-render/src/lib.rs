//! CPU PNG, self-contained SVG, and vector PDF renderers for Zenith.
//!
//! Owns the raster backend adapter trait (tiny-skia is the first engine),
//! deterministic PNG production from a scene display list, SVG and raster
//! image decode, glyph rasterization, and enforcement of all raster-time
//! determinism rules. Backend types never appear in the public API.

mod backend;
mod compose;
mod error;
mod intrinsic;
mod pdf;
mod render;
mod scopes;
mod svg;
mod svg_style;
mod tiny_skia;

pub use backend::{RasterBackend, RasterImage};
pub use compose::composite_over;
pub use error::RenderError;
pub use intrinsic::{asset_intrinsic_size, asset_intrinsic_sizes};
pub use pdf::{PdfOptions, render_pdf, render_pdf_multi, render_pdf_multi_with, render_pdf_with};
pub use render::{
    composite_spread, encode_png, render_image, render_image_scaled, render_png, render_png_scaled,
    render_spread_png, render_spread_png_scaled, scaled_size,
};
pub use svg::{
    SvgOutput, SvgRasterizationReason, SvgRasterizedRegion, render_svg, render_svg_with,
};
pub use tiny_skia::TinySkiaBackend;
