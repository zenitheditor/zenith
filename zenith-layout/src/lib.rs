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
pub mod ot_layout;
pub mod rustybuzz_engine;

// Curated flat re-exports for the public surface.
pub use engine::{
    FallbackResult, FontFeature, KerningPairAdjustment, PositionedGlyph, ShapeRequest,
    TextDirection, TextLayoutEngine, ZenithGlyphRun,
};
pub use error::LayoutError;
pub use font_meta::{FaceMetadata, face_metadata};
pub use glyph_outline::{
    GlyphOutline, GlyphOutlineContour, GlyphOutlineRequest, GlyphOutlineSegment, GlyphRunOutline,
    GlyphRunOutlineRequest, OutlinedGlyph, glyph_outline, glyph_outline_contours,
    glyph_outline_path_node, glyph_outline_path_subpaths, glyph_run_outline,
};
pub use ot_layout::{
    FeatureEntry, FeatureList, GLYPH_ALTERNATES_LIMITS, GlyphAlternates, list_glyph_alternates,
    list_layout_features,
};
pub use rustybuzz_engine::RustybuzzEngine;
