//! Deterministic write-side asset producers for frozen Zenith assets.

mod error;
mod file_import;
mod model;
#[cfg(test)]
mod smoke;
mod svg_native;
mod zpx_bake;

pub use error::ProduceError;
pub use file_import::FileImportProducer;
pub use model::{
    AssetProducer, FileImportProvenance, ProduceRequest, ProducedAsset, Provenance,
    ZpxBakeProvenance,
};
pub use svg_native::{SvgNativeOptions, svg_to_native_paths};
pub use zpx_bake::ZpxBakeProducer;
