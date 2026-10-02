//! Pure logic for `zenith render`.
//!
//! Public entry points:
//! - [`to_scene_json`]   — parse → validate → compile → scene JSON string.
//! - [`to_png`]          — parse → validate → compile → PNG bytes (no assets).
//! - [`to_png_with_dir`] — like `to_png`, with asset directory, lock, and policy flags.
//! - [`to_pdf_with_dir`] — parse → validate → compile → PDF bytes (one page).
//! - [`to_pdf_all_pages_with_dir`] — render every page into one multi-page PDF.
//! - [`to_png_all_pages`] — render every page to PNG.
//! - [`to_png_spread`]   — render a two-page spread to PNG.
//!
//! All operate entirely on in-memory source text; the caller is responsible
//! for all filesystem I/O.
//!
//! This module is split across concern-grouped submodules:
//! - `entry`      — the error type, render artifacts, and the public entry points.
//! - `assets`     — font/asset provider construction and disk-based diagnostics.
//! - `pages`      — shared page compile and the parallel page map.
//! - `pipeline`   — shared parse/validate/page-resolution/hash helpers.
//! - [`data_input`] — load a [`DataContext`](zenith_core::DataContext) from a JSON or CSV file (`--data`).

mod assets;
pub mod data_input;
mod entry;
mod pages;
mod pipeline;
mod text_source;

#[cfg(test)]
mod tests;

pub use assets::collect_image_dimension_diagnostics;
pub(crate) use assets::{
    build_asset_provider, build_font_provider, collect_missing_asset_diagnostics,
};
pub use data_input::{DataInputError, load_data_context};
pub use entry::{
    PdfArtifact, PngArtifact, RenderCmdErr, RenderEntryOptions, SceneArtifact, SpreadRenderOpts,
    to_pdf_all_pages_with_dir, to_pdf_all_pages_with_dir_options, to_pdf_with_dir,
    to_pdf_with_dir_options, to_png, to_png_all_pages, to_png_all_pages_options, to_png_spread,
    to_png_with_dir, to_png_with_dir_options, to_scene_json, to_scene_json_with_options,
};
pub(crate) use text_source::resolve_text_sources;
