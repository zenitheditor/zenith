//! The measured area of a page raster: the trim box crop, its ink, and its
//! linear-light surface.

use std::collections::BTreeMap;

use zenith_raster::{LinearRgba, Surface, decode_srgb_u8};

use crate::error::PipelineError;

/// Straight-alpha RGBA8 pixels of the measured area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Crop {
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// `width * height * 4` bytes, row-major.
    pub(crate) rgba: Vec<u8>,
}

/// A pixel rectangle `[x, x + width) × [y, y + height)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PixelRect {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

/// The pixels that differ from the most common pixel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Ink {
    /// The most common pixel, RGBA8. A tie takes the lowest packed value.
    pub(crate) background: [u8; 4],
    /// The number of pixels that differ from `background`.
    pub(crate) pixel_count: u64,
    /// `(min_x, min_y, max_x, max_y)` pixel edges, max exclusive.
    pub(crate) bounds: Option<(u32, u32, u32, u32)>,
}

/// Copy `rect` out of a `width × height` RGBA8 image, clipped to the image.
///
/// # Errors
///
/// `perceive.raster_failed` (exit 2) when `rgba` is shorter than
/// `width * height * 4` bytes or the clipped area is empty.
pub(crate) fn crop(
    rgba: &[u8],
    width: u32,
    height: u32,
    rect: PixelRect,
) -> Result<Crop, PipelineError> {
    let x0 = rect.x.min(width);
    let y0 = rect.y.min(height);
    let x1 = rect.x.saturating_add(rect.width).min(width);
    let y1 = rect.y.saturating_add(rect.height).min(height);
    if x1 <= x0 || y1 <= y0 {
        return Err(raster_error(
            "the trim box covers no pixel of the page raster",
        ));
    }
    let row_bytes = width as usize * 4;
    if rgba.len() < row_bytes * height as usize {
        return Err(raster_error("the page raster is shorter than its size"));
    }
    let mut out = Vec::with_capacity((x1 - x0) as usize * (y1 - y0) as usize * 4);
    for y in y0..y1 {
        let start = y as usize * row_bytes + x0 as usize * 4;
        let end = y as usize * row_bytes + x1 as usize * 4;
        let row = rgba
            .get(start..end)
            .ok_or_else(|| raster_error("the page raster is shorter than its size"))?;
        out.extend_from_slice(row);
    }
    Ok(Crop {
        width: x1 - x0,
        height: y1 - y0,
        rgba: out,
    })
}

/// The ink of `crop`.
pub(crate) fn ink(crop: &Crop) -> Ink {
    let mut counts: BTreeMap<u32, u64> = BTreeMap::new();
    for px in crop.rgba.as_chunks::<4>().0 {
        *counts.entry(u32::from_be_bytes(*px)).or_insert(0) += 1;
    }
    let mut background = 0_u32;
    let mut best = 0_u64;
    for (&value, &count) in &counts {
        if count > best {
            best = count;
            background = value;
        }
    }
    let mut pixel_count = 0_u64;
    let mut bounds: Option<(u32, u32, u32, u32)> = None;
    let width = crop.width.max(1) as usize;
    for (index, px) in crop.rgba.as_chunks::<4>().0.iter().enumerate() {
        if u32::from_be_bytes(*px) == background {
            continue;
        }
        pixel_count += 1;
        let x = u32::try_from(index % width).unwrap_or(u32::MAX);
        let y = u32::try_from(index / width).unwrap_or(u32::MAX);
        bounds = Some(match bounds {
            None => (x, y, x.saturating_add(1), y.saturating_add(1)),
            Some((x0, y0, x1, y1)) => (
                x0.min(x),
                y0.min(y),
                x1.max(x.saturating_add(1)),
                y1.max(y.saturating_add(1)),
            ),
        });
    }
    Ink {
        background: background.to_be_bytes(),
        pixel_count,
        bounds,
    }
}

/// `crop` as premultiplied linear-light pixels.
///
/// Each 8-bit channel decodes through [`decode_srgb_u8`], which uses the
/// deterministic `zenith_geometry::math` power function.
///
/// # Errors
///
/// `perceive.raster_failed` (exit 2) when the surface cannot be built.
pub(crate) fn surface(crop: &Crop) -> Result<Surface, PipelineError> {
    let mut decode = [0.0_f32; 256];
    for (value, slot) in decode.iter_mut().enumerate() {
        *slot = decode_srgb_u8(u8::try_from(value).unwrap_or(u8::MAX));
    }
    let channel = |byte: u8| decode.get(usize::from(byte)).copied().unwrap_or(0.0);
    let mut pixels = Vec::with_capacity(crop.rgba.len() / 4);
    for &[r, g, b, a] in crop.rgba.as_chunks::<4>().0 {
        let pixel = LinearRgba::straight(channel(r), channel(g), channel(b), f32::from(a) / 255.0)
            .map_err(|e| raster_error(&format!("invalid pixel: {e:?}")))?;
        pixels.push(pixel);
    }
    Surface::from_pixels(crop.width, crop.height, pixels)
        .map_err(|e| raster_error(&format!("cannot build the surface: {e:?}")))
}

/// Lowercase `#rrggbbaa` of `pixel`.
pub(crate) fn hex(pixel: [u8; 4]) -> String {
    let [r, g, b, a] = pixel;
    format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
}

fn raster_error(message: &str) -> PipelineError {
    PipelineError::new(
        "perceive.raster_failed",
        format!("perceive error: {message}"),
        2,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(pixels: &[[u8; 4]]) -> Vec<u8> {
        pixels.iter().flatten().copied().collect()
    }

    const W: [u8; 4] = [255, 255, 255, 255];
    const K: [u8; 4] = [0, 0, 0, 255];

    #[test]
    fn crop_copies_the_clipped_rectangle() {
        let rgba = image(&[W, K, W, W, W, K]);
        let out = crop(
            &rgba,
            3,
            2,
            PixelRect {
                x: 1,
                y: 0,
                width: 5,
                height: 5,
            },
        )
        .unwrap();
        assert_eq!((out.width, out.height), (2, 2));
        assert_eq!(out.rgba, image(&[K, W, W, K]));
    }

    #[test]
    fn crop_rejects_an_empty_area() {
        let rgba = image(&[W]);
        let rect = PixelRect {
            x: 1,
            y: 0,
            width: 1,
            height: 1,
        };
        assert!(crop(&rgba, 1, 1, rect).is_err());
    }

    #[test]
    fn ink_bounds_cover_pixels_that_differ_from_the_most_common_pixel() {
        let crop = Crop {
            width: 3,
            height: 3,
            rgba: image(&[W, W, W, W, K, K, W, W, W]),
        };
        let ink = ink(&crop);
        assert_eq!(ink.background, W);
        assert_eq!(ink.pixel_count, 2);
        assert_eq!(ink.bounds, Some((1, 1, 3, 2)));
    }

    #[test]
    fn ink_tie_takes_the_lowest_pixel_and_flat_pages_have_no_bounds() {
        let tie = ink(&Crop {
            width: 2,
            height: 1,
            rgba: image(&[W, K]),
        });
        assert_eq!(tie.background, K);
        let flat = ink(&Crop {
            width: 2,
            height: 1,
            rgba: image(&[W, W]),
        });
        assert_eq!(flat.pixel_count, 0);
        assert_eq!(flat.bounds, None);
    }

    #[test]
    fn surface_decodes_srgb_to_linear() {
        let s = surface(&Crop {
            width: 2,
            height: 1,
            rgba: image(&[W, [0, 0, 0, 0]]),
        })
        .unwrap();
        assert_eq!(s.pixels()[0].r(), 1.0);
        assert_eq!(s.pixels()[1].a(), 0.0);
    }

    #[test]
    fn hex_is_lowercase_rgba() {
        assert_eq!(hex([255, 10, 0, 128]), "#ff0a0080");
    }
}
