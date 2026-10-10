//! `align.near_miss`: a node edge sits just off an edge its siblings share.
//!
//! Among the checked siblings of one parent (see [`super::arrange`]), each
//! axis value is compared: `left`, `right`, `hcenter`, and `top` of the box.
//! A text uses its glyph ink for `left` / `right` / `hcenter` and its first
//! baseline in place of `top`, so texts compare baselines with texts only.
//! A node's value is a near miss when it sits 0.75–3px from a value that at
//! least 2 OTHER siblings share within ±0.25px, more siblings share that
//! value than share the node's own, and one of them lies near the node on
//! the cross axis (gap at most 2 × the larger cross extent + 16px). A node
//! whose authored box value (box edge or center, box top for a baseline)
//! equals that of at least 2 of those siblings is aligned as authored and
//! not reported: the ink offset is glyph side bearing or type size. Each
//! node reports at most one horizontal and one vertical near miss, in axis
//! order `left`, `right`, `hcenter`, then `top`, `baseline`.

use std::collections::{BTreeMap, BTreeSet};

use zenith_core::Diagnostic;

use super::arrange::{Sibling, direction, id_list, moved, num, set_px};
use super::paint::Authored;

/// Smallest offset reported, in px.
const MIN_OFF: f64 = 0.75;
/// Largest offset reported, in px.
const MAX_OFF: f64 = 3.0;
/// Values within this distance share an edge, in px.
const SHARED: f64 = 0.25;
/// Siblings other than the node that must share the value.
const MIN_SHARED: usize = 2;
/// Fixed part of the cross-axis proximity limit, in px.
const NEAR_PAD: f64 = 16.0;

/// One compared axis value.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    Left,
    Right,
    HCenter,
    Top,
    Baseline,
}

impl Axis {
    const ALL: [Axis; 5] = [
        Axis::Left,
        Axis::Right,
        Axis::HCenter,
        Axis::Top,
        Axis::Baseline,
    ];

    fn name(self) -> &'static str {
        match self {
            Axis::Left => "left",
            Axis::Right => "right",
            Axis::HCenter => "hcenter",
            Axis::Top => "top",
            Axis::Baseline => "baseline",
        }
    }

    /// `true` for a value along x (the cross axis is y).
    fn horizontal(self) -> bool {
        match self {
            Axis::Left | Axis::Right | Axis::HCenter => true,
            Axis::Top | Axis::Baseline => false,
        }
    }

    /// The node's value on this axis, if it has one.
    fn value(self, s: &Sibling<'_>) -> Option<f64> {
        let b = s.text.map_or(s.rect, |t| t.bounds);
        match self {
            Axis::Left => Some(b.x),
            Axis::Right => Some(b.x + b.w),
            Axis::HCenter => Some(b.x + b.w / 2.0),
            Axis::Top => s.text.is_none().then_some(s.rect.y),
            Axis::Baseline => s.text.and_then(|t| t.ink.baseline),
        }
    }

    /// The node's authored-box value on this axis: the box edge or center,
    /// and the box top of a text on the baseline axis.
    fn box_value(self, s: &Sibling<'_>) -> Option<f64> {
        let b = s.rect;
        match self {
            Axis::Left => Some(b.x),
            Axis::Right => Some(b.x + b.w),
            Axis::HCenter => Some(b.x + b.w / 2.0),
            Axis::Top => s.text.is_none().then_some(b.y),
            Axis::Baseline => s.text.map(|_| b.y),
        }
    }
}

/// The cross-axis interval of a node: its ink for a text, else its box.
fn cross(s: &Sibling<'_>, horizontal: bool) -> (f64, f64) {
    let b = s.text.map_or(s.rect, |t| t.bounds);
    if horizontal {
        (b.y, b.y + b.h)
    } else {
        (b.x, b.x + b.w)
    }
}

/// `true` when `a` and `b` lie near each other on the cross axis.
fn near(a: &Sibling<'_>, b: &Sibling<'_>, horizontal: bool) -> bool {
    let (a0, a1) = cross(a, horizontal);
    let (b0, b1) = cross(b, horizontal);
    let gap = (b0 - a1).max(a0 - b1).max(0.0);
    gap <= 2.0 * (a1 - a0).max(b1 - b0) + NEAR_PAD
}

/// A near miss found for one node.
struct Miss {
    axis: Axis,
    value: f64,
    shared: f64,
    /// Indexes into the sibling set of the siblings sharing `shared`.
    members: Vec<usize>,
}

/// One axis of a sibling set: the finite values with their sibling
/// indexes, sorted by value, then index.
///
/// A non-finite value never shares an edge or sits near one (every
/// comparison with it is false), so it is left out.
struct Column {
    sorted: Vec<(f64, usize)>,
}

impl Column {
    fn new(values: impl Iterator<Item = (usize, Option<f64>)>) -> Column {
        let mut sorted: Vec<(f64, usize)> = values
            .filter_map(|(i, v)| v.filter(|v| v.is_finite()).map(|v| (v, i)))
            .collect();
        sorted.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        Column { sorted }
    }

    /// The entries `w` with `w - c` in `[lo, hi]`, as a sorted run.
    /// `w - c` never decreases as `w` grows, so they are contiguous.
    fn run(&self, c: f64, lo: f64, hi: f64) -> &[(f64, usize)] {
        let start = self.sorted.partition_point(|(w, _)| *w - c < lo);
        let end = self.sorted.partition_point(|(w, _)| *w - c <= hi);
        self.sorted.get(start..end.max(start)).unwrap_or(&[])
    }

    /// The entries sharing `c`: `|w - c| <= SHARED`.
    fn sharing(&self, c: f64) -> &[(f64, usize)] {
        self.run(c, -SHARED, SHARED)
    }
}

/// How much work a near-miss search did: sorted-run lookups and
/// candidate values evaluated. Grows near `n log n` in the sibling count.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct Work {
    pub(super) lookups: u64,
    pub(super) candidates: u64,
}

/// Every `align.near_miss` of one sibling set.
pub(super) fn near_miss(
    set: &[Sibling<'_>],
    authored: &BTreeMap<String, Authored>,
) -> Vec<Diagnostic> {
    near_miss_counted(set, authored, &mut Work::default())
}

/// [`near_miss`], adding the work done to `work`.
pub(super) fn near_miss_counted(
    set: &[Sibling<'_>],
    authored: &BTreeMap<String, Authored>,
    work: &mut Work,
) -> Vec<Diagnostic> {
    if set.len() <= MIN_SHARED {
        return Vec::new();
    }
    let values: Vec<[Option<f64>; 5]> = set.iter().map(|s| Axis::ALL.map(|a| a.value(s))).collect();
    let columns: Vec<Column> = (0..Axis::ALL.len())
        .map(|k| {
            Column::new(
                values
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (i, v.get(k).copied().flatten())),
            )
        })
        .collect();
    let mut out = Vec::new();
    for (n, s) in set.iter().enumerate() {
        let mut found: [Option<Miss>; 2] = [None, None];
        for (k, axis) in Axis::ALL.into_iter().enumerate() {
            let slot = usize::from(!axis.horizontal());
            if found.get(slot).is_some_and(Option::is_some) {
                continue;
            }
            let (Some(v), Some(column)) = (
                values.get(n).and_then(|row| row.get(k).copied().flatten()),
                columns.get(k),
            ) else {
                continue;
            };
            if let (Some(miss), Some(cell)) = (
                find_miss(set, column, n, v, axis, work),
                found.get_mut(slot),
            ) {
                *cell = Some(miss);
            }
        }
        for miss in found.into_iter().flatten() {
            out.push(diagnostic(set, s, miss, authored));
        }
    }
    out
}

/// The near miss of node `n` (value `v`) on `axis`, if any.
///
/// The candidate values are those 0.75–3px from `v`, read from the sorted
/// `column`. Equal candidate values give equal outcomes, so each is
/// evaluated once, at its lowest sibling index (the first in sibling
/// order, as a ties-keep-first scan over siblings would pick).
fn find_miss(
    set: &[Sibling<'_>],
    column: &Column,
    n: usize,
    v: f64,
    axis: Axis,
    work: &mut Work,
) -> Option<Miss> {
    if !v.is_finite() {
        return None;
    }
    let me = set.get(n)?;
    work.lookups += 1;
    // `n` itself shares its own value.
    let own = column.sharing(v).len().saturating_sub(1);
    work.lookups += 2;
    let mut candidates: Vec<(usize, f64)> = column
        .run(v, -MAX_OFF, -MIN_OFF)
        .iter()
        .chain(column.run(v, MIN_OFF, MAX_OFF))
        .map(|&(c, m)| (m, c))
        .collect();
    candidates.sort_by_key(|&(m, _)| m);
    let mut seen: BTreeSet<u64> = BTreeSet::new();
    let mut best: Option<Miss> = None;
    for (_, c) in candidates {
        if !seen.insert(c.to_bits()) {
            continue;
        }
        work.candidates += 1;
        work.lookups += 1;
        // `n` is 0.75px or more from `c`, so it is never in this run.
        let run = column.sharing(c);
        if run.len() < MIN_SHARED || run.len() <= own {
            continue;
        }
        let mut members: Vec<usize> = run.iter().map(|&(_, k)| k).collect();
        members.sort_unstable();
        let close = members
            .iter()
            .filter_map(|&k| set.get(k))
            .any(|o| near(me, o, axis.horizontal()));
        if !close {
            continue;
        }
        // The shared value: the median of the members' values (the run is
        // sorted by value).
        let Some(&(c, _)) = run.get(run.len() / 2) else {
            continue;
        };
        let off = (c - v).abs();
        if !(MIN_OFF..=MAX_OFF).contains(&off) {
            continue;
        }
        // Boxes that share the edge are aligned as authored: the offset is
        // glyph side bearing or a different type size.
        if let Some(own) = axis.box_value(me) {
            let agree = members
                .iter()
                .filter_map(|&k| set.get(k))
                .filter(|o| axis.box_value(o).is_some_and(|b| (b - own).abs() <= SHARED))
                .count();
            if agree >= MIN_SHARED {
                continue;
            }
        }
        // More members first, then the smaller offset, then the smaller value.
        let better = best.as_ref().is_none_or(|b| {
            members
                .len()
                .cmp(&b.members.len())
                .then_with(|| (b.shared - v).abs().total_cmp(&off))
                .then_with(|| b.shared.total_cmp(&c))
                .is_gt()
        });
        if better {
            best = Some(Miss {
                axis,
                value: v,
                shared: c,
                members,
            });
        }
    }
    best
}

fn diagnostic(
    set: &[Sibling<'_>],
    s: &Sibling<'_>,
    miss: Miss,
    authored: &BTreeMap<String, Authored>,
) -> Diagnostic {
    let id = &s.entry.id;
    let axis = miss.axis;
    let delta = miss.shared - miss.value;
    let names = id_list(
        miss.members
            .iter()
            .filter_map(|&k| set.get(k))
            .map(|o| o.entry.id.as_str()),
        3,
    );
    let property = if axis.horizontal() { "x" } else { "y" };
    let target = moved(s, authored, axis.horizontal(), delta);
    let head = format!(
        "'{id}' {}={} is {}px off {} shared by {names}",
        axis.name(),
        num(miss.value),
        num(delta.abs()),
        num(miss.shared),
    );
    let (message, fix) = match target {
        Some(at) => (
            format!("{head} — set {property}=(px){}", num(at)),
            Some(set_px(property, at)),
        ),
        None => {
            let way = direction(axis.horizontal(), delta > 0.0);
            (
                format!("{head} — move '{id}' {way} {}px", num(delta.abs())),
                None,
            )
        }
    };
    Diagnostic::advisory("align.near_miss", message, s.entry.span, Some(id.clone())).with_fix(fix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::lint::ledger::Entry;
    use crate::layout::LayoutBox;

    fn entry(id: String) -> Entry {
        Entry::for_test(&id, "rect", false)
    }

    /// The search before the sorted columns: every sibling against every
    /// sibling. Kept as the reference the fast search must match.
    fn reference(set: &[Sibling<'_>], authored: &BTreeMap<String, Authored>) -> Vec<Diagnostic> {
        if set.len() <= MIN_SHARED {
            return Vec::new();
        }
        let values: Vec<[Option<f64>; 5]> =
            set.iter().map(|s| Axis::ALL.map(|a| a.value(s))).collect();
        let mut out = Vec::new();
        for (n, s) in set.iter().enumerate() {
            let mut found: [Option<Miss>; 2] = [None, None];
            for (k, axis) in Axis::ALL.into_iter().enumerate() {
                let slot = usize::from(!axis.horizontal());
                if found[slot].is_some() {
                    continue;
                }
                let column = |i: usize| values[i][k];
                let Some(v) = column(n) else {
                    continue;
                };
                let others = |c: f64| -> Vec<usize> {
                    (0..set.len())
                        .filter(|&k| k != n)
                        .filter(|&k| column(k).is_some_and(|w| (w - c).abs() <= SHARED))
                        .collect()
                };
                let own = others(v).len();
                let me = &set[n];
                let mut best: Option<Miss> = None;
                for m in (0..set.len()).filter(|&m| m != n) {
                    let Some(c) = column(m) else { continue };
                    let off = (c - v).abs();
                    if !(MIN_OFF..=MAX_OFF).contains(&off) {
                        continue;
                    }
                    let members = others(c);
                    if members.len() < MIN_SHARED || members.len() <= own {
                        continue;
                    }
                    if !members
                        .iter()
                        .any(|&k| near(me, &set[k], axis.horizontal()))
                    {
                        continue;
                    }
                    let mut shared: Vec<f64> = members.iter().filter_map(|&k| column(k)).collect();
                    shared.sort_by(f64::total_cmp);
                    let c = shared[shared.len() / 2];
                    let off = (c - v).abs();
                    if !(MIN_OFF..=MAX_OFF).contains(&off) {
                        continue;
                    }
                    if let Some(own) = axis.box_value(me) {
                        let agree = members
                            .iter()
                            .filter(|&&k| {
                                axis.box_value(&set[k])
                                    .is_some_and(|b| (b - own).abs() <= SHARED)
                            })
                            .count();
                        if agree >= MIN_SHARED {
                            continue;
                        }
                    }
                    let better = best.as_ref().is_none_or(|b| {
                        members
                            .len()
                            .cmp(&b.members.len())
                            .then_with(|| (b.shared - v).abs().total_cmp(&off))
                            .then_with(|| b.shared.total_cmp(&c))
                            .is_gt()
                    });
                    if better {
                        best = Some(Miss {
                            axis,
                            value: v,
                            shared: c,
                            members,
                        });
                    }
                }
                found[slot] = best;
            }
            for miss in found.into_iter().flatten() {
                out.push(diagnostic(set, s, miss, authored));
            }
        }
        out
    }

    /// A deterministic generator (an LCG).
    struct Gen(u64);

    impl Gen {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0 >> 33
        }
    }

    /// Boxes on a coarse grid, some nudged 0.5–3.5px off it, some equal.
    fn boxes(n: usize, seed: u64) -> Vec<LayoutBox> {
        let mut g = Gen(seed);
        (0..n)
            .map(|_| {
                let col = (g.next() % 6) as f64 * 40.0;
                let row = (g.next() % 6) as f64 * 40.0;
                let jx =
                    [0.0, 0.0, 0.0, 0.1, 0.8, 1.5, 2.9, 3.2, -1.0, -2.5][(g.next() % 10) as usize];
                let jy = [0.0, 0.0, 0.2, 1.1, -0.9, 2.0][(g.next() % 6) as usize];
                let w = [20.0, 20.0, 22.0, 18.5][(g.next() % 4) as usize];
                LayoutBox {
                    x: col + jx,
                    y: row + jy,
                    w,
                    h: 20.0,
                }
            })
            .collect()
    }

    fn siblings<'a>(entries: &'a [Entry], rects: &[LayoutBox]) -> Vec<Sibling<'a>> {
        entries
            .iter()
            .zip(rects)
            .enumerate()
            .map(|(index, (entry, rect))| Sibling {
                index,
                entry,
                rect: *rect,
                text: None,
            })
            .collect()
    }

    #[test]
    fn the_sorted_search_matches_the_reference_exactly() {
        let authored = BTreeMap::new();
        let mut reported = 0;
        for seed in 0..60_u64 {
            let n = 3 + (seed as usize % 40);
            let rects = boxes(n, seed);
            let entries: Vec<Entry> = (0..n).map(|i| entry(format!("n{i}"))).collect();
            let set = siblings(&entries, &rects);
            let fast = near_miss(&set, &authored);
            let slow = reference(&set, &authored);
            let render =
                |d: &[Diagnostic]| -> Vec<String> { d.iter().map(|d| format!("{d:?}")).collect() };
            assert_eq!(render(&fast), render(&slow), "seed {seed}");
            reported += fast.len();
        }
        assert!(reported > 50, "the sets exercise near misses: {reported}");
    }

    /// The work grows near `n log n`: at most a fixed number of lookups
    /// and candidates per sibling and axis, for 2000 and 30000 siblings.
    #[test]
    fn work_stays_near_linear_in_the_sibling_count() {
        let authored = BTreeMap::new();
        for n in [2000_usize, 30_000] {
            let rects: Vec<LayoutBox> = (0..n)
                .map(|i| LayoutBox {
                    x: 20.0 + (i % 40) as f64 * 30.0 + if i % 17 == 0 { 1.5 } else { 0.0 },
                    y: 20.0 + (i / 40) as f64 * 30.0 + if i % 23 == 0 { 2.0 } else { 0.0 },
                    w: 20.0,
                    h: 20.0,
                })
                .collect();
            let entries: Vec<Entry> = (0..n).map(|i| entry(format!("r{i}"))).collect();
            let set = siblings(&entries, &rects);
            let mut work = Work::default();
            let found = near_miss_counted(&set, &authored, &mut work);
            assert!(!found.is_empty());
            let per = (n * Axis::ALL.len()) as u64;
            assert!(work.lookups <= 8 * per, "{n}: {work:?}");
            assert!(work.candidates <= 4 * per, "{n}: {work:?}");
        }
    }
}
