use crate::swatch_png::SWATCH_PNG;
use std::sync::Arc;
use zenith_core::{AssetKind, BytesAssetProvider};

pub fn swatch_provider() -> BytesAssetProvider {
    let mut p = BytesAssetProvider::new();
    p.register("asset.swatch", AssetKind::Image, Arc::from(SWATCH_PNG));
    p
}
