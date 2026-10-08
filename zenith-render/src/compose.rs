//! Deterministic source-over placement of one raster image onto another.

use crate::backend::RasterImage;
use crate::error::RenderError;

/// Composite `src` over `dst` with its top-left corner at pixel `(x, y)`.
///
/// Both images are straight-alpha RGBA8. Each pixel uses integer source-over
/// with fixed rounding, so output is identical on every machine:
///
/// - `dw = round(da × (255 − sa) / 255)` (backdrop weight)
/// - `a_out = sa + dw`
/// - `c_out = round((sc × sa + dc × dw) / a_out)`
///
/// An opaque source pixel is copied as is. A transparent one leaves `dst`
/// unchanged. The part of `src` that falls outside `dst` is skipped.
///
/// # Errors
///
/// Returns [`RenderError`] when either buffer is shorter than its declared
/// `width × height × 4` bytes.
pub fn composite_over(
    dst: &mut RasterImage,
    src: &RasterImage,
    x: u32,
    y: u32,
) -> Result<(), RenderError> {
    check_len(dst, "destination")?;
    check_len(src, "source")?;
    if x >= dst.width || y >= dst.height {
        return Ok(());
    }
    let cols = src.width.min(dst.width - x) as usize;
    let rows = src.height.min(dst.height - y) as usize;
    let src_stride = src.width as usize * 4;
    let dst_stride = dst.width as usize * 4;
    for row in 0..rows {
        let s_start = row * src_stride;
        let d_start = (y as usize + row) * dst_stride + x as usize * 4;
        let (Some(s_row), Some(d_row)) = (
            src.rgba.get(s_start..s_start + cols * 4),
            dst.rgba.get_mut(d_start..d_start + cols * 4),
        ) else {
            return Err(RenderError::new(format!(
                "composite row {row} out of bounds placing {}x{} at ({x}, {y})",
                src.width, src.height
            )));
        };
        for (d, s) in d_row
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(s_row.as_chunks::<4>().0.iter())
        {
            over_pixel(d, s);
        }
    }
    Ok(())
}

/// Integer straight-alpha source-over of one four-byte RGBA pixel.
fn over_pixel(d: &mut [u8], s: &[u8]) {
    let (Some(&sa), Some(&da)) = (s.get(3), d.get(3)) else {
        return;
    };
    if sa == 0 {
        return;
    }
    if sa == 255 {
        d.copy_from_slice(s);
        return;
    }
    let sa = u32::from(sa);
    let da = u32::from(da);
    let inv = 255 - sa;
    // Backdrop weight in alpha units, rounded: da × (255 − sa) / 255.
    let dw = (da * inv + 127) / 255;
    let a_out = sa + dw;
    if a_out == 0 {
        return;
    }
    for (dc, &sc) in d.iter_mut().zip(s.iter()).take(3) {
        let num = u32::from(sc) * sa + u32::from(*dc) * dw;
        *dc = ((num + a_out / 2) / a_out).min(255) as u8;
    }
    if let Some(alpha) = d.get_mut(3) {
        *alpha = a_out.min(255) as u8;
    }
}

fn check_len(image: &RasterImage, which: &str) -> Result<(), RenderError> {
    let need = (image.width as usize)
        .checked_mul(image.height as usize)
        .and_then(|n| n.checked_mul(4));
    match need {
        Some(n) if image.rgba.len() >= n => Ok(()),
        _ => Err(RenderError::new(format!(
            "{which} image buffer holds {} bytes, fewer than {}x{} RGBA8 needs",
            image.rgba.len(),
            image.width,
            image.height
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, rgba: [u8; 4]) -> RasterImage {
        RasterImage {
            width: w,
            height: h,
            rgba: rgba.repeat((w * h) as usize),
        }
    }

    #[test]
    fn opaque_source_is_copied_at_offset() {
        let mut dst = solid(4, 3, [10, 10, 10, 255]);
        let src = solid(2, 1, [200, 100, 50, 255]);
        composite_over(&mut dst, &src, 1, 2).unwrap();
        let px = |x: usize, y: usize| &dst.rgba[(y * 4 + x) * 4..(y * 4 + x) * 4 + 4];
        assert_eq!(px(1, 2), &[200, 100, 50, 255]);
        assert_eq!(px(2, 2), &[200, 100, 50, 255]);
        assert_eq!(px(0, 2), &[10, 10, 10, 255]);
        assert_eq!(px(1, 1), &[10, 10, 10, 255]);
    }

    #[test]
    fn transparent_source_leaves_destination() {
        let mut dst = solid(2, 2, [1, 2, 3, 255]);
        composite_over(&mut dst, &solid(2, 2, [9, 9, 9, 0]), 0, 0).unwrap();
        assert_eq!(dst.rgba, solid(2, 2, [1, 2, 3, 255]).rgba);
    }

    #[test]
    fn half_alpha_over_opaque_blends() {
        let mut dst = solid(1, 1, [0, 0, 0, 255]);
        composite_over(&mut dst, &solid(1, 1, [255, 255, 255, 128]), 0, 0).unwrap();
        assert_eq!(dst.rgba[3], 255);
        assert!((127..=129).contains(&dst.rgba[0]), "got {}", dst.rgba[0]);
    }

    #[test]
    fn overhanging_source_is_clipped() {
        let mut dst = solid(2, 2, [0, 0, 0, 255]);
        composite_over(&mut dst, &solid(3, 3, [255, 0, 0, 255]), 1, 1).unwrap();
        assert_eq!(&dst.rgba[12..16], &[255, 0, 0, 255]);
        assert_eq!(&dst.rgba[0..4], &[0, 0, 0, 255]);
        composite_over(&mut dst, &solid(1, 1, [9, 9, 9, 255]), 5, 5).unwrap();
    }

    #[test]
    fn short_buffer_is_an_error() {
        let mut dst = solid(2, 2, [0, 0, 0, 255]);
        let bad = RasterImage {
            width: 2,
            height: 2,
            rgba: vec![0; 3],
        };
        assert!(composite_over(&mut dst, &bad, 0, 0).is_err());
    }
}
