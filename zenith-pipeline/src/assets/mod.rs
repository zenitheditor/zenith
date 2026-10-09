//! Project inputs read through the host: fonts, image and SVG assets, text
//! sources, data contexts, and the diagnostics about missing files.

mod data;
mod disk;
mod fonts;
mod images;
mod lock;
mod text_source;

#[cfg(test)]
mod tests;

pub use data::{DataInputError, load_data_context};
pub use disk::{
    collect_image_dimension_diagnostics, collect_missing_asset_diagnostics,
    collect_missing_import_asset_diagnostics, disk_diagnostics, disk_diagnostics_with_imports,
};
pub use fonts::{build_font_provider, build_font_provider_with_imports};
pub use images::{
    build_asset_provider, build_asset_provider_with_imports, image_sizes, read_image_sizes,
};
pub use text_source::resolve_text_sources;
