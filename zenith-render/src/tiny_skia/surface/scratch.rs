//! Copies between a scratch buffer and the surface buffer it stands in for.
//!
//! Both buffers are windows of the same full-page device space. Only their
//! overlap moves. A scratch pixel outside the surface starts at zero and is
//! dropped after the draw: tiny-skia blends each pixel on its own, so those
//! pixels never affect a surface pixel.
//!
//! Scratch pixmaps reuse one buffer per thread for the length of a render
//! (see [`ScratchScope`]). A shader draw's scratch starts at the page
//! origin, so it can be nearly page-sized; a fresh zeroed allocation per draw
//! would clear all of it each time (wasm's allocator clears reused memory,
//! and wasm memory never shrinks). The reused buffer is cleared only on the
//! rows the last draw touched.

use std::cell::RefCell;

use tiny_skia::{IntSize, Mask, Pixmap};

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

/// The scratch pixel buffer of this thread and the byte range of it that
/// can hold non-zero bytes. Every byte outside that range is zero.
#[derive(Default)]
struct Reuse {
    buf: Vec<u8>,
    dirty: (usize, usize),
}

thread_local! {
    static PIXELS: RefCell<Reuse> = RefCell::new(Reuse::default());
}

/// Keeps the scratch buffer of this thread for one render and frees it when
/// dropped, so no memory outlives the render.
pub(in crate::tiny_skia) struct ScratchScope(());

impl ScratchScope {
    /// Start a render's scratch scope.
    pub(in crate::tiny_skia) fn enter() -> ScratchScope {
        ScratchScope(())
    }
}

impl Drop for ScratchScope {
    fn drop(&mut self) {
        PIXELS.with(|p| *p.borrow_mut() = Reuse::default());
    }
}

/// An all-zero buffer of `len` bytes: the reused one, cleared where the
/// last draw wrote.
fn zeroed(len: usize) -> Vec<u8> {
    PIXELS.with(|p| {
        let mut reuse = p.borrow_mut();
        let mut buf = std::mem::take(&mut reuse.buf);
        let (s, e) = reuse.dirty;
        let e = e.min(buf.len());
        if let Some(span) = buf.get_mut(s..e) {
            span.fill(0);
        }
        reuse.dirty = (0, 0);
        buf.truncate(len);
        buf.resize(len, 0);
        buf
    })
}

/// Return `buf` to the reuse slot; only `dirty` can hold non-zero bytes.
fn give_back(buf: Vec<u8>, dirty: (usize, usize)) {
    PIXELS.with(|p| {
        let mut reuse = p.borrow_mut();
        if buf.capacity() >= reuse.buf.capacity() {
            *reuse = Reuse { buf, dirty };
        }
    });
}

/// A scratch pixmap for `area` holding the surface pixels it overlaps.
pub(super) fn pixmap_from(target: &Pixmap, surface: Surface, area: DeviceBox) -> Option<Pixmap> {
    let len = (area.w() as usize)
        .checked_mul(area.h() as usize)?
        .checked_mul(4)?;
    let size = IntSize::from_wh(area.w(), area.h())?;
    let mut scratch = Pixmap::from_vec(zeroed(len), size)?;
    copy_overlap(
        target.data(),
        surface.device_box(),
        scratch.data_mut(),
        area,
        4,
    );
    Some(scratch)
}

/// Write the surface part of `scratch` (covering `area`) back to `target`,
/// and keep the scratch buffer for the next draw. The draw's ink lies in
/// `ink`.
pub(super) fn pixmap_back(
    target: &mut Pixmap,
    surface: Surface,
    scratch: Pixmap,
    area: DeviceBox,
    ink: DeviceBox,
) {
    copy_overlap(
        scratch.data(),
        area,
        target.data_mut(),
        surface.device_box(),
        4,
    );
    // Rows that can hold non-zero bytes: the copied overlap and the ink.
    let mut rows = (ink.y0, ink.y1);
    if let Some(o) = area.intersect(surface.device_box()) {
        rows = (rows.0.min(o.y0), rows.1.max(o.y1));
    }
    let stride = area.w() as usize * 4;
    let row = |y: u32| (y.clamp(area.y0, area.y1) - area.y0) as usize * stride;
    give_back(scratch.take(), (row(rows.0), row(rows.1)));
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
    fn a_reused_buffer_starts_all_zero_outside_the_overlap() {
        let surface = Surface {
            x: 4,
            y: 4,
            w: 2,
            h: 2,
            page_w: 50,
            page_h: 50,
            exact: true,
        };
        let target = Pixmap::new(2, 2).expect("alloc");
        let _scope = ScratchScope::enter();
        let big = DeviceBox {
            x0: 0,
            y0: 0,
            x1: 30,
            y1: 20,
        };
        let mut first = pixmap_from(&target, surface, big).expect("scratch");
        let ink = DeviceBox {
            x0: 2,
            y0: 10,
            x1: 25,
            y1: 18,
        };
        for y in ink.y0..ink.y1 {
            let at = (y * 30 + ink.x0) as usize * 4;
            first.data_mut()[at..at + 23 * 4].fill(7);
        }
        let mut back = Pixmap::new(2, 2).expect("alloc");
        pixmap_back(&mut back, surface, first, big, ink);
        // Another layout over the same bytes comes back all zero.
        let other = DeviceBox {
            x0: 0,
            y0: 0,
            x1: 17,
            y1: 40,
        };
        let second = pixmap_from(&target, surface, other).expect("scratch");
        assert!(second.data().iter().all(|&b| b == 0));
    }

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
        let _scope = ScratchScope::enter();
        let scratch = pixmap_from(&target, surface, area).expect("scratch");
        // Surface pixel (2, 0) = device (12, 5) = scratch (0, 5).
        let s = (5 * 8) * 4;
        assert_eq!(&scratch.data()[s..s + 4], &target.data()[8..12]);
        let mut back = Pixmap::new(4, 3).expect("alloc");
        let kept = scratch.clone();
        pixmap_back(&mut back, surface, kept, area, area);
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
