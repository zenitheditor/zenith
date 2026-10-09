//! Compile and render entry points: scene JSON, PNG (page, every page,
//! spread, contact sheet), SVG, PDF, and compiled pages with node boxes.
//!
//! Wiring only; the concerns live in submodules:
//! - `options` — [`RenderOptions`] and [`SpreadOptions`].
//! - `scale` — the accepted raster scale range.
//! - `compile` — shared page compile and [`compile_pages`].
//! - `raster` — multi-page raster at a chosen scale.
//! - `png` — scene JSON and PNG entry points.
//! - `region` — one window of a page at any scale (the editor canvas).
//! - `sheet` — the contact sheet.
//! - `svg` / `pdf` — vector entry points.
//! - `view` — one page of a parsed document with its node boxes and an
//!   optional raster (the editor canvas path).

mod compile;
mod options;
mod pdf;
mod png;
mod raster;
mod region;
mod scale;
mod sheet;
mod svg;
mod view;

pub use compile::{CompiledPage, CompiledPages, PageSelection, compile_pages};
pub use options::{RenderOptions, SpreadOptions};
pub use pdf::{PdfArtifact, render_pdf, render_pdf_pages};
pub use png::{
    PngArtifact, PngPagesArtifact, SceneArtifact, render_png, render_png_pages, render_png_spread,
    render_scene_json,
};
pub use region::{RegionArtifact, RegionView, region_error, region_rect, render_png_region};
pub use scale::{MAX_RENDER_SCALE, check_render_scale};
pub use sheet::{
    ContactSheetArtifact, SHEET_AUTO_MAX_WIDTH_PX, SHEET_GUTTER_PX, SHEET_LABEL_BAND_PX,
    render_contact_sheet,
};
pub use svg::{
    SvgArtifact, SvgPageArtifact, SvgPagesArtifact, rasterization_diagnostics, render_svg,
    render_svg_pages,
};
pub use view::{PageView, PngImage, ViewOptions, view_page};
pub use zenith_render::{DeviceRect, PageRect};
