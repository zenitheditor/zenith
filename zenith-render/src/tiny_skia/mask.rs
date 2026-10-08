//! Coverage-mask attenuation for masked layers.
//!
//! A mask is a per-pixel coverage field (a shape — rect / rounded-rect / ellipse
//! — optionally feathered with a Gaussian blur and optionally inverted) that
//! attenuates a captured layer's ink. At `EndMask` the captured ink buffer is
//! multiplied, per pixel, by the mask's coverage: opaque coverage (255) leaves
//! the ink untouched, zero coverage makes it fully transparent.
//!
//! All arithmetic is pure integer / `f64` with fixed evaluation order and the
//! same deterministic rounding used elsewhere in the backend (`(v * c + 127) /
//! 255`), so output is byte-identical across runs on the same machine. The
//! coverage rasterization uses anti-aliased path fill (curved shapes need
//! sub-pixel coverage) which is pure-software and deterministic — matching
//! `build_align_mask` and the ellipse fills.

use tiny_skia::{FillRule, Mask, PathBuilder, Pixmap, Rect, Transform};
use zenith_scene::{MaskShape, MaskSpec};

use super::blur::{BlurScratch, gaussian_blur_premul};
use super::crop::{Region, blur_crop};
use super::paths::build_rounded_rect_path;

/// Attenuate `pm` (premultiplied RGBA8) in place by the coverage field described
/// by `spec`.
///
/// `region` is the ink bounding box of `pm`. Every byte outside it is zero and
/// stays zero, so only `region` is attenuated. Builds the coverage for `region`
/// (one byte per pixel, `0..=255`) and multiplies all four premultiplied
/// channels of each pixel uniformly by `coverage / 255` with deterministic
/// rounding. Multiplying premultiplied RGBA uniformly by a coverage factor is
/// exactly alpha attenuation (the premultiplied invariant `c <= a` is
/// preserved). If the coverage buffer cannot be built (allocation failure), `pm`
/// is left untouched — the ink then composites unmasked (a safe degrade, never a
/// panic).
pub(super) fn attenuate_by_mask(pm: &mut Pixmap, spec: &MaskSpec, region: Region) {
    let (width, height) = (pm.width(), pm.height());
    let Some(coverage) = build_mask_coverage(spec, width, height, region) else {
        return; // alloc failure → degrade: leave ink unmasked
    };

    let stride = width as usize * 4;
    let row_len = region.w as usize * 4;
    let x_off = region.x as usize * 4;
    let data = pm.data_mut();
    for (dy, cov_row) in coverage.chunks_exact(region.w as usize).enumerate() {
        let start = (region.y as usize + dy) * stride + x_off;
        let Some(px_row) = data.get_mut(start..start + row_len) else {
            return;
        };
        // `chunks_exact` guarantees exactly 4 bytes per chunk.
        for (px, &cov) in px_row.as_chunks_mut::<4>().0.iter_mut().zip(cov_row) {
            let cov = u32::from(cov);
            // (channel * cov + 127) / 255 — same rounding as shadow.rs premultiply.
            for ch in px.iter_mut() {
                let v = u32::from(*ch);
                *ch = (((v * cov) + 127) / 255).min(255) as u8;
            }
        }
    }
}

/// Build the coverage of `region` for `spec` on a `width` x `height` page.
///
/// The result holds one byte `0..=255` per pixel of `region`, row-major. The
/// shape is rasterized page-sized, so its anti-aliasing matches the full page.
/// A feather blurs only `region` grown by the blur reach (see `crop`). Pixels at
/// least that far inside an interior crop edge read the same window as a
/// full-page blur, so the coverage in `region` is byte-identical.
///
/// Returns `None` on any allocation failure or degenerate geometry, in which
/// case the caller leaves the ink unmasked.
fn build_mask_coverage(
    spec: &MaskSpec,
    width: u32,
    height: u32,
    region: Region,
) -> Option<Vec<u8>> {
    let mut mask = Mask::new(width, height)?;

    // Build the shape path in device space at (x, y, w, h).
    let (x, y, w, h) = (spec.x as f32, spec.y as f32, spec.w as f32, spec.h as f32);
    let path = match spec.shape {
        MaskShape::Rect => {
            let rect = Rect::from_xywh(x, y, w, h)?;
            PathBuilder::from_rect(rect)
        }
        MaskShape::Ellipse => {
            let rect = Rect::from_xywh(x, y, w, h)?;
            PathBuilder::from_oval(rect)?
        }
        MaskShape::RoundedRect => {
            // Clamp radius to a non-negative value no larger than half the box.
            let r = (spec.radius as f32).max(0.0).min(w / 2.0).min(h / 2.0);
            build_rounded_rect_path(x, y, w, h, [r; 4])?
        }
    };

    // AA on: curved shapes need sub-pixel coverage; deterministic same-machine
    // (matches build_align_mask). Identity transform — spec coords are already
    // device/page-absolute pixels.
    mask.fill_path(&path, FillRule::Winding, true, Transform::identity());

    // Coverage = the mask's single alpha channel, optionally feathered.
    let mut coverage: Vec<u8> = if spec.feather > 0.0 {
        // Feather by blurring the coverage. `gaussian_blur_premul` only operates
        // on a Pixmap, so splat the single-channel mask alpha of the crop into
        // all four channels (AAAA), blur, then read the alpha byte back.
        let crop = blur_crop(region, spec.feather, width, height)?;
        let mut temp = Pixmap::new(crop.w, crop.h)?;
        {
            let src = mask.data();
            let dst = temp.data_mut();
            for (dy, out_row) in dst.chunks_exact_mut(crop.w as usize * 4).enumerate() {
                let start = (crop.y as usize + dy) * width as usize + crop.x as usize;
                let src_row = src.get(start..start + crop.w as usize)?;
                for (out, &a) in out_row.as_chunks_mut::<4>().0.iter_mut().zip(src_row) {
                    out[0] = a;
                    out[1] = a;
                    out[2] = a;
                    out[3] = a;
                }
            }
        }
        gaussian_blur_premul(&mut temp, spec.feather, &mut BlurScratch::default());
        let inner = Region {
            x: region.x.checked_sub(crop.x)?,
            y: region.y.checked_sub(crop.y)?,
            w: region.w,
            h: region.h,
        };
        read_region(temp.data(), crop.w, 4, 3, inner)?
    } else {
        read_region(mask.data(), width, 1, 0, region)?
    };

    // Invert: coverage byte c -> 255 - c.
    if spec.invert {
        for c in coverage.iter_mut() {
            *c = 255 - *c;
        }
    }

    Some(coverage)
}

/// Read one byte per pixel of `region` from a row-major buffer `stride_px`
/// pixels wide, with `bpp` bytes per pixel, taking byte `channel` of each.
///
/// Returns `None` when `region` leaves the buffer.
fn read_region(
    data: &[u8],
    stride_px: u32,
    bpp: usize,
    channel: usize,
    region: Region,
) -> Option<Vec<u8>> {
    let stride = stride_px as usize * bpp;
    let row_len = region.w as usize * bpp;
    let mut out = Vec::with_capacity(region.w as usize * region.h as usize);
    for dy in 0..region.h as usize {
        let start = (region.y as usize + dy) * stride + region.x as usize * bpp;
        let row = data.get(start..start + row_len)?;
        out.extend(
            row.chunks_exact(bpp)
                .map(|px| px.get(channel).copied().unwrap_or(0)),
        );
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_scene::{MaskShape, MaskSpec};

    fn full(w: u32, h: u32) -> Region {
        Region::full(w, h).expect("region")
    }

    /// Build a fully-opaque red premultiplied pixmap.
    fn red_pixmap(w: u32, h: u32) -> Pixmap {
        let mut pm = Pixmap::new(w, h).expect("alloc");
        for px in pm.data_mut().as_chunks_mut::<4>().0 {
            px[0] = 200; // r (premultiplied, a=255 → unchanged)
            px[1] = 0;
            px[2] = 0;
            px[3] = 255;
        }
        pm
    }

    fn rect_spec(w: f64, h: f64, invert: bool) -> MaskSpec {
        MaskSpec {
            shape: MaskShape::Rect,
            radius: 0.0,
            feather: 0.0,
            invert,
            x: 0.0,
            y: 0.0,
            w,
            h,
        }
    }

    #[test]
    fn full_rect_no_invert_leaves_pixmap_unchanged() {
        let mut pm = red_pixmap(8, 6);
        let before = pm.data().to_vec();
        attenuate_by_mask(&mut pm, &rect_spec(8.0, 6.0, false), full(8, 6));
        assert_eq!(pm.data(), &before[..], "coverage 255 → no change");
    }

    #[test]
    fn full_rect_inverted_makes_pixmap_transparent() {
        let mut pm = red_pixmap(8, 6);
        attenuate_by_mask(&mut pm, &rect_spec(8.0, 6.0, true), full(8, 6));
        assert!(
            pm.data().iter().all(|&b| b == 0),
            "inverted full coverage → fully transparent",
        );
    }

    #[test]
    fn ellipse_clears_corners_keeps_center() {
        let (w, h) = (16u32, 16u32);
        let mut pm = red_pixmap(w, h);
        let spec = MaskSpec {
            shape: MaskShape::Ellipse,
            radius: 0.0,
            feather: 0.0,
            invert: false,
            x: 0.0,
            y: 0.0,
            w: f64::from(w),
            h: f64::from(h),
        };
        attenuate_by_mask(&mut pm, &spec, full(w, h));
        let data = pm.data();
        // Corner pixel (0,0) is outside the inscribed ellipse → transparent.
        assert_eq!(data.get(3).copied(), Some(0), "corner alpha is 0");
        // Center pixel is inside → fully opaque.
        let cx = (w / 2) as usize;
        let cy = (h / 2) as usize;
        let center_a = (cy * w as usize + cx) * 4 + 3;
        assert_eq!(
            data.get(center_a).copied(),
            Some(255),
            "center alpha is 255"
        );
    }

    #[test]
    fn coverage_is_deterministic() {
        let spec = MaskSpec {
            shape: MaskShape::Ellipse,
            radius: 0.0,
            feather: 2.5,
            invert: true,
            x: 1.0,
            y: 1.0,
            w: 30.0,
            h: 20.0,
        };
        let a = build_mask_coverage(&spec, 32, 24, full(32, 24)).expect("coverage a");
        let b = build_mask_coverage(&spec, 32, 24, full(32, 24)).expect("coverage b");
        assert_eq!(a, b, "two identical calls must be byte-identical");
    }

    /// Cropped feather coverage equals the full-page coverage inside `region`.
    fn assert_region_coverage_matches(spec: &MaskSpec, w: u32, h: u32, region: Region) {
        let page = build_mask_coverage(spec, w, h, full(w, h)).expect("page coverage");
        let cropped = build_mask_coverage(spec, w, h, region).expect("region coverage");
        let mut expected = Vec::new();
        for y in region.y..region.y + region.h {
            let start = (y * w + region.x) as usize;
            expected.extend_from_slice(&page[start..start + region.w as usize]);
        }
        assert_eq!(cropped, expected, "coverage differs for {region:?}");
    }

    #[test]
    fn feathered_region_coverage_matches_full_page() {
        let (w, h) = (48u32, 40u32);
        let spec = MaskSpec {
            shape: MaskShape::Ellipse,
            radius: 0.0,
            feather: 3.5,
            invert: true,
            x: 6.3,
            y: 4.7,
            w: 30.0,
            h: 28.0,
        };
        let regions = [
            // Interior.
            Region {
                x: 18,
                y: 15,
                w: 6,
                h: 5,
            },
            // Touching left, right, top and bottom page edges.
            Region {
                x: 0,
                y: 10,
                w: 4,
                h: 6,
            },
            Region {
                x: 44,
                y: 12,
                w: 4,
                h: 5,
            },
            Region {
                x: 20,
                y: 0,
                w: 7,
                h: 3,
            },
            Region {
                x: 15,
                y: 36,
                w: 9,
                h: 4,
            },
            // Whole page.
            full(w, h),
        ];
        for region in regions {
            assert_region_coverage_matches(&spec, w, h, region);
        }
        let wide = MaskSpec {
            feather: 30.0,
            ..spec
        };
        assert_region_coverage_matches(
            &wide,
            w,
            h,
            Region {
                x: 18,
                y: 15,
                w: 6,
                h: 5,
            },
        );
    }

    #[test]
    fn region_attenuation_matches_full_page() {
        let (w, h) = (32u32, 24u32);
        let spec = MaskSpec {
            shape: MaskShape::RoundedRect,
            radius: 5.0,
            feather: 2.0,
            invert: false,
            x: 3.0,
            y: 2.0,
            w: 20.0,
            h: 16.0,
        };
        // Ink only inside (10..16, 6..12).
        let mut ink = Pixmap::new(w, h).expect("alloc");
        for y in 6..12usize {
            for x in 10..16usize {
                let i = (y * w as usize + x) * 4;
                ink.data_mut()[i..i + 4].copy_from_slice(&[90, 40, 10, 180]);
            }
        }
        let mut full_page = ink.clone();
        attenuate_by_mask(&mut full_page, &spec, full(w, h));
        let mut cropped = ink.clone();
        let bbox = Region {
            x: 10,
            y: 6,
            w: 6,
            h: 6,
        };
        attenuate_by_mask(&mut cropped, &spec, bbox);
        assert_eq!(full_page.data(), cropped.data());
    }
}
