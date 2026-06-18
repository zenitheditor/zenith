//! Font sourcing layer: provider trait, data types, and the bundled default.

mod provider;

pub use provider::{BytesFontProvider, FontData, FontProvider, FontStyle, default_provider};
