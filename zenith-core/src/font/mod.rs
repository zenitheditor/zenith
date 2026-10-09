//! Font sourcing layer: provider trait, data types, and the bundled default.

pub mod alternates;
mod bundled;
pub mod embedded;
pub mod local;
mod miss_log;
mod provider;

pub use alternates::{FontAlternateFeature, FontAlternateParseError, parse_font_alternate_spec};
pub use bundled::{
    BundledFace, bundled_face_by_file, bundled_face_for, bundled_faces, default_provider,
};
pub use local::{LocalFontEntry, filter_wanted_families, scan_font_dirs};
pub use miss_log::{FaceRequest, FontMissLog};
pub use provider::{BytesFontProvider, FontData, FontProvider, FontSource, FontStyle};
