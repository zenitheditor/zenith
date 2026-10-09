//! Pixel-format conversion and PNG encoding for the tiny-skia backend.
//!
//! The PNG bytes match the former path exactly. That path re-premultiplied the
//! straight image, copied it into a `Pixmap`, and called
//! `tiny_skia::Pixmap::encode_png`, which demultiplied it again. Both lossy hops
//! fold into one 256×256 lookup table indexed by (alpha, channel).

use std::sync::OnceLock;

use super::pixels::premultiplied_to_straight;
use crate::backend::RasterImage;
use crate::error::RenderError;

/// A per-(alpha, channel) byte mapping. Every `u8` pair indexes in range.
struct ChannelLut([[u8; 256]; 256]);

impl ChannelLut {
    fn build(f: impl Fn(u8, u8) -> u8) -> Self {
        let mut table = [[0u8; 256]; 256];
        for (alpha, row) in (0..=u8::MAX).zip(table.iter_mut()) {
            for (value, cell) in (0..=u8::MAX).zip(row.iter_mut()) {
                *cell = f(alpha, value);
            }
        }
        Self(table)
    }

    fn row(&self, alpha: u8) -> &[u8; 256] {
        &self.0[usize::from(alpha)]
    }
}

/// Premultiplied channel → straight channel, per [`premultiplied_to_straight`].
fn straight_lut() -> &'static ChannelLut {
    static LUT: OnceLock<ChannelLut> = OnceLock::new();
    LUT.get_or_init(|| {
        ChannelLut::build(|alpha, value| premultiplied_to_straight(value, 0, 0, alpha).0)
    })
}

/// Straight channel → the byte tiny-skia's encoder wrote to the PNG.
fn png_lut() -> &'static ChannelLut {
    static LUT: OnceLock<ChannelLut> = OnceLock::new();
    LUT.get_or_init(|| ChannelLut::build(png_channel))
}

/// The former encode path for one straight channel at one alpha.
///
/// Hop 1 re-premultiplies with `(v * a + 127) / 255`, and alpha 0 gives 0.
/// Hop 2 is tiny-skia 0.11.4 `PremultipliedColorU8::demultiply`. Opaque
/// pixels pass through. Others compute `(p as f64 / (a / 255.0) + 0.5) as u8`.
fn png_channel(alpha: u8, straight: u8) -> u8 {
    if alpha == 0 {
        return 0;
    }
    let premul = ((u16::from(straight) * u16::from(alpha) + 127) / 255).min(255) as u8;
    if alpha == u8::MAX {
        return premul;
    }
    let a = f64::from(alpha) / 255.0;
    (f64::from(premul) / a + 0.5) as u8
}

/// Premultiplied channel → the byte the PNG holds: [`straight_lut`] then
/// [`png_lut`] folded into one table (a composition of two tables is exact).
fn premul_png_lut() -> &'static ChannelLut {
    static LUT: OnceLock<ChannelLut> = OnceLock::new();
    LUT.get_or_init(|| {
        ChannelLut::build(|alpha, value| {
            png_channel(alpha, premultiplied_to_straight(value, 0, 0, alpha).0)
        })
    })
}

/// Encode premultiplied RGBA8 `rows` (each `width * 4` bytes, `height` of
/// them) to the PNG bytes [`encode_straight_png`] gives for their straight
/// image, in one pass and one pixel buffer.
pub(super) fn encode_premultiplied_rows<'a>(
    width: u32,
    height: u32,
    rows: impl Iterator<Item = &'a [u8]>,
) -> Result<Vec<u8>, RenderError> {
    let lut = premul_png_lut();
    let row_len = width as usize * 4;
    let mut pixels = vec![0u8; row_len * height as usize];
    let mut filled = 0;
    for (dst, row) in pixels.chunks_exact_mut(row_len.max(1)).zip(rows) {
        if row.len() != row_len {
            return Err(RenderError::new(
                "pixel row length mismatch during PNG encoding",
            ));
        }
        map_rgb_into(dst, row, lut);
        filled += 1;
    }
    if filled != height as usize {
        return Err(RenderError::new(
            "pixel buffer length mismatch during PNG encoding",
        ));
    }
    write_png(width, height, &pixels)
}

/// Convert premultiplied RGBA8 to straight RGBA8. Alpha passes through.
pub(super) fn premultiplied_to_straight_rgba(premul: &[u8]) -> Vec<u8> {
    map_rgb(premul, straight_lut())
}

fn map_rgb(src: &[u8], lut: &ChannelLut) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    map_rgb_into(&mut out, src, lut);
    out
}

/// [`map_rgb`] into `out` (as long as `src`).
fn map_rgb_into(out: &mut [u8], src: &[u8], lut: &ChannelLut) {
    for (dst, px) in out
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(src.as_chunks::<4>().0.iter())
    {
        let ([r, g, b, a], [dr, dg, db, da]) = (px, dst);
        let row = lut.row(*a);
        *dr = row[usize::from(*r)];
        *dg = row[usize::from(*g)];
        *db = row[usize::from(*b)];
        *da = *a;
    }
}

/// Encode a straight-alpha image to PNG bytes.
///
/// The encoder settings match tiny-skia 0.11.4 `PixmapRef::encode_png`:
/// RGBA, 8-bit, and the `png` crate defaults for everything else.
pub(super) fn encode_straight_png(image: &RasterImage) -> Result<Vec<u8>, RenderError> {
    let expected = u64::from(image.width) * u64::from(image.height) * 4;
    if image.width == 0 || image.height == 0 {
        return Err(RenderError::new(format!(
            "failed to allocate pixmap for encoding ({}×{})",
            image.width, image.height
        )));
    }
    if u64::try_from(image.rgba.len()).ok() != Some(expected) {
        return Err(RenderError::new(
            "pixel buffer length mismatch during PNG encoding",
        ));
    }

    let pixels = map_rgb(&image.rgba, png_lut());
    write_png(image.width, image.height, &pixels)
}

/// Write PNG-ready RGBA8 `pixels` as PNG bytes, with the settings of
/// [`encode_straight_png`].
fn write_png(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, RenderError> {
    if width == 0 || height == 0 {
        return Err(RenderError::new(format!(
            "failed to allocate pixmap for encoding ({width}×{height})"
        )));
    }
    let png_err = |e: png::EncodingError| RenderError::new(format!("PNG encoding failed: {e}"));
    let mut data = Vec::new();
    let mut encoder = png::Encoder::new(&mut data, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(png_err)?;
    writer.write_image_data(pixels).map_err(png_err)?;
    writer.finish().map_err(png_err)?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::Pixmap;

    /// The former `encode_png` path, kept verbatim as the byte reference.
    fn reference_encode_png(image: &RasterImage) -> Vec<u8> {
        let mut premul = Vec::with_capacity(image.rgba.len());
        for chunk in image.rgba.as_chunks::<4>().0 {
            let (r, g, b, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
            if a == 0 {
                premul.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                let a_u16 = u16::from(a);
                let mul = |v: u8| -> u8 {
                    let result = (u16::from(v) * a_u16 + 127) / 255;
                    result.min(255) as u8
                };
                premul.push(mul(r));
                premul.push(mul(g));
                premul.push(mul(b));
                premul.push(a);
            }
        }
        let mut pixmap = Pixmap::new(image.width, image.height).unwrap();
        pixmap.data_mut().copy_from_slice(&premul);
        pixmap.encode_png().unwrap()
    }

    /// The former per-pixel straight conversion in `rasterize`.
    fn reference_straight(premul: &[u8]) -> Vec<u8> {
        let mut rgba = Vec::with_capacity(premul.len());
        for c in premul.as_chunks::<4>().0 {
            let (r, g, b, a) = premultiplied_to_straight(c[0], c[1], c[2], c[3]);
            rgba.extend_from_slice(&[r, g, b, a]);
        }
        rgba
    }

    /// Full pipeline from a premultiplied pixmap to PNG, old vs new.
    fn assert_pipeline_identical(width: u32, height: u32, premul: &[u8]) {
        let old_straight = reference_straight(premul);
        let new_straight = premultiplied_to_straight_rgba(premul);
        assert_eq!(old_straight, new_straight);
        let image = RasterImage {
            width,
            height,
            rgba: new_straight,
        };
        assert_eq!(
            reference_encode_png(&image),
            encode_straight_png(&image).unwrap()
        );
    }

    #[test]
    fn opaque_image_png_bytes_identical() {
        let (w, h) = (37u32, 23u32);
        let premul: Vec<u8> = (0..w * h)
            .flat_map(|i| [(i * 7) as u8, (i * 13) as u8, (i * 29) as u8, 255])
            .collect();
        assert_pipeline_identical(w, h, &premul);
    }

    #[test]
    fn transparent_image_png_bytes_identical() {
        let (w, h) = (19u32, 11u32);
        assert_pipeline_identical(w, h, &vec![0u8; (w * h * 4) as usize]);
    }

    #[test]
    fn every_alpha_value_pair_png_bytes_identical() {
        // Pixel (x, y): alpha = y, premul channels <= alpha.
        // Red covers every value in 0..=alpha. Green and blue are scrambled.
        let mut premul = Vec::with_capacity(256 * 256 * 4);
        let mut seed: u32 = 0x9E37_79B9;
        for y in 0..=255u32 {
            for x in 0..=255u32 {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let span = y + 1;
                let r = x % span;
                let g = (seed >> 8) % span;
                let b = (seed >> 20) % span;
                premul.extend_from_slice(&[r as u8, g as u8, b as u8, y as u8]);
            }
        }
        assert_pipeline_identical(256, 256, &premul);
    }

    #[test]
    fn every_alpha_straight_pair_png_bytes_identical() {
        // Straight input covering every (alpha, value) pair, including
        // straight images not produced by `rasterize`.
        let rgba: Vec<u8> = (0..=255u8)
            .flat_map(|a| (0..=255u8).flat_map(move |v| [v, 255 - v, v ^ 0x5A, a]))
            .collect();
        let image = RasterImage {
            width: 256,
            height: 256,
            rgba,
        };
        assert_eq!(
            reference_encode_png(&image),
            encode_straight_png(&image).unwrap()
        );
    }

    #[test]
    fn luts_match_per_pixel_functions_for_all_inputs() {
        for a in 0..=255u8 {
            for v in 0..=255u8 {
                assert_eq!(
                    straight_lut().row(a)[usize::from(v)],
                    premultiplied_to_straight(v, v, v, a).0
                );
                // Composed: re-premultiply, then tiny-skia demultiply.
                let px = reference_encode_pixel(v, a);
                assert_eq!(png_lut().row(a)[usize::from(v)], px[0], "a={a} v={v}");
                assert_eq!(px[3], a);
            }
        }
    }

    /// One channel through the former re-premultiply and tiny-skia demultiply.
    fn reference_encode_pixel(v: u8, a: u8) -> [u8; 4] {
        let p = if a == 0 {
            0
        } else {
            ((u16::from(v) * u16::from(a) + 127) / 255).min(255) as u8
        };
        let c = tiny_skia::PremultipliedColorU8::from_rgba(p, p, p, a)
            .unwrap()
            .demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }

    #[test]
    fn rejects_length_mismatch() {
        let image = RasterImage {
            width: 2,
            height: 2,
            rgba: vec![0; 12],
        };
        assert!(encode_straight_png(&image).is_err());
    }
}
