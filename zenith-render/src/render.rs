//! Public entry points: rasterize a scene to pixels or PNG bytes.

use zenith_core::{AssetProvider, FontProvider};
use zenith_scene::Scene;

use crate::backend::{RasterBackend, RasterImage};
use crate::error::RenderError;
use crate::tiny_skia::{TinySkiaBackend, scaled_px};

/// Rasterize `scene` and encode the result as PNG bytes.
///
/// Uses the [`TinySkiaBackend`] internally.  The output is deterministic:
/// the same scene always produces identical bytes.
///
/// The `fonts` parameter is used to resolve font bytes for any
/// [`zenith_scene::SceneCommand::DrawGlyphRun`] commands in the scene; the
/// `assets` parameter resolves raster image bytes for any
/// [`zenith_scene::SceneCommand::DrawImage`] commands. Runs/images whose id
/// cannot be resolved are silently skipped.
///
/// # Errors
///
/// Returns [`RenderError`] when the scene dimensions are invalid or PNG
/// encoding fails.
pub fn render_png(
    scene: &Scene,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
) -> Result<Vec<u8>, RenderError> {
    render_png_scaled(scene, 1.0, fonts, assets)
}

/// Rasterize `scene` at output `scale` and encode the result as PNG bytes.
///
/// See [`RasterBackend::rasterize_scaled`] for the scale rules. `scale = 1.0`
/// is byte-identical to [`render_png`].
///
/// # Errors
///
/// Returns [`RenderError`] when `scale` is not finite and `> 0`, the scaled
/// dimensions are invalid, or PNG encoding fails.
pub fn render_png_scaled(
    scene: &Scene,
    scale: f64,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
) -> Result<Vec<u8>, RenderError> {
    let backend = TinySkiaBackend;
    let image = backend.rasterize_scaled(scene, scale, fonts, assets)?;
    backend.encode_png(&image)
}

/// Encode a straight-alpha [`RasterImage`] as deterministic PNG bytes.
///
/// # Errors
///
/// Returns [`RenderError`] when PNG encoding fails.
pub fn encode_png(image: &RasterImage) -> Result<Vec<u8>, RenderError> {
    TinySkiaBackend.encode_png(image)
}

/// Device size `(width, height)` in pixels of a `width × height` page at
/// output `scale`: `max(1, round(side × scale))` per axis (`f64::round`, half
/// away from zero).
///
/// This is the exact size [`render_image_scaled`] produces, so callers can lay
/// out a composite before rendering.
///
/// # Errors
///
/// Returns [`RenderError`] when `scale` is not finite and `> 0`, or either
/// side is invalid or exceeds the maximum raster dimension.
pub fn scaled_size(width: f64, height: f64, scale: f64) -> Result<(u32, u32), RenderError> {
    Ok((
        scaled_px(width, scale, "width")?,
        scaled_px(height, scale, "height")?,
    ))
}

/// Rasterize two scenes (`left`, `right`), composite them SIDE BY SIDE via
/// [`composite_spread`], and encode the result as deterministic PNG bytes.
///
/// `left` is blitted at `x = 0` and `right` at `x = left.width + gutter_px`.
/// `gutter_px` transparent columns are inserted between the two pages. When
/// `gutter_px = 0` the output is byte-identical to the pre-gutter behavior.
/// The shared `fonts`/`assets` providers resolve glyph runs and images for
/// both scenes.
///
/// # Errors
///
/// Returns [`RenderError`] when either scene's dimensions are invalid, the
/// combined width overflows, or PNG encoding fails.
pub fn render_spread_png(
    left: &Scene,
    right: &Scene,
    gutter_px: u32,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
) -> Result<Vec<u8>, RenderError> {
    render_spread_png_scaled(left, right, gutter_px, 1.0, fonts, assets)
}

/// [`render_spread_png`] with both pages rasterized at output `scale`.
///
/// `gutter_px` is in output (device) pixels; the caller scales it. `scale =
/// 1.0` is byte-identical to [`render_spread_png`].
///
/// # Errors
///
/// Returns [`RenderError`] when `scale` is invalid, either scene's scaled
/// dimensions are invalid, the combined width overflows, or PNG encoding
/// fails.
pub fn render_spread_png_scaled(
    left: &Scene,
    right: &Scene,
    gutter_px: u32,
    scale: f64,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
) -> Result<Vec<u8>, RenderError> {
    let backend = TinySkiaBackend;
    let left_img = backend.rasterize_scaled(left, scale, fonts, assets)?;
    let right_img = backend.rasterize_scaled(right, scale, fonts, assets)?;
    let spread = composite_spread(&left_img, &right_img, gutter_px)?;
    backend.encode_png(&spread)
}

/// The widest spread gutter [`composite_spread`] takes, in px: the page
/// pixmap dimension limit, so a gutter never allocates more than a page.
pub const MAX_SPREAD_GUTTER: u32 = 16_384;

/// Composite two rasterized pages SIDE BY SIDE into one image with an optional
/// transparent gutter between them.
///
/// `left` is blitted at `x = 0`, `right` at `x = left.width + gutter_px`,
/// both at `y = 0`. The `gutter_px` columns between the two pages remain fully
/// transparent (the canvas is initialised to straight-alpha `0,0,0,0`; no
/// special fill is required). When `gutter_px = 0` the output is byte-identical
/// to the pre-gutter behavior.
///
/// The output canvas is `width = left.width + gutter_px + right.width` and
/// `height = max(left.height, right.height)`. Pixels are copied verbatim
/// (straight-alpha RGBA8) — there is no blending, so the result is
/// deterministic.
///
/// # Errors
///
/// Returns [`RenderError`] if the combined width overflows `u32`, or if
/// `gutter_px` exceeds [`MAX_SPREAD_GUTTER`].
pub fn composite_spread(
    left: &RasterImage,
    right: &RasterImage,
    gutter_px: u32,
) -> Result<RasterImage, RenderError> {
    if gutter_px > MAX_SPREAD_GUTTER {
        return Err(RenderError::new(format!(
            "spread gutter {gutter_px} px exceeds the {MAX_SPREAD_GUTTER} px limit; set a \
             smaller spread-gutter"
        )));
    }
    let width = left
        .width
        .checked_add(gutter_px)
        .and_then(|w| w.checked_add(right.width))
        .ok_or_else(|| {
            RenderError::new(format!(
                "spread width overflow: {} + {} + {} exceeds u32",
                left.width, gutter_px, right.width
            ))
        })?;
    let height = left.height.max(right.height);

    let stride = (width as usize)
        .checked_mul(4)
        .ok_or_else(|| RenderError::new(format!("spread row stride overflow for width {width}")))?;
    let total = stride.checked_mul(height as usize).ok_or_else(|| {
        RenderError::new(format!("spread buffer size overflow ({width}×{height})"))
    })?;

    // Fully transparent canvas (straight-alpha 0,0,0,0). The gutter columns
    // are never written, so they remain transparent automatically.
    let mut rgba = vec![0u8; total];

    blit(&mut rgba, stride, left, 0);
    blit(&mut rgba, stride, right, (left.width + gutter_px) as usize);

    Ok(RasterImage {
        width,
        height,
        rgba,
    })
}

/// Copy every row of `src` into `dst` (row stride `dst_stride` bytes) starting
/// at pixel column `x_offset`, with `y = 0`. Pixels are copied straight (no
/// blending). `dst` is assumed large enough (the caller sized it from
/// `composite_spread`).
fn blit(dst: &mut [u8], dst_stride: usize, src: &RasterImage, x_offset: usize) {
    let src_stride = src.width as usize * 4;
    let byte_offset = x_offset * 4;
    for row in 0..src.height as usize {
        let src_start = row * src_stride;
        let src_row = match src.rgba.get(src_start..src_start + src_stride) {
            Some(r) => r,
            None => break, // src buffer shorter than declared height — stop safely
        };
        let dst_start = row * dst_stride + byte_offset;
        if let Some(dst_row) = dst.get_mut(dst_start..dst_start + src_stride) {
            dst_row.copy_from_slice(src_row);
        }
    }
}

/// Rasterize `scene` to a [`RasterImage`] (straight-alpha RGBA8 pixels).
///
/// Useful for pixel-level assertions in tests without decoding a PNG.
///
/// The `fonts` parameter is used to resolve font bytes for any
/// [`zenith_scene::SceneCommand::DrawGlyphRun`] commands in the scene; the
/// `assets` parameter resolves raster image bytes for any
/// [`zenith_scene::SceneCommand::DrawImage`] commands. Runs/images whose id
/// cannot be resolved are silently skipped.
///
/// # Errors
///
/// Returns [`RenderError`] when the scene dimensions are invalid.
pub fn render_image(
    scene: &Scene,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
) -> Result<RasterImage, RenderError> {
    render_image_scaled(scene, 1.0, fonts, assets)
}

/// Rasterize `scene` at output `scale` to a [`RasterImage`].
///
/// See [`RasterBackend::rasterize_scaled`] for the scale rules. `scale = 1.0`
/// is byte-identical to [`render_image`].
///
/// # Errors
///
/// Returns [`RenderError`] when `scale` is not finite and `> 0`, or the
/// scaled dimensions are invalid.
pub fn render_image_scaled(
    scene: &Scene,
    scale: f64,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
) -> Result<RasterImage, RenderError> {
    TinySkiaBackend.rasterize_scaled(scene, scale, fonts, assets)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a solid-color RasterImage of `w`×`h` filled with `rgba`.
    fn solid(w: u32, h: u32, rgba: [u8; 4]) -> RasterImage {
        let mut buf = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..(w * h) {
            buf.extend_from_slice(&rgba);
        }
        RasterImage {
            width: w,
            height: h,
            rgba: buf,
        }
    }

    #[test]
    fn composite_spread_width_is_sum() {
        let left = solid(30, 20, [255, 0, 0, 255]);
        let right = solid(40, 20, [0, 0, 255, 255]);
        let out = composite_spread(&left, &right, 0).expect("composite");
        assert_eq!(out.width, 70, "spread width must be wA + wB");
        assert_eq!(out.height, 20, "spread height must be max(hA, hB)");
        assert_eq!(out.rgba.len(), (70 * 20 * 4) as usize);
    }

    #[test]
    fn a_gutter_past_the_limit_is_an_error_not_an_allocation() {
        let page = solid(2, 2, [1, 2, 3, 255]);
        assert!(composite_spread(&page, &page, MAX_SPREAD_GUTTER).is_ok());
        for gutter in [MAX_SPREAD_GUTTER + 1, u32::MAX] {
            let Err(e) = composite_spread(&page, &page, gutter) else {
                panic!("gutter {gutter} accepted");
            };
            assert!(e.to_string().contains("spread-gutter"), "{e}");
        }
    }

    #[test]
    fn composite_spread_height_is_max() {
        let left = solid(10, 50, [1, 2, 3, 255]);
        let right = solid(10, 30, [4, 5, 6, 255]);
        let out = composite_spread(&left, &right, 0).expect("composite");
        assert_eq!(out.height, 50, "height must be the taller of the two");
    }

    #[test]
    fn composite_spread_places_pages_side_by_side() {
        let left = solid(2, 1, [10, 20, 30, 255]);
        let right = solid(3, 1, [40, 50, 60, 255]);
        let out = composite_spread(&left, &right, 0).expect("composite");
        // Row 0: two left pixels, then three right pixels.
        assert_eq!(&out.rgba[0..4], &[10, 20, 30, 255], "x=0 is left page");
        assert_eq!(&out.rgba[4..8], &[10, 20, 30, 255], "x=1 is left page");
        assert_eq!(&out.rgba[8..12], &[40, 50, 60, 255], "x=2 is right page");
        assert_eq!(&out.rgba[12..16], &[40, 50, 60, 255], "x=3 is right page");
        assert_eq!(&out.rgba[16..20], &[40, 50, 60, 255], "x=4 is right page");
    }

    #[test]
    fn composite_spread_short_page_leaves_transparent_gap() {
        // Left is taller; the right page's bottom rows stay transparent.
        let left = solid(1, 2, [9, 9, 9, 255]);
        let right = solid(1, 1, [8, 8, 8, 255]);
        let out = composite_spread(&left, &right, 0).expect("composite");
        assert_eq!(out.width, 2);
        assert_eq!(out.height, 2);
        // Row 1 (second row), right column (x=1) was never written → transparent.
        let stride = 2 * 4;
        let row1_right = &out.rgba[stride + 4..stride + 8];
        assert_eq!(
            row1_right,
            &[0, 0, 0, 0],
            "gap below short page is transparent"
        );
    }

    /// gutter=0 must produce output byte-identical to the pre-gutter behavior:
    /// width = left.width + right.width, right page starts at x = left.width.
    #[test]
    fn composite_spread_gutter_zero_is_byte_identical_to_no_gutter() {
        let left = solid(5, 3, [1, 2, 3, 255]);
        let right = solid(7, 3, [4, 5, 6, 255]);
        // Build what the "no gutter" path would produce manually.
        let width = (left.width + right.width) as usize;
        let height = left.height.max(right.height) as usize;
        let mut expected = vec![0u8; width * height * 4];
        let stride = width * 4;
        for row in 0..left.height as usize {
            for col in 0..left.width as usize {
                let dst = row * stride + col * 4;
                expected[dst..dst + 4].copy_from_slice(&[1, 2, 3, 255]);
            }
        }
        for row in 0..right.height as usize {
            for col in 0..right.width as usize {
                let dst = row * stride + (left.width as usize + col) * 4;
                expected[dst..dst + 4].copy_from_slice(&[4, 5, 6, 255]);
            }
        }
        let out = composite_spread(&left, &right, 0).expect("composite gutter=0");
        assert_eq!(out.width as usize, width, "width with gutter=0");
        assert_eq!(
            out.rgba, expected,
            "gutter=0 output must be byte-identical to no-gutter"
        );
    }

    /// gutter=40 must produce width = left+40+right, with the right page shifted
    /// by 40 columns and the 40 gutter columns staying fully transparent.
    #[test]
    fn composite_spread_gutter_shifts_right_page_and_leaves_transparent_columns() {
        let left = solid(10, 1, [11, 22, 33, 255]);
        let right = solid(20, 1, [44, 55, 66, 255]);
        let gutter: u32 = 40;
        let out = composite_spread(&left, &right, gutter).expect("composite gutter=40");
        assert_eq!(out.width, 10 + 40 + 20, "width = left + gutter + right");
        // The last pixel of the left page (x=9) is left-colored.
        assert_eq!(
            &out.rgba[9 * 4..10 * 4],
            &[11, 22, 33, 255],
            "x=9 is left page"
        );
        // x=10..x=49 are the gutter — must be fully transparent.
        for x in 10usize..50 {
            let pixel = &out.rgba[x * 4..(x + 1) * 4];
            assert_eq!(
                pixel,
                &[0, 0, 0, 0],
                "gutter column {x} must be transparent"
            );
        }
        // The first pixel of the right page (x=50) is right-colored.
        assert_eq!(
            &out.rgba[50 * 4..51 * 4],
            &[44, 55, 66, 255],
            "x=50 is right page"
        );
    }
}
