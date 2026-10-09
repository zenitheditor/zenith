//! Region render: one window of a page at any scale, in device pixels.
//!
//! A full-page raster allocates the whole page at the output scale, so it caps
//! the scale. A region render allocates only the window, so the scale has no
//! fixed cap. Use it to draw the visible part of a page at the exact device
//! scale (`zoom × devicePixelRatio`).
//!
//! Exactness: while the whole page at the scale fits the region caps
//! ([`MAX_REGION_SIDE`] per side, [`MAX_REGION_PIXELS`] in total), a region
//! equals the same window cut from the full-page render, byte for byte. Two
//! tiny-skia behaviors depend on where the pixmap starts, and the render
//! steps around both:
//! - tiny-skia clips a path that is not inside its pixmap: it chops edges at
//!   the border and splits curves at their extrema, so a crossing path gets
//!   different edges. A path that crosses the window edge renders on a scratch
//!   buffer that holds its whole device box.
//! - Gradient and image shaders map each pixel center back through the
//!   inverse transform in `f32`. A shader draw's scratch buffer starts at the
//!   page origin, so the pixel centers are the full render's.
//!
//! Blur, shadow, and mask feather read ink beyond the window: the surface
//! grows past the window by their reach.
//!
//! A larger page (one that has no feasible full render) skips the scratch
//! buffers, so memory stays bounded by the window. Then a path that crosses
//! the window edge can differ by a few anti-aliasing steps on its edge pixels,
//! and a shader sample by one level. There is no seam: no pixel differs by
//! more than those steps.
//!
//! [`snap_view`] turns a page-pixel view into the device rect to render.

use zenith_core::{AssetProvider, FontProvider};
use zenith_scene::Scene;

use crate::backend::RasterImage;
use crate::error::RenderError;
use crate::tiny_skia::{
    MAX_SURFACE_SIDE, Window, check_scale, page_px, rasterize_window, rasterize_window_png,
};

/// Largest region side in device pixels.
///
/// Equals tiny-skia's largest anti-aliased surface side: a larger pixmap is
/// tiled and its paths lose anti-aliasing.
pub const MAX_REGION_SIDE: u32 = MAX_SURFACE_SIDE;

/// Largest region area in device pixels: `4096 × 4096`, 64 MiB of RGBA.
pub const MAX_REGION_PIXELS: u64 = 1 << 24;

/// Largest page side at the output scale, in device pixels: `2^20`.
///
/// The render maps geometry into full-page device space in `f32`. At `2^20`
/// one `f32` step is `1/16` device px. Measured: the same edge drawn far
/// from the page origin stays within four anti-aliasing steps of the edge
/// drawn near it up to `2^23`. Past `2^24` a window origin is no longer exact
/// in `f32`. `2^20` keeps a wide margin and still allows a 20 000 px page at
/// 800 % zoom on a DPR 3 display.
pub const MAX_DEVICE_EXTENT: u32 = 1 << 20;

/// A rectangle of device pixels: the page rendered at some scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceRect {
    /// Left edge.
    pub x: u32,
    /// Top edge.
    pub y: u32,
    /// Width (at least 1).
    pub width: u32,
    /// Height (at least 1).
    pub height: u32,
}

/// A rectangle in page pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageRect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

/// Why a region render was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum RegionError {
    /// `scale` is not finite and `> 0`.
    InvalidScale {
        /// The scale received.
        scale: f64,
    },
    /// The page at `scale` exceeds [`MAX_DEVICE_EXTENT`] on an axis.
    ScaleTooLarge {
        /// The scale received.
        scale: f64,
        /// The longer page side at `scale`, in device pixels.
        extent: f64,
    },
    /// The view or device rect is not finite, has no area, or misses the
    /// page.
    InvalidView {
        /// What is wrong, with the values received.
        message: String,
    },
    /// The device rect exceeds [`MAX_REGION_SIDE`] or [`MAX_REGION_PIXELS`].
    TooLarge {
        /// Requested width in device pixels.
        width: u32,
        /// Requested height in device pixels.
        height: u32,
    },
    /// The scene has an invalid page size, or rasterizing failed.
    Render(RenderError),
}

impl RegionError {
    /// The diagnostic code: `render.invalid_scale`, `render.scale_too_large`,
    /// `render.invalid_viewport`, `render.region_too_large`, or
    /// `render.raster_failed`.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            RegionError::InvalidScale { .. } => "render.invalid_scale",
            RegionError::ScaleTooLarge { .. } => "render.scale_too_large",
            RegionError::InvalidView { .. } => "render.invalid_viewport",
            RegionError::TooLarge { .. } => "render.region_too_large",
            RegionError::Render(_) => "render.raster_failed",
        }
    }
}

impl std::fmt::Display for RegionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegionError::InvalidScale { scale } => write!(
                f,
                "invalid scale {scale}; pass a finite scale greater than 0"
            ),
            RegionError::ScaleTooLarge { scale, extent } => write!(
                f,
                "scale {scale} makes the page {extent} device px on its longer side; the most \
                 is {MAX_DEVICE_EXTENT}, so lower the scale"
            ),
            RegionError::InvalidView { message } => f.write_str(message),
            RegionError::TooLarge { width, height } => write!(
                f,
                "region {width}x{height} device px exceeds the limit of {MAX_REGION_SIDE} px per \
                 side and {MAX_REGION_PIXELS} px in total; request a smaller viewport or a \
                 lower scale"
            ),
            RegionError::Render(e) => write!(f, "render error: {e}"),
        }
    }
}

impl std::error::Error for RegionError {}

impl From<RenderError> for RegionError {
    fn from(e: RenderError) -> Self {
        RegionError::Render(e)
    }
}

/// The device size `(width, height)` of a `page_width × page_height` page at
/// `scale`: `max(1, round(side × scale))` per axis, as in
/// [`scaled_size`](crate::scaled_size), with no full-page allocation cap.
///
/// # Errors
///
/// [`RegionError::InvalidScale`] for a non-finite or non-positive scale,
/// [`RegionError::Render`] for an invalid page size, and
/// [`RegionError::ScaleTooLarge`] when a side exceeds [`MAX_DEVICE_EXTENT`].
pub fn device_page_size(
    page_width: f64,
    page_height: f64,
    scale: f64,
) -> Result<(u32, u32), RegionError> {
    check_scale(scale).map_err(|_| RegionError::InvalidScale { scale })?;
    let too_large = || RegionError::ScaleTooLarge {
        scale,
        extent: (page_width.max(page_height) * scale).round(),
    };
    let w = page_px(page_width, scale, "width")?.ok_or_else(too_large)?;
    let h = page_px(page_height, scale, "height")?.ok_or_else(too_large)?;
    if w.max(h) > MAX_DEVICE_EXTENT {
        return Err(too_large());
    }
    Ok((w, h))
}

/// The device rect that covers `view` (page pixels) at `scale`.
///
/// The origin snaps down and the far edge snaps up to whole device pixels:
/// `x0 = floor(x × scale)`, `x1 = ceil((x + width) × scale)`, the same for
/// `y`. The result is clamped to the device page (see [`device_page_size`]),
/// so a view partly off the page yields its on-page part.
///
/// # Errors
///
/// The errors of [`device_page_size`], [`RegionError::InvalidView`] for a
/// non-finite view, a non-positive size, or a view that misses the page, and
/// [`RegionError::TooLarge`] past [`MAX_REGION_SIDE`] or
/// [`MAX_REGION_PIXELS`].
pub fn snap_view(
    page_width: f64,
    page_height: f64,
    scale: f64,
    view: PageRect,
) -> Result<DeviceRect, RegionError> {
    let (dw, dh) = device_page_size(page_width, page_height, scale)?;
    let PageRect {
        x,
        y,
        width,
        height,
    } = view;
    if ![x, y, width, height].iter().all(|v| v.is_finite()) || width <= 0.0 || height <= 0.0 {
        return Err(RegionError::InvalidView {
            message: format!(
                "viewport {{x: {x}, y: {y}, w: {width}, h: {height}}} is invalid; pass finite \
                 page-px numbers with w and h greater than 0"
            ),
        });
    }
    let clamp = |v: f64, max: u32| v.max(0.0).min(f64::from(max));
    let x0 = clamp((x * scale).floor(), dw);
    let y0 = clamp((y * scale).floor(), dh);
    let x1 = clamp(((x + width) * scale).ceil(), dw);
    let y1 = clamp(((y + height) * scale).ceil(), dh);
    if x1 <= x0 || y1 <= y0 {
        return Err(RegionError::InvalidView {
            message: format!(
                "viewport {{x: {x}, y: {y}, w: {width}, h: {height}}} lies outside the \
                 {page_width}x{page_height} px page; pass a viewport that overlaps the page"
            ),
        });
    }
    let rect = DeviceRect {
        x: x0 as u32,
        y: y0 as u32,
        width: (x1 - x0) as u32,
        height: (y1 - y0) as u32,
    };
    check_size(rect)?;
    Ok(rect)
}

/// Refuse a rect past the region caps.
fn check_size(rect: DeviceRect) -> Result<(), RegionError> {
    let area = u64::from(rect.width) * u64::from(rect.height);
    if rect.width > MAX_REGION_SIDE || rect.height > MAX_REGION_SIDE || area > MAX_REGION_PIXELS {
        return Err(RegionError::TooLarge {
            width: rect.width,
            height: rect.height,
        });
    }
    Ok(())
}

/// Rasterize the device rect `rect` of `scene` at `scale` to straight-alpha
/// pixels.
///
/// `rect` is in device pixels of the page at `scale` and must lie inside it
/// (see [`snap_view`]). The output is `rect.width × rect.height` pixels and
/// deterministic. See the module docs for how it relates to the full-page
/// render.
///
/// # Errors
///
/// The errors of [`device_page_size`], [`RegionError::InvalidView`] for a rect
/// with no area or one that leaves the page, [`RegionError::TooLarge`] past the
/// caps, and [`RegionError::Render`] when rasterizing fails.
pub fn render_region_image(
    scene: &Scene,
    scale: f64,
    rect: DeviceRect,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
) -> Result<RasterImage, RegionError> {
    let window = checked_window(scene, scale, rect)?;
    Ok(rasterize_window(scene, scale, window, fonts, assets)?)
}

/// The raster window of device rect `rect`, after the checks
/// [`render_region_image`] documents.
fn checked_window(scene: &Scene, scale: f64, rect: DeviceRect) -> Result<Window, RegionError> {
    let (page_w, page_h) = device_page_size(scene.width, scene.height, scale)?;
    check_size(rect)?;
    let inside = rect.width > 0
        && rect.height > 0
        && rect.x.checked_add(rect.width).is_some_and(|r| r <= page_w)
        && rect.y.checked_add(rect.height).is_some_and(|b| b <= page_h);
    if !inside {
        return Err(RegionError::InvalidView {
            message: format!(
                "device rect {}x{} at ({}, {}) is empty or leaves the {page_w}x{page_h} device \
                 page; pass a rect from snap_view",
                rect.width, rect.height, rect.x, rect.y
            ),
        });
    }
    Ok(Window {
        x: rect.x,
        y: rect.y,
        w: rect.width,
        h: rect.height,
        page_w,
        page_h,
    })
}

/// [`render_region_image`] encoded as deterministic PNG bytes: the bytes
/// `encode_png(render_region_image(..))` gives, made in one pass from the
/// premultiplied surface.
///
/// # Errors
///
/// The errors of [`render_region_image`], and [`RegionError::Render`] when PNG
/// encoding fails.
pub fn render_region_png(
    scene: &Scene,
    scale: f64,
    rect: DeviceRect,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
) -> Result<Vec<u8>, RegionError> {
    let window = checked_window(scene, scale, rect)?;
    Ok(rasterize_window_png(scene, scale, window, fonts, assets)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(x: f64, y: f64, width: f64, height: f64) -> PageRect {
        PageRect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn snap_rounds_out_and_clamps() {
        let r = snap_view(100.0, 80.0, 1.5, view(10.2, -5.0, 20.1, 200.0)).expect("rect");
        assert_eq!(
            r,
            DeviceRect {
                x: 15,
                y: 0,
                width: 31,
                height: 120
            }
        );
    }

    #[test]
    fn snap_rejects_bad_input() {
        let codes: Vec<&str> = [
            snap_view(100.0, 80.0, 0.0, view(0.0, 0.0, 1.0, 1.0)),
            snap_view(100.0, 80.0, f64::NAN, view(0.0, 0.0, 1.0, 1.0)),
            snap_view(100.0, 80.0, 1e9, view(0.0, 0.0, 1.0, 1.0)),
            snap_view(100.0, 80.0, 1.0, view(0.0, 0.0, 0.0, 1.0)),
            snap_view(100.0, 80.0, 1.0, view(f64::NAN, 0.0, 1.0, 1.0)),
            snap_view(100.0, 80.0, 1.0, view(200.0, 0.0, 10.0, 10.0)),
            snap_view(10_000.0, 10_000.0, 40.0, view(0.0, 0.0, 300.0, 300.0)),
        ]
        .iter()
        .map(|r| r.as_ref().map_err(RegionError::code).err().unwrap_or("ok"))
        .collect();
        assert_eq!(
            codes,
            [
                "render.invalid_scale",
                "render.invalid_scale",
                "render.scale_too_large",
                "render.invalid_viewport",
                "render.invalid_viewport",
                "render.invalid_viewport",
                "render.region_too_large",
            ]
        );
    }
}
