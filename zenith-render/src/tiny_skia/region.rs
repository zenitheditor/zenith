//! Region render: rasterize one device-pixel window of a page.
//!
//! The window grows by the scene's effect pad (see `reach`) into a surface,
//! the surface renders, and the window is cut out. When the whole page fits
//! the region caps, the surface runs in exact mode (see `surface`). The surface is capped at
//! [`MAX_SURFACE_SIDE`] per axis and [`MAX_SURFACE_PIXELS`] in total. A pad
//! that does not fit is shortened, and effect pixels within the missing reach
//! of the window edge can then differ from the full render.

use tiny_skia::Pixmap;
use zenith_core::{AssetProvider, FontProvider};
use zenith_scene::Scene;

use super::backend::rasterize_surface;
use super::encode::{encode_premultiplied_rows, premultiplied_to_straight_rgba};
use super::reach::effect_pad;
use super::surface::Surface;
use crate::backend::RasterImage;
use crate::error::RenderError;
use crate::region::MAX_REGION_PIXELS;

/// Largest surface side in pixels. tiny-skia anti-aliases a path only while
/// every device coordinate stays below 8192 (`8192 << 2` overflows its
/// 16-bit supersample grid), and tiles a larger pixmap.
pub(crate) const MAX_SURFACE_SIDE: u32 = 8191;

/// Largest surface area in pixels (128 MiB of RGBA per buffer).
pub(crate) const MAX_SURFACE_PIXELS: u64 = 1 << 25;

/// A device-pixel window: origin and size, within a page of `page` pixels.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Window {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) w: u32,
    pub(crate) h: u32,
    pub(crate) page_w: u32,
    pub(crate) page_h: u32,
}

/// Rasterize `window` of `scene` at `scale` to a straight-alpha image.
///
/// The caller checks that `window` lies inside the page and within the
/// region caps.
///
/// # Errors
///
/// Returns [`RenderError`] when a pixmap cannot be allocated or a blend layer
/// fails to composite.
pub(crate) fn rasterize_window(
    scene: &Scene,
    scale: f64,
    window: Window,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
) -> Result<RasterImage, RenderError> {
    let surface = padded_surface(window, effect_pad(scene, scale));
    let pixmap = rasterize_surface(scene, scale, surface, fonts, assets)?;
    cut(&pixmap, surface, window)
}

/// [`rasterize_window`] encoded as PNG: the bytes
/// `encode_png(rasterize_window(..))` gives, without the straight image in
/// between (one pass from the surface to the PNG pixels).
///
/// # Errors
///
/// The [`rasterize_window`] errors, and [`RenderError`] when encoding fails.
pub(crate) fn rasterize_window_png(
    scene: &Scene,
    scale: f64,
    window: Window,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
) -> Result<Vec<u8>, RenderError> {
    let surface = padded_surface(window, effect_pad(scene, scale));
    let pixmap = rasterize_surface(scene, scale, surface, fonts, assets)?;
    let rows = window_rows(&pixmap, surface, window)?;
    encode_premultiplied_rows(window.w, window.h, rows.into_iter())
}

/// `window` grown by `pad` on every side, clamped to the page and the
/// surface caps.
fn padded_surface(window: Window, pad: u32) -> Surface {
    let fit_side = |side: u32| (MAX_SURFACE_SIDE.saturating_sub(side)) / 2;
    let mut pad_x = pad.min(fit_side(window.w));
    let mut pad_y = pad.min(fit_side(window.h));
    loop {
        let surface = grow(window, pad_x, pad_y);
        let area = u64::from(surface.w) * u64::from(surface.h);
        if area <= MAX_SURFACE_PIXELS || (pad_x == 0 && pad_y == 0) {
            return surface;
        }
        pad_x /= 2;
        pad_y /= 2;
    }
}

/// `window` grown by `pad_x` / `pad_y`, clamped to the page.
fn grow(window: Window, pad_x: u32, pad_y: u32) -> Surface {
    let x0 = window.x.saturating_sub(pad_x);
    let y0 = window.y.saturating_sub(pad_y);
    let x1 = window
        .x
        .saturating_add(window.w)
        .saturating_add(pad_x)
        .min(window.page_w);
    let y1 = window
        .y
        .saturating_add(window.h)
        .saturating_add(pad_y)
        .min(window.page_h);
    Surface {
        x: x0,
        y: y0,
        w: x1.saturating_sub(x0).max(1),
        h: y1.saturating_sub(y0).max(1),
        page_w: window.page_w,
        page_h: window.page_h,
        exact: exact_page(window.page_w, window.page_h),
    }
}

/// True when the page at this scale is small enough to render whole: then a
/// region render is exact (see `surface`), and its scratch buffers stay
/// within the page.
fn exact_page(page_w: u32, page_h: u32) -> bool {
    page_w <= MAX_SURFACE_SIDE
        && page_h <= MAX_SURFACE_SIDE
        && u64::from(page_w) * u64::from(page_h) <= MAX_REGION_PIXELS
}

/// Cut `window` out of the surface pixmap and convert it to straight alpha.
fn cut(pixmap: &Pixmap, surface: Surface, window: Window) -> Result<RasterImage, RenderError> {
    let rows = window_rows(pixmap, surface, window)?;
    let mut premul = Vec::with_capacity(window.w as usize * 4 * rows.len());
    for row in rows {
        premul.extend_from_slice(row);
    }
    Ok(RasterImage {
        width: window.w,
        height: window.h,
        rgba: premultiplied_to_straight_rgba(&premul),
    })
}

/// The premultiplied rows of `window` in the surface pixmap, top to bottom.
fn window_rows(
    pixmap: &Pixmap,
    surface: Surface,
    window: Window,
) -> Result<Vec<&[u8]>, RenderError> {
    let outside = || {
        RenderError::new(format!(
            "region {}x{} at ({}, {}) leaves the {}x{} surface at ({}, {})",
            window.w, window.h, window.x, window.y, surface.w, surface.h, surface.x, surface.y
        ))
    };
    let dx = window.x.checked_sub(surface.x).ok_or_else(outside)? as usize;
    let dy = window.y.checked_sub(surface.y).ok_or_else(outside)? as usize;
    let stride = surface.w as usize * 4;
    let row_len = window.w as usize * 4;
    (0..window.h as usize)
        .map(|row| {
            let start = (dy + row) * stride + dx * 4;
            pixmap
                .data()
                .get(start..start + row_len)
                .ok_or_else(outside)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{BytesAssetProvider, default_provider};
    use zenith_scene::{Color, Paint, SceneCommand};

    fn window(x: u32, y: u32, w: u32, h: u32) -> Window {
        Window {
            x,
            y,
            w,
            h,
            page_w: 20_000,
            page_h: 20_000,
        }
    }

    #[test]
    fn pad_grows_and_clamps_to_the_page() {
        let s = padded_surface(window(5, 100, 50, 40), 10);
        assert_eq!((s.x, s.y, s.w, s.h), (0, 90, 65, 60));
    }

    #[test]
    fn pad_shrinks_to_the_side_cap() {
        let s = padded_surface(window(5000, 5000, 8000, 100), 500);
        assert!(s.w <= MAX_SURFACE_SIDE);
        assert_eq!(s.h, 1100);
    }

    /// A page too large for exact mode, with an ellipse at `at` page px.
    fn far_scene(at: f64) -> Scene {
        let mut s = Scene::new(16_000.0, 16_000.0);
        s.commands.push(SceneCommand::FillEllipse {
            x: at,
            y: at,
            w: 300.0,
            h: 200.0,
            paint: Paint::solid(Color::srgb(10, 120, 220, 255)),
            rx: None,
            ry: None,
        });
        s
    }

    /// The empirical basis of `MAX_DEVICE_EXTENT`: the same ellipse edge,
    /// once near the page origin and once 14 900 page px further, through
    /// equal windows. Up to `2^23` device px the two differ by at most four
    /// coverage steps (alpha 64) on a few edge pixels, the size of the edge
    /// clipping difference, and the ink area stays within 0.1 %.
    #[test]
    fn far_coordinates_keep_edges_up_to_the_extent_cap() {
        let fonts = default_provider();
        let assets = BytesAssetProvider::new();
        let mut scale = 1.0;
        while 16_000.0 * scale <= f64::from(1u32 << 23) {
            let page = (16_000.0 * scale) as u32;
            let at = |x: f64, y: f64| Window {
                x: (x * scale) as u32 - 128,
                y: (y * scale) as u32 - 128,
                w: 256,
                h: 256,
                page_w: page,
                page_h: page,
            };
            let near =
                rasterize_window(&far_scene(200.0), scale, at(200.0, 300.0), &fonts, &assets)
                    .expect("near");
            let far = rasterize_window(
                &far_scene(15_100.0),
                scale,
                at(15_100.0, 15_200.0),
                &fonts,
                &assets,
            )
            .expect("far");
            let mut worst = 0u8;
            let (mut ink_near, mut ink_far) = (0i64, 0i64);
            for (a, b) in near.rgba.chunks(4).zip(far.rgba.chunks(4)) {
                worst = worst.max(a[3].abs_diff(b[3]));
                ink_near += i64::from(a[3]);
                ink_far += i64::from(b[3]);
            }
            assert!(worst <= 64, "scale {scale}: alpha differs by {worst}");
            assert!(
                (ink_near - ink_far).abs() * 1000 <= ink_near,
                "scale {scale}: ink {ink_near} vs {ink_far}"
            );
            scale *= 2.0;
        }
    }

    #[test]
    fn pad_shrinks_to_the_area_cap() {
        let s = padded_surface(window(5000, 5000, 5000, 5000), 1500);
        assert!(u64::from(s.w) * u64::from(s.h) <= MAX_SURFACE_PIXELS);
        assert!(s.w > 5000, "some pad survives");
    }
}
