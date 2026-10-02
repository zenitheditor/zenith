//! Region helpers that limit capture effects to pixels that can be non-zero.
//!
//! A capture pixmap is page-sized, but its ink often covers a small area. Each
//! effect (shadow, blur, mask feather, filter, layer composite) works on the
//! ink's bounding box. A blur grows that box by its reach plus one pixel.
//!
//! Output stays byte-identical to the full-page path:
//! - A box pass reads a clamped window, so a cropped pass differs from a
//!   full-page pass only where the window crosses an interior crop edge.
//! - With a margin of reach + 1, every pixel on or beyond an interior crop edge
//!   is zero at the input of every pass. The clamped read then sums the same
//!   zeros the full-page read sums.
//! - A crop edge clamped to the page edge clamps exactly as the full page does.
//! - A transparent source pixel leaves the destination unchanged under
//!   source-over in tiny-skia (highp `d * 1 + 0`, lowp `(d * 255 + 255) >> 8`).
//!   Skipping such pixels draws the same bytes.

use tiny_skia::{Pixmap, PixmapPaint, Transform};

use super::blur::blur_reach;

/// Page-space pixel rectangle. `w` and `h` are at least 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Region {
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) w: u32,
    pub(super) h: u32,
}

impl Region {
    /// The whole surface of a `width` x `height` pixmap. `None` for a zero size.
    #[cfg(test)]
    pub(super) fn full(width: u32, height: u32) -> Option<Region> {
        if width == 0 || height == 0 {
            return None;
        }
        Some(Region {
            x: 0,
            y: 0,
            w: width,
            h: height,
        })
    }

    /// Exclusive right edge.
    fn right(self) -> u32 {
        self.x.saturating_add(self.w)
    }

    /// Exclusive bottom edge.
    fn bottom(self) -> u32 {
        self.y.saturating_add(self.h)
    }
}

/// Bounding box of every pixel with any non-zero byte.
///
/// Returns `None` when every byte is zero. A pixel with zero alpha but a
/// non-zero color byte counts as ink, so the box covers every byte a blur reads.
pub(super) fn ink_bbox(pm: &Pixmap) -> Option<Region> {
    let stride = pm.width() as usize * 4;
    if stride == 0 {
        return None;
    }
    // (left, top, right, bottom), all inclusive pixel indices.
    let mut bounds: Option<(usize, usize, usize, usize)> = None;
    for (y, row) in pm.data().chunks_exact(stride).enumerate() {
        let Some(first) = row.iter().position(|&b| b != 0) else {
            continue;
        };
        let last = row.iter().rposition(|&b| b != 0).unwrap_or(first);
        let (left, right) = (first / 4, last / 4);
        bounds = Some(match bounds {
            None => (left, y, right, y),
            Some((l, t, r, _)) => (l.min(left), t, r.max(right), y),
        });
    }
    let (left, top, right, bottom) = bounds?;
    Some(Region {
        x: u32::try_from(left).ok()?,
        y: u32::try_from(top).ok()?,
        w: u32::try_from(right - left + 1).ok()?,
        h: u32::try_from(bottom - top + 1).ok()?,
    })
}

/// Grow `bbox` by the reach of a blur with `sigma` plus one pixel.
///
/// The result is clamped to the `width` x `height` page. Returns `None` when the
/// clamped rectangle is empty.
pub(super) fn blur_crop(bbox: Region, sigma: f64, width: u32, height: u32) -> Option<Region> {
    let margin = blur_reach(sigma).saturating_add(1);
    let x0 = bbox.x.saturating_sub(margin);
    let y0 = bbox.y.saturating_sub(margin);
    let x1 = bbox.right().saturating_add(margin).min(width);
    let y1 = bbox.bottom().saturating_add(margin).min(height);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some(Region {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    })
}

/// Copy `region` of `src` into a new pixmap of the region's size.
///
/// Returns `None` on allocation failure or when `region` leaves `src`.
pub(super) fn copy_region(src: &Pixmap, region: Region) -> Option<Pixmap> {
    let mut out = Pixmap::new(region.w, region.h)?;
    let src_stride = src.width() as usize * 4;
    let row_len = region.w as usize * 4;
    let x_off = region.x as usize * 4;
    let src_data = src.data();
    for (dy, dst_row) in out.data_mut().chunks_exact_mut(row_len).enumerate() {
        let start = (region.y as usize + dy) * src_stride + x_off;
        let src_row = src_data.get(start..start + row_len)?;
        dst_row.copy_from_slice(src_row);
    }
    Some(out)
}

/// Draw `src` onto `target` with its top-left pixel at (`x`, `y`).
///
/// Each source pixel lands exactly at its offset, clipped to `target`. The part
/// left of or above the target is trimmed first, so tiny-skia always gets a
/// non-negative position. tiny-skia mis-places a negative position:
/// `Rect::round` computes `(floor(x) + 0.5) as i32`, which truncates toward
/// zero, so the rect starts at `x + 1` and spans one column past the source.
/// `SpreadMode::Pad` then smears the source's last column into it. Rows behave
/// the same. A source fully off `target` draws nothing.
pub(super) fn draw_at(target: &mut Pixmap, src: &Pixmap, x: i64, y: i64, paint: &PixmapPaint) {
    let (tw, th) = (i64::from(target.width()), i64::from(target.height()));
    let (sw, sh) = (i64::from(src.width()), i64::from(src.height()));
    // Visible source span, in source pixels.
    let (sx0, sy0) = ((-x).max(0), (-y).max(0));
    let (sx1, sy1) = (sw.min(tw - x), sh.min(th - y));
    if sx1 <= sx0 || sy1 <= sy0 {
        return;
    }
    let (Ok(xi), Ok(yi)) = (i32::try_from(x + sx0), i32::try_from(y + sy0)) else {
        return;
    };
    if sx0 == 0 && sy0 == 0 {
        target.draw_pixmap(xi, yi, src.as_ref(), paint, Transform::identity(), None);
        return;
    }
    let visible = Region {
        x: u32::try_from(sx0).unwrap_or(0),
        y: u32::try_from(sy0).unwrap_or(0),
        w: u32::try_from(sx1 - sx0).unwrap_or(0),
        h: u32::try_from(sy1 - sy0).unwrap_or(0),
    };
    if let Some(part) = copy_region(src, visible) {
        target.draw_pixmap(xi, yi, part.as_ref(), paint, Transform::identity(), None);
    }
}

/// Draw only `region` of page-sized `src` onto page-sized `target`.
///
/// Every byte of `src` outside `region` must be zero. On allocation failure the
/// whole `src` is drawn, which gives the same bytes.
pub(super) fn draw_region(target: &mut Pixmap, src: &Pixmap, region: Region, paint: &PixmapPaint) {
    match copy_region(src, region) {
        Some(part) => draw_at(
            target,
            &part,
            i64::from(region.x),
            i64::from(region.y),
            paint,
        ),
        None => target.draw_pixmap(0, 0, src.as_ref(), paint, Transform::identity(), None),
    }
}

/// Draw the ink bounding box of page-sized `src` onto `target`.
///
/// Empty ink draws nothing.
pub(super) fn draw_ink_region(target: &mut Pixmap, src: &Pixmap, paint: &PixmapPaint) {
    if let Some(bbox) = ink_bbox(src) {
        draw_region(target, src, bbox, paint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::{BlendMode, FillRule, FilterQuality, Paint, PathBuilder};

    /// A page with an anti-aliased circle, so edge pixels carry partial alpha.
    fn circle_page(w: u32, h: u32, cx: f32, cy: f32, r: f32) -> Pixmap {
        let mut pm = Pixmap::new(w, h).expect("alloc");
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        paint.set_color_rgba8(30, 140, 220, 200);
        let path = PathBuilder::from_circle(cx, cy, r).expect("circle");
        pm.fill_path(
            &path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
        pm
    }

    /// A page filled with a semi-opaque backdrop, so no-op draws are visible.
    fn backdrop(w: u32, h: u32) -> Pixmap {
        let mut pm = Pixmap::new(w, h).expect("alloc");
        for (i, px) in pm.data_mut().chunks_exact_mut(4).enumerate() {
            let v = (i % 97) as u8;
            px[0] = v;
            px[1] = v / 2;
            px[2] = v / 3;
            px[3] = 120;
        }
        pm
    }

    #[test]
    fn ink_bbox_of_empty_page_is_none() {
        let pm = Pixmap::new(10, 8).expect("alloc");
        assert_eq!(ink_bbox(&pm), None);
    }

    #[test]
    fn ink_bbox_covers_every_non_zero_byte() {
        let mut pm = Pixmap::new(10, 8).expect("alloc");
        // Pixel (2, 1): only a color byte set.
        pm.data_mut()[(10 + 2) * 4] = 5;
        // Pixel (7, 5): only alpha set.
        pm.data_mut()[(5 * 10 + 7) * 4 + 3] = 9;
        assert_eq!(
            ink_bbox(&pm),
            Some(Region {
                x: 2,
                y: 1,
                w: 6,
                h: 5
            })
        );
    }

    #[test]
    fn blur_crop_clamps_to_page() {
        let bbox = Region {
            x: 1,
            y: 2,
            w: 3,
            h: 3,
        };
        let crop = blur_crop(bbox, 2.0, 12, 10).expect("crop");
        let reach = blur_reach(2.0) + 1;
        assert_eq!(crop.x, 0);
        assert_eq!(crop.y, 0);
        assert_eq!(crop.right(), (bbox.right() + reach).min(12));
        assert_eq!(crop.bottom(), (bbox.bottom() + reach).min(10));
    }

    #[test]
    fn draw_ink_region_matches_full_page_draw_with_opacity() {
        let (w, h) = (40, 30);
        let src = circle_page(w, h, 20.0, 12.0, 6.5);
        let paint = PixmapPaint {
            opacity: 0.6,
            blend_mode: BlendMode::SourceOver,
            quality: FilterQuality::Nearest,
        };
        let mut full = backdrop(w, h);
        full.draw_pixmap(0, 0, src.as_ref(), &paint, Transform::identity(), None);
        let mut cropped = backdrop(w, h);
        draw_ink_region(&mut cropped, &src, &paint);
        assert_eq!(full.data(), cropped.data());
    }

    #[test]
    fn draw_ink_region_matches_full_page_draw_at_page_edges() {
        let (w, h) = (24, 20);
        let src = circle_page(w, h, 0.0, 19.0, 7.0);
        let paint = PixmapPaint::default();
        let mut full = backdrop(w, h);
        full.draw_pixmap(0, 0, src.as_ref(), &paint, Transform::identity(), None);
        let mut cropped = backdrop(w, h);
        draw_ink_region(&mut cropped, &src, &paint);
        assert_eq!(full.data(), cropped.data());
    }

    #[test]
    fn draw_at_places_negative_offset_exactly() {
        // Reference: the same source copied by hand onto a zero canvas.
        let src = circle_page(12, 10, 6.0, 5.0, 5.0);
        for (x, y) in [(-3i64, 2i64), (4, -5), (-11, -9), (20, 3)] {
            let mut target = Pixmap::new(16, 14).expect("alloc");
            draw_at(&mut target, &src, x, y, &PixmapPaint::default());
            let mut expected = vec![0u8; 16 * 14 * 4];
            for sy in 0..10i64 {
                for sx in 0..12i64 {
                    let (tx, ty) = (sx + x, sy + y);
                    if (0..16).contains(&tx) && (0..14).contains(&ty) {
                        let s = ((sy * 12 + sx) * 4) as usize;
                        let t = ((ty * 16 + tx) * 4) as usize;
                        expected[t..t + 4].copy_from_slice(&src.data()[s..s + 4]);
                    }
                }
            }
            assert_eq!(target.data(), &expected[..], "offset ({x}, {y})");
        }
    }

    #[test]
    fn draw_at_skips_fully_off_page_source() {
        let src = circle_page(8, 8, 4.0, 4.0, 3.0);
        let mut target = backdrop(16, 16);
        let before = target.data().to_vec();
        draw_at(&mut target, &src, 16, 0, &PixmapPaint::default());
        draw_at(&mut target, &src, -8, 0, &PixmapPaint::default());
        draw_at(
            &mut target,
            &src,
            i64::from(i32::MAX),
            0,
            &PixmapPaint::default(),
        );
        assert_eq!(target.data(), &before[..]);
    }
}
