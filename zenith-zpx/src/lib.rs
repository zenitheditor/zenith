//! ZPX raster substrate document model and deterministic manifest codec.

pub mod error;
pub mod manifest;
pub mod model;
pub mod paint;

pub use error::ZpxError;
pub use manifest::{parse_manifest, serialize_manifest};
pub use model::{
    Adjustment, AlphaMode, BlobRef, Brush, Canvas, ColorSpace, ContentHash, DabSample, Layer,
    LayerSource, Mask, MaskSource, Stroke, StrokeProgram, ZpxDoc,
};
pub use paint::render_program;
