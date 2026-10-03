//! The `RasterBackend` trait — the swappable seam between scene and pixels.
//!
//! No backend-specific types (e.g. tiny-skia) appear anywhere in this module.

use zenith_core::{AssetProvider, FontProvider};
use zenith_scene::Scene;

use crate::error::RenderError;

/// A rasterized image in straight-alpha RGBA8 format (row-major).
///
/// Pixels are stored as `[r, g, b, a, r, g, b, a, …]` with row stride
/// `width * 4`.  Alpha is **straight** (un-premultiplied), matching the
/// `Color` type in `zenith-scene`.
pub struct RasterImage {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Raw RGBA8 bytes (`width * height * 4` bytes).
    pub rgba: Vec<u8>,
}

/// Trait that abstracts over different CPU rasterization backends.
///
/// The associated methods take and return only types from this crate or the
/// standard library — no backend-specific types cross the boundary.
pub trait RasterBackend {
    /// Rasterize a scene to straight-alpha RGBA8 pixels plus dimensions.
    ///
    /// The `fonts` parameter is used to resolve font bytes for glyph runs.
    /// The `assets` parameter is used to resolve raster image bytes for
    /// `DrawImage` commands. Runs/images whose id cannot be resolved are
    /// silently skipped — they do not cause an error.
    fn rasterize(
        &self,
        scene: &Scene,
        fonts: &dyn FontProvider,
        assets: &dyn AssetProvider,
    ) -> Result<RasterImage, RenderError> {
        self.rasterize_scaled(scene, 1.0, fonts, assets)
    }

    /// Rasterize a scene at output `scale` (`1.0` = one device pixel per page
    /// pixel).
    ///
    /// The scale applies at raster time as a root transform, never as a
    /// resample of a full-size image. Each output axis is
    /// `max(1, round(page × scale))` pixels (`f64::round`, half away from
    /// zero). Pixel-sized effects (shadow offset and blur, blur radius, mask
    /// box and feather, noise cell) scale with it. `scale = 1.0` is
    /// byte-identical to [`RasterBackend::rasterize`].
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] when `scale` is not finite and `> 0`, or the
    /// scaled dimensions are invalid.
    fn rasterize_scaled(
        &self,
        scene: &Scene,
        scale: f64,
        fonts: &dyn FontProvider,
        assets: &dyn AssetProvider,
    ) -> Result<RasterImage, RenderError>;

    /// Encode a [`RasterImage`] as deterministic PNG bytes.
    fn encode_png(&self, image: &RasterImage) -> Result<Vec<u8>, RenderError>;
}
