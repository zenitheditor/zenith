//! Raster image decoding (PNG, JPEG) into premultiplied `Pixmap`s.

use tiny_skia::{IntRect, Pixmap};
use zenith_scene::SrcRect;

/// Decode a raster image asset into a premultiplied `Pixmap`.
///
/// Supports PNG (via tiny-skia's built-in decoder) and baseline/progressive
/// JPEG (via `jpeg-decoder`). Returns `None` for unsupported formats or
/// malformed data, in which case the caller skips drawing the asset.
/// Deterministic: pixel output depends only on the input bytes.
pub(crate) fn decode_raster_image(bytes: &[u8]) -> Option<Pixmap> {
    // PNG signature.
    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return Pixmap::decode_png(bytes).ok();
    }
    // JPEG SOI marker.
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return decode_jpeg(bytes);
    }
    None
}

/// Clamp and truncate source-crop endpoints before copying premultiplied pixels.
/// Absent crops return the original pixmap. Empty crops skip the image.
pub(crate) fn crop_raster_image(decoded: Pixmap, crop: Option<&SrcRect>) -> Option<Pixmap> {
    let Some(sr) = crop else {
        return Some(decoded);
    };
    let rect = crop_raster_rect((decoded.width(), decoded.height()), sr)?;
    decoded.as_ref().clone_rect(rect)
}

/// Resolve effective crop dimensions without copying raster pixels.
pub(crate) fn crop_raster_rect(dimensions: (u32, u32), sr: &SrcRect) -> Option<IntRect> {
    let (rx, ry, rw, rh) = (sr.x, sr.y, sr.w, sr.h);
    let src_w = dimensions.0 as f64;
    let src_h = dimensions.1 as f64;
    let cx = rx.max(0.0).min(src_w) as i32;
    let cy = ry.max(0.0).min(src_h) as i32;
    let cx2 = (rx + rw).max(0.0).min(src_w) as i32;
    let cy2 = (ry + rh).max(0.0).min(src_h) as i32;
    let cw = (cx2 - cx).max(0) as u32;
    let ch = (cy2 - cy).max(0) as u32;
    if cw == 0 || ch == 0 {
        return None;
    }
    IntRect::from_xywh(cx, cy, cw, ch)
}

/// Decode a JPEG into an opaque premultiplied `Pixmap`. Handles RGB24 and L8
/// (grayscale) pixel formats; other formats (L16, CMYK32) return `None`.
fn decode_jpeg(bytes: &[u8]) -> Option<Pixmap> {
    use jpeg_decoder::{Decoder, PixelFormat};
    use tiny_skia::PremultipliedColorU8;

    let mut decoder = Decoder::new(std::io::Cursor::new(bytes));
    let pixels = decoder.decode().ok()?;
    let info = decoder.info()?;
    let mut pixmap = Pixmap::new(u32::from(info.width), u32::from(info.height))?;
    let dst = pixmap.pixels_mut();

    match info.pixel_format {
        PixelFormat::RGB24 => {
            for ([r, g, b], px) in pixels.as_chunks::<3>().0.iter().zip(dst.iter_mut()) {
                // Opaque source: premultiplied == straight at alpha 255.
                *px = PremultipliedColorU8::from_rgba(*r, *g, *b, 255)?;
            }
        }
        PixelFormat::L8 => {
            for (v, px) in pixels.iter().zip(dst.iter_mut()) {
                *px = PremultipliedColorU8::from_rgba(*v, *v, *v, 255)?;
            }
        }
        _ => return None,
    }
    Some(pixmap)
}
