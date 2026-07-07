//! ZPX raster substrate document model and deterministic manifest codec.

pub mod error;
pub mod manifest;
pub mod model;

pub use error::ZpxError;
pub use manifest::{parse_manifest, serialize_manifest};
pub use model::{
    Adjustment, AlphaMode, BlobRef, Canvas, ColorSpace, ContentHash, Layer, LayerSource, Mask,
    MaskSource, ZpxDoc,
};
