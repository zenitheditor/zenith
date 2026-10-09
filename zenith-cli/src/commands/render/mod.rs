//! `zenith render` on the native host.
//!
//! Parse, policy, imports, fonts, assets, compile, and render live in
//! `zenith-pipeline`. This module binds them to the native host:
//! - `entry` — one native-host wrapper per pipeline entry point, plus
//!   `--data` loading from disk.
//! - `batch` — merge / variant output encoding and their config policy.
//! - `scale` — `--scale` / `--raster-scale` flag parsing.

mod batch;
mod entry;
mod scale;

#[cfg(test)]
mod tests;

pub use batch::{BatchExportOptions, BatchFormat};
pub(crate) use batch::{encode_batch_scene, load_batch_policy};
pub use entry::{
    load_data_context, to_contact_sheet, to_pdf_all_pages_with_dir,
    to_pdf_all_pages_with_dir_options, to_pdf_with_dir, to_pdf_with_dir_options, to_png,
    to_png_all_pages, to_png_all_pages_options, to_png_spread, to_png_with_dir,
    to_png_with_dir_options, to_scene_json, to_scene_json_with_options, to_svg_all_pages_with_dir,
    to_svg_all_pages_with_dir_options, to_svg_with_dir, to_svg_with_dir_options,
};
pub(crate) use scale::parse_scale_flag;
