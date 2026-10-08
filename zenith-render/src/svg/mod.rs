//! Deterministic SVG export with explicit raster fallback reports.
mod asset_text;
mod assets;
pub(crate) use assets::checked_svg;
mod document;
mod effects;
mod geometry;
mod paint;
mod scopes;
mod text;
mod writer;

pub use document::{
    SvgOptions, SvgOutput, SvgRasterizationReason, SvgRasterizedRegion, render_svg,
    render_svg_with, render_svg_with_options,
};
