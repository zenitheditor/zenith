//! Copies between a scratch buffer and the surface buffer it stands in for.
//!
//! Both buffers are windows of the same full-page device space. Only their
//! overlap moves. A scratch pixel outside the surface starts at zero and is
//! dropped after the draw: tiny-skia blends each pixel on its own, so those
//! pixels never affect a surface pixel.

use tiny_skia::{Mask, Pixmap};

use super::window::{DeviceBox, Surface};

/// Row ranges of the overlap of `a` and `b`, as byte offsets into each buffer
/// (`bpp` bytes per pixel). Each entry is `(a_start, b_start, len)`.
fn overlap_rows(a: DeviceBox, b: DeviceBox, bpp: usize) -> Vec<(usize, usize, usize)> {
    let Some(o) = a.intersect(b) else {
        return Vec::new();
    };
    let len = o.w() as usize * bpp;
    (o.y0..o.y1)
        .map(|y| {
            let at = |buf: DeviceBox| {
                ((y - buf.y0) as usize * buf.w() as usize + (o.x0 - buf.x0) as usize) * bpp
            };
            (at(a), at(b), len)
        })
        .collect()
}

/// Copy the overlap of `src` (covering `src_box`) into `dst` (covering
/// `dst_box`). Rows that leave either buffer are skipped.
fn copy_overlap(src: &[u8], src_box: DeviceBox, dst: &mut [u8], dst_box: DeviceBox, bpp: usize) {
    for (s, d, len) in overlap_rows(src_box, dst_box, bpp) {
        if let (Some(from), Some(to)) = (src.get(s..s + len), dst.get_mut(d..d + len)) {
            to.copy_from_slice(from);
        }
    }
}

/// A scratch pixmap for `area` holding the surface pixels it overlaps.
pub(super) fn pixmap_from(target: &Pixmap, surface: Surface, area: DeviceBox) -> Option<Pixmap> {
    let mut scratch = Pixmap::new(area.w(), area.h())?;
    copy_overlap(
        target.data(),
        surface.device_box(),
        scratch.data_mut(),
        area,
        4,
    );
    Some(scratch)
}

/// Write the surface part of `scratch` (covering `area`) back to `target`.
pub(super) fn pixmap_back(
    target: &mut Pixmap,
    surface: Surface,
    scratch: &Pixmap,
    area: DeviceBox,
) {
    copy_overlap(
        scratch.data(),
        area,
        target.data_mut(),
        surface.device_box(),
        4,
    );
}

/// A scratch mask for `area` holding the coverage of the surface mask
/// `mask` it overlaps. Zero elsewhere.
pub(super) fn mask_from(mask: &Mask, surface: Surface, area: DeviceBox) -> Option<Mask> {
    let mut scratch = Mask::new(area.w(), area.h())?;
    copy_overlap(
        mask.data(),
        surface.device_box(),
        scratch.data_mut(),
        area,
        1,
    );
    Some(scratch)
}

/// Write the surface part of the scratch mask `scratch` (covering `area`)
/// into the surface mask `mask`.
pub(super) fn mask_back(mask: &mut Mask, surface: Surface, scratch: &Mask, area: DeviceBox) {
    copy_overlap(
        scratch.data(),
        area,
        mask.data_mut(),
        surface.device_box(),
        1,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_round_trip_through_the_overlap() {
        let surface = Surface {
            x: 10,
            y: 5,
            w: 4,
            h: 3,
            page_w: 50,
            page_h: 50,
            exact: true,
        };
        let mut target = Pixmap::new(4, 3).expect("alloc");
        for (i, b) in target.data_mut().iter_mut().enumerate() {
            *b = i as u8;
        }
        let area = DeviceBox {
            x0: 12,
            y0: 0,
            x1: 20,
            y1: 7,
        };
        let scratch = pixmap_from(&target, surface, area).expect("scratch");
        // Surface pixel (2, 0) = device (12, 5) = scratch (0, 5).
        let s = (5 * 8) * 4;
        assert_eq!(&scratch.data()[s..s + 4], &target.data()[8..12]);
        let mut back = Pixmap::new(4, 3).expect("alloc");
        pixmap_back(&mut back, surface, &scratch, area);
        // Only the overlap (device x 12..14, y 5..7) comes back.
        for row in 0..3usize {
            let r = row * 16;
            assert_eq!(&back.data()[r..r + 8], &[0u8; 8]);
            let want = if row < 2 {
                &target.data()[r + 8..r + 16]
            } else {
                &[0u8; 8][..]
            };
            assert_eq!(&back.data()[r + 8..r + 16], want);
        }
    }
}
