//! Raster image → PDF image XObject encoding.
//!
//! The asset bytes (PNG or JPEG) are decoded to straight-alpha RGBA8 via the
//! same decoder the raster backend uses, then split into a FlateDecode RGB
//! image XObject plus, when any pixel is non-opaque, a FlateDecode DeviceGray
//! `/SMask` carrying the alpha channel. Re-encoding to Flate (rather than
//! passing JPEGs through as DCTDecode) keeps a single, uniform, deterministic
//! path for both formats. miniz_oxide deflate is deterministic for fixed input.

use miniz_oxide::deflate::compress_to_vec_zlib;

use zenith_scene::SrcRect;

use super::color::rgb_to_cmyk;
use crate::tiny_skia::{crop_raster_image, decode_raster_to_pixmap};

/// A decoded image ready to embed: deflated RGB or CMYK samples, the optional
/// deflated alpha SMask, and the pixel dimensions.
pub(super) struct DecodedImage {
    pub(super) width: u32,
    pub(super) height: u32,
    /// `true` when `rgb_flate` stores 4-channel DeviceCMYK8 samples; `false` for DeviceRGB8.
    pub(super) cmyk: bool,
    /// zlib-deflated interleaved RGB8 or CMYK8 samples (row-major, top-to-bottom).
    pub(super) rgb_flate: Vec<u8>,
    /// zlib-deflated DeviceGray8 alpha samples; `None` when the image is fully
    /// opaque (no `/SMask` needed).
    pub(super) alpha_flate: Option<Vec<u8>>,
}

/// Decode `bytes` (PNG or JPEG) and prepare it for embedding, or `None` when the
/// format is unsupported / the data is malformed / dimensions are zero.
pub(super) fn decode_for_pdf(
    bytes: &[u8],
    crop: Option<&SrcRect>,
    cmyk: bool,
) -> Option<DecodedImage> {
    // Reuse the raster decoder (PNG via tiny-skia, JPEG via jpeg-decoder). The
    // returned Pixmap holds premultiplied RGBA; convert back to straight alpha
    // so the color plane and the SMask are independent (PDF composites them).
    let pixmap = crop_raster_image(decode_raster_to_pixmap(bytes)?, crop)?;
    let width = pixmap.width();
    let height = pixmap.height();
    if width == 0 || height == 0 {
        return None;
    }
    // Demultiply each pixel to straight (un-premultiplied) RGBA8, then defer to
    // the shared splitter so the PDF image path is identical to the one used for
    // rasterized filter regions.
    let px = pixmap.pixels();
    let mut straight = Vec::with_capacity(px.len() * 4);
    for p in px {
        let a = p.alpha();
        let s = p.demultiply();
        straight.push(s.red());
        straight.push(s.green());
        straight.push(s.blue());
        straight.push(a);
    }
    decoded_image_from_straight_rgba(&straight, width, height, cmyk)
}

/// Build a [`DecodedImage`] from a STRAIGHT-alpha RGBA8 buffer (row-major,
/// top-to-bottom), splitting it into a deflated interleaved RGB8 (or CMYK8 when
/// `cmyk` is `true`) plane plus an optional deflated DeviceGray8 alpha SMask.
/// Returns `None` when the dimensions are zero or `rgba` is not exactly
/// `width * height * 4` bytes.
///
/// This is the single splitter shared by [`decode_for_pdf`] (after it demultiplies
/// its decoded Pixmap) and the PDF backend's rasterize-and-embed filter path. The
/// alpha SMask is omitted (`None`) when every pixel is fully opaque, matching the
/// decode path byte-for-byte.
pub(super) fn decoded_image_from_straight_rgba(
    rgba: &[u8],
    width: u32,
    height: u32,
    cmyk: bool,
) -> Option<DecodedImage> {
    if width == 0 || height == 0 {
        return None;
    }
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))?;
    if rgba.len() != expected {
        return None;
    }

    let pixel_count = rgba.len() / 4;
    let mut color_samples = Vec::with_capacity(pixel_count * if cmyk { 4 } else { 3 });
    let mut alpha = Vec::with_capacity(pixel_count);
    let mut any_transparent = false;
    for [r, g, b, a] in rgba.as_chunks::<4>().0 {
        if cmyk {
            let [c, m, y, k] = rgb_to_cmyk(
                f32::from(*r) / 255.0,
                f32::from(*g) / 255.0,
                f32::from(*b) / 255.0,
            );
            color_samples.push((c * 255.0).round() as u8);
            color_samples.push((m * 255.0).round() as u8);
            color_samples.push((y * 255.0).round() as u8);
            color_samples.push((k * 255.0).round() as u8);
        } else {
            color_samples.push(*r);
            color_samples.push(*g);
            color_samples.push(*b);
        }
        alpha.push(*a);
        if *a != 255 {
            any_transparent = true;
        }
    }

    let level = 6; // miniz_oxide default; deterministic for fixed input.
    let rgb_flate = compress_to_vec_zlib(&color_samples, level);
    let alpha_flate = if any_transparent {
        Some(compress_to_vec_zlib(&alpha, level))
    } else {
        None
    };

    Some(DecodedImage {
        width,
        height,
        cmyk,
        rgb_flate,
        alpha_flate,
    })
}
