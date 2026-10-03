//! Per-lowering memo of measured sizes.
//!
//! A frame is solved several times while its ancestors measure and place it,
//! and every solve measures its children again. Nested hugging frames would
//! repeat the same measurements at every level, and each text or code
//! measurement shapes or scratch-compiles. The memo keeps each result for the
//! rest of one [`super::lower_nodes`] call.
//!
//! Keys are a node's address in the tree being lowered plus the query input
//! (the width cap, or the width, plus the probe position for a subtree whose
//! height depends on its position). The tree is not reallocated during a
//! lowering: placement writes geometry into nodes after their subtree is
//! measured, and nothing measures a subtree after it is placed. The memo is
//! never iterated, so the address values never reach the output.
//! Temporaries (instance expansions, measure probes) are measured by their
//! own lowering and never keyed here.

use std::cell::RefCell;
use std::collections::BTreeMap;

use zenith_core::Node;

use crate::compile::ProbeAt;

use super::measure::Unsized;

/// The memo key of `node`: its address in the tree being lowered.
pub(super) fn node_key(node: &Node) -> usize {
    std::ptr::from_ref(node).addr()
}

/// The key of an optional width cap: `None` for no cap, else its bits.
pub(super) fn cap_key(cap: Option<f64>) -> Option<u64> {
    cap.map(f64::to_bits)
}

/// The key of a probe position: the bits of its four values.
pub(super) fn probe_key(p: ProbeAt) -> [u64; 4] {
    [p.x.to_bits(), p.y.to_bits(), p.dx.to_bits(), p.dy.to_bits()]
}

/// Content bounds `(min_x, min_y, w, h)` of an expanded instance.
pub(super) type Bounds = Option<(f64, f64, f64, f64)>;

/// A measured size, or [`Unsized`].
type Size = Result<f64, Unsized>;

/// Key of a hug width: node address and width-cap bits.
pub(super) type WidthKey = (usize, Option<u64>);

/// Key of a hug height: node address and width bits.
pub(super) type HeightKey = (usize, u64);

/// Key of a positioned hug height: node address, width bits, probe bits.
pub(super) type PlacedHeightKey = (usize, u64, [u64; 4]);

/// One memo table.
type Table<K, V> = RefCell<BTreeMap<K, V>>;

/// Measured sizes of one lowering, by node address and query input.
#[derive(Default)]
pub(super) struct Memo {
    hug_w: Table<WidthKey, Size>,
    hug_h: Table<HeightKey, Size>,
    hug_h_at: Table<PlacedHeightKey, Size>,
    free: Table<usize, (f64, f64)>,
    bounds: Table<usize, Bounds>,
    depends: Table<usize, bool>,
}

/// Read `key` from `map`. A map that is borrowed elsewhere reads as a miss.
fn get<K: Ord, V: Copy>(map: &Table<K, V>, key: &K) -> Option<V> {
    map.try_borrow().ok()?.get(key).copied()
}

/// Store `value` under `key`. A map that is borrowed elsewhere keeps no entry.
fn put<K: Ord, V>(map: &Table<K, V>, key: K, value: V) {
    if let Ok(mut m) = map.try_borrow_mut() {
        m.insert(key, value);
    }
}

impl Memo {
    /// The cached value of `key` in `map`, else `compute()` stored under it.
    ///
    /// No borrow is held while `compute` runs, so it may recurse into the memo.
    fn cached<K: Ord + Copy, V: Copy>(map: &Table<K, V>, key: K, compute: impl FnOnce() -> V) -> V {
        if let Some(v) = get(map, &key) {
            return v;
        }
        let v = compute();
        put(map, key, v);
        v
    }

    pub(super) fn hug_w(&self, key: WidthKey, compute: impl FnOnce() -> Size) -> Size {
        Self::cached(&self.hug_w, key, compute)
    }

    pub(super) fn hug_h(&self, key: HeightKey, compute: impl FnOnce() -> Size) -> Size {
        Self::cached(&self.hug_h, key, compute)
    }

    pub(super) fn hug_h_at(&self, key: PlacedHeightKey, compute: impl FnOnce() -> Size) -> Size {
        Self::cached(&self.hug_h_at, key, compute)
    }

    pub(super) fn depends(&self, key: usize, compute: impl FnOnce() -> bool) -> bool {
        Self::cached(&self.depends, key, compute)
    }

    pub(super) fn free(&self, key: usize, compute: impl FnOnce() -> (f64, f64)) -> (f64, f64) {
        Self::cached(&self.free, key, compute)
    }

    pub(super) fn bounds(&self, key: usize, compute: impl FnOnce() -> Bounds) -> Bounds {
        Self::cached(&self.bounds, key, compute)
    }
}
