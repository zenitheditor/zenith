//! CPU PNG, self-contained SVG, and vector PDF renderers for Zenith.
//!
//! Owns the raster backend adapter trait (tiny-skia is the first engine),
//! deterministic PNG production from a scene display list, SVG and raster
//! image decode, glyph rasterization, and enforcement of all raster-time
//! determinism rules. Backend types never appear in the public API.

mod backend;
mod compose;
mod error;
mod glyph_bitmap;
mod intrinsic;
mod pdf;
mod raster_capture;
mod render;
mod scopes;
mod svg;
mod svg_style;
mod tiny_skia;

pub use backend::{RasterBackend, RasterImage};
pub use compose::composite_over;
pub use error::RenderError;
pub use intrinsic::{asset_intrinsic_size, asset_intrinsic_sizes};
pub use pdf::{
    PdfExportOptions, PdfOptions, PdfOutput, PdfRasterizationReason, PdfRasterizedRegion,
    render_pdf, render_pdf_multi, render_pdf_multi_report, render_pdf_multi_report_with_options,
    render_pdf_multi_with, render_pdf_report, render_pdf_report_with_options, render_pdf_with,
};
pub use render::{
    composite_spread, encode_png, render_image, render_image_scaled, render_png, render_png_scaled,
    render_spread_png, render_spread_png_scaled, scaled_size,
};
pub use svg::{
    SvgOptions, SvgOutput, SvgRasterizationReason, SvgRasterizedRegion, render_svg,
    render_svg_with, render_svg_with_options,
};
pub use tiny_skia::TinySkiaBackend;
