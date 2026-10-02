//! Font sourcing layer: provider trait, data types, and the bundled default.

pub mod alternates;
pub mod embedded;
pub mod local;
mod provider;

pub use alternates::{FontAlternateFeature, FontAlternateParseError, parse_font_alternate_spec};
pub use local::{LocalFontEntry, filter_wanted_families, scan_font_dirs};
pub use provider::{
    BytesFontProvider, FontData, FontProvider, FontSource, FontStyle, default_provider,
};
