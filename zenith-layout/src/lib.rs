//! Text-shaping layer for Zenith.
//!
//! Shapes a text run into positioned glyphs using `rustybuzz` + `ttf-parser`
//! (the same proven stack as the sibling `oxipdf` project). All third-party
//! types are confined to `rustybuzz_engine`; downstream crates see only
//! Zenith-owned records.

pub mod engine;
pub mod error;
pub mod font_meta;
pub mod glyph_outline;
pub mod rustybuzz_engine;

// Curated flat re-exports for the public surface.
pub use engine::{
    FallbackResult, FontFeature, PositionedGlyph, ShapeRequest, TextDirection, TextLayoutEngine,
    ZenithGlyphRun,
};
pub use error::LayoutError;
pub use font_meta::{FaceMetadata, face_metadata};
pub use glyph_outline::{GlyphOutline, GlyphOutlineRequest, GlyphOutlineSegment, glyph_outline};
pub use rustybuzz_engine::RustybuzzEngine;
