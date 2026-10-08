//! Deterministic SVG export with explicit raster fallback reports.
mod asset_text;
mod assets;
mod document;
mod effects;
mod geometry;
mod paint;
mod scopes;
mod text;
mod writer;

pub use document::{
    SvgOutput, SvgRasterizationReason, SvgRasterizedRegion, render_svg, render_svg_with,
};
