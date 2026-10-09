//! [`CapturePool`]: surface-sized effect captures, reused within one render.
//!
//! Every shadow, blur, filter, and mask opens a surface-sized capture. A new
//! `Pixmap` per capture costs a large zeroed allocation (page faults for
//! fresh memory) for a few small nodes. The pool hands out all-zero buffers
//! and takes them back after zeroing only the bytes the capture could have
//! set, so a capture starts exactly as a new `Pixmap` would.

use tiny_skia::Pixmap;

use super::crop::Region;

/// The most idle buffers the pool keeps. Captures nest only a few deep.
const KEEP: usize = 4;

/// Where a returned capture can hold non-zero bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Dirty {
    /// Every byte is zero.
    Clean,
    /// Every non-zero byte lies in this region.
    Region(Region),
    /// Any byte can be non-zero.
    All,
}

/// All-zero `width × height` pixmaps for effect captures.
pub(super) struct CapturePool {
    width: u32,
    height: u32,
    free: Vec<Pixmap>,
}

impl CapturePool {
    /// An empty pool of `width × height` buffers.
    pub(super) fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            free: Vec::new(),
        }
    }

    /// An all-zero buffer: an idle one, else a new one. `None` when the
    /// allocation fails.
    pub(super) fn take(&mut self) -> Option<Pixmap> {
        self.free
            .pop()
            .or_else(|| Pixmap::new(self.width, self.height))
    }

    /// Take back `pm`. `dirty` names where it can hold non-zero bytes; the
    /// pool zeroes that part. A buffer of another size is dropped.
    pub(super) fn give(&mut self, mut pm: Pixmap, dirty: Dirty) {
        if self.free.len() >= KEEP || pm.width() != self.width || pm.height() != self.height {
            return;
        }
        match dirty {
            Dirty::Clean => {}
            Dirty::All => pm.data_mut().fill(0),
            Dirty::Region(r) => zero_region(&mut pm, r),
        }
        self.free.push(pm);
    }
}

/// Zero the bytes of `region` (clipped to `pm`).
fn zero_region(pm: &mut Pixmap, region: Region) {
    let width = pm.width() as usize;
    let stride = width * 4;
    let x0 = (region.x as usize).min(width);
    let x1 = (region.x as usize)
        .saturating_add(region.w as usize)
        .min(width);
    let rows = pm
        .data_mut()
        .chunks_exact_mut(stride)
        .skip(region.y as usize)
        .take(region.h as usize);
    for row in rows {
        if let Some(span) = row.get_mut(x0 * 4..x1 * 4) {
            span.fill(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_returned_buffer_comes_back_all_zero() {
        let mut pool = CapturePool::new(8, 6);
        let mut pm = pool.take().expect("alloc");
        // Ink in rows 1..=3, columns 2..=5.
        for y in 1..=3usize {
            for x in 2..=5usize {
                pm.data_mut()[(y * 8 + x) * 4..(y * 8 + x) * 4 + 4].copy_from_slice(&[9, 8, 7, 6]);
            }
        }
        pool.give(
            pm,
            Dirty::Region(Region {
                x: 2,
                y: 1,
                w: 4,
                h: 3,
            }),
        );
        let again = pool.take().expect("reuse");
        assert!(again.data().iter().all(|&b| b == 0));
        let mut all = again;
        all.data_mut().fill(3);
        pool.give(all, Dirty::All);
        assert!(pool.take().expect("reuse").data().iter().all(|&b| b == 0));
    }

    #[test]
    fn region_past_the_edge_is_clipped_and_other_sizes_are_dropped() {
        let mut pool = CapturePool::new(4, 4);
        let mut pm = pool.take().expect("alloc");
        pm.data_mut().fill(1);
        pool.give(
            pm,
            Dirty::Region(Region {
                x: 0,
                y: 0,
                w: 9,
                h: 9,
            }),
        );
        assert!(pool.take().expect("reuse").data().iter().all(|&b| b == 0));
        pool.give(Pixmap::new(5, 4).expect("alloc"), Dirty::Clean);
        assert!(pool.free.is_empty());
    }
}
