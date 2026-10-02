//! Local/system font scanning: metadata only, with an on-disk index.

mod index;
mod scan;

pub use scan::{LocalFontEntry, filter_wanted_families, scan_font_dirs};
