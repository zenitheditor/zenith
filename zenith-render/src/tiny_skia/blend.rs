//! Source-over of a premultiplied pixmap at an integer offset, with the
//! arithmetic of tiny-skia's highp pipeline, without the pipeline.
//!
//! `Pixmap::draw_pixmap` at an integer offset with `PixmapPaint::default()`
//! (source-over, opacity 1) runs the stages seed shader, transform, gather,
//! and `source_over_rgba` over every pixel of the source rect. Per channel:
//!
//! ```text
//! s = src as f32 * (1 / 255)      d = dst as f32 * (1 / 255)
//! out = d * (1 - sa) + s          (f32 multiply, then add; no fused multiply-add)
//! byte = round_ties_even(clamp(out, 0, 1) * 255)
//! ```
//!
//! [`source_over_at`] computes exactly that per pixel, so the bytes equal
//! `draw_pixmap`'s on every target (tiny-skia rounds ties to even on every
//! platform: SSE `cvtps2dq`, wasm `nearest`, and its scalar fallback).
//! The test `source_over_matches_draw_pixmap_for_every_channel_triple`
//! checks every (destination, source, source alpha) triple against
//! tiny-skia.

use tiny_skia::Pixmap;

/// `1 / 255` as tiny-skia's highp `load_8888` multiplies by it.
const FACTOR: f32 = 1.0 / 255.0;

/// `2^23`. For `v` in `[0, 2^22)`, `v + 2^23` rounds `v` to the nearest
/// integer, ties to even (the default IEEE rounding of the add), and that
/// integer is the low mantissa bits of the sum. No call to `rintf`, no
/// float-to-int conversion.
const ROUND: f32 = 8_388_608.0;

/// One channel: `d * (1 - sa) + s`, back to a byte.
#[inline(always)]
fn channel(d: u8, s: u8, inv_sa: f32) -> u8 {
    let out = f32::from(d) * FACTOR * inv_sa + f32::from(s) * FACTOR;
    let v = out.clamp(0.0, 1.0) * 255.0;
    // `v` is in [0, 255]: the sum is `2^23 + round_ties_even(v)`, and the
    // low byte of its bits is the rounded value.
    (v + ROUND).to_bits() as u8
}

/// Source-over of one premultiplied pixel.
#[inline(always)]
fn blend_pixel(d: &mut [u8; 4], s: &[u8; 4]) {
    let inv_sa = 1.0 - f32::from(s[3]) * FACTOR;
    for (dc, &sc) in d.iter_mut().zip(s) {
        *dc = channel(*dc, sc, inv_sa);
    }
}

/// Source-over of four premultiplied pixels (16 bytes).
#[inline(always)]
fn blend_block(d: &mut [u8; 16], s: &[u8; 16]) {
    let mut inv = [0.0f32; 16];
    for (lanes, px) in inv
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(s.as_chunks::<4>().0)
    {
        *lanes = [1.0 - f32::from(px[3]) * FACTOR; 4];
    }
    for ((dc, &sc), &ia) in d.iter_mut().zip(s).zip(&inv) {
        *dc = channel(*dc, sc, ia);
    }
}

/// Source-over `src` onto `target` with its top-left pixel at (`x`, `y`).
///
/// The source must lie inside `target` (`x + width <= target width`, the
/// same for rows); returns `false` and draws nothing otherwise. A fully
/// transparent source pixel leaves the destination as it is, which is what
/// the formula gives for it (see the test `transparent_source_keeps_every_destination_byte`).
pub(super) fn source_over_at(target: &mut Pixmap, src: &Pixmap, x: u32, y: u32) -> bool {
    let (tw, th) = (target.width() as usize, target.height() as usize);
    let (sw, sh) = (src.width() as usize, src.height() as usize);
    let (x, y) = (x as usize, y as usize);
    if x.checked_add(sw).is_none_or(|r| r > tw) || y.checked_add(sh).is_none_or(|b| b > th) {
        return false;
    }
    let (t_stride, s_stride) = (tw * 4, sw * 4);
    let target_rows = target.data_mut().chunks_exact_mut(t_stride).skip(y);
    for (t_row, s_row) in target_rows.zip(src.data().chunks_exact(s_stride)) {
        let Some(t_span) = t_row.get_mut(x * 4..x * 4 + s_stride) else {
            return false;
        };
        // Four pixels per step, all 16 lanes at once, so the step vectorizes.
        let (d_blocks, d_tail) = t_span.as_chunks_mut::<16>();
        let (s_blocks, s_tail) = s_row.as_chunks::<16>();
        for (d, s) in d_blocks.iter_mut().zip(s_blocks) {
            if u128::from_ne_bytes(*s) != 0 {
                blend_block(d, s);
            }
        }
        let pixels = d_tail
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(s_tail.as_chunks::<4>().0);
        for (d, s) in pixels {
            if u32::from_ne_bytes(*s) != 0 {
                blend_pixel(d, s);
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::{PixmapPaint, Transform};

    /// A 256 × 256 pixmap whose pixel (x, y) is `f(x, y)` in every channel
    /// but alpha, and `alpha(x, y)` in alpha.
    fn grid(f: impl Fn(u8, u8) -> u8, alpha: impl Fn(u8, u8) -> u8) -> Pixmap {
        let mut pm = Pixmap::new(256, 256).expect("alloc");
        for (i, px) in pm.data_mut().as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (x, y) = ((i % 256) as u8, (i / 256) as u8);
            let v = f(x, y);
            *px = [v, v, v, alpha(x, y)];
        }
        pm
    }

    #[test]
    fn source_over_matches_draw_pixmap_for_every_channel_triple() {
        // Destination byte d along x (all four channels). Source: alpha sa
        // fixed per round, color s along y with s <= sa (premultiplied).
        let dst = grid(|x, _| x, |x, _| x);
        for sa in 0..=255u8 {
            let src = grid(|_, y| y.min(sa), |_, _| sa);
            let mut want = dst.clone();
            want.draw_pixmap(
                0,
                0,
                src.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
            let mut got = dst.clone();
            assert!(source_over_at(&mut got, &src, 0, 0));
            assert!(got.data() == want.data(), "source alpha {sa} differs");
        }
    }

    #[test]
    fn magic_rounding_is_round_ties_even_on_the_byte_range() {
        // Every f32 in [0, 255] that is a multiple of 2^-12 (ties included).
        for k in 0..=255 * 4096u32 {
            let v = k as f32 / 4096.0;
            assert_eq!((v + ROUND) - ROUND, v.round_ties_even(), "{v}");
            assert_eq!(
                u32::from((v + ROUND).to_bits() as u8),
                v.round_ties_even() as u32,
                "{v}"
            );
        }
    }

    #[test]
    fn transparent_source_keeps_every_destination_byte() {
        for d in 0..=255u8 {
            assert_eq!(channel(d, 0, 1.0), d);
        }
    }

    #[test]
    fn offset_draw_matches_draw_pixmap_and_refuses_overflow() {
        let dst = grid(|x, y| x ^ y, |x, y| x.max(y));
        let src = {
            let mut pm = Pixmap::new(40, 30).expect("alloc");
            for (i, px) in pm.data_mut().as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let a = (i * 37 % 256) as u8;
                let c = (i * 11 % 256) as u8;
                *px = [c.min(a), (c / 2).min(a), 0, a];
            }
            pm
        };
        for (x, y) in [(0, 0), (7, 3), (216, 226)] {
            let mut want = dst.clone();
            want.draw_pixmap(
                x as i32,
                y as i32,
                src.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
            let mut got = dst.clone();
            assert!(source_over_at(&mut got, &src, x, y));
            assert!(got.data() == want.data(), "offset ({x}, {y}) differs");
        }
        let mut got = dst.clone();
        assert!(!source_over_at(&mut got, &src, 217, 0));
        assert!(!source_over_at(&mut got, &src, 0, 227));
        assert!(got.data() == dst.data(), "a refused draw changed pixels");
    }
}
