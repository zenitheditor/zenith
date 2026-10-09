use zenith_core::BytesAssetProvider;

/// A shared empty asset provider for tests that draw no images.
pub fn no_assets() -> BytesAssetProvider {
    BytesAssetProvider::new()
}
