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

use std::collections::BTreeMap;

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

/// Every `align.near_miss` of one sibling set.
pub(super) fn near_miss(
    set: &[Sibling<'_>],
    authored: &BTreeMap<String, Authored>,
) -> Vec<Diagnostic> {
    if set.len() <= MIN_SHARED {
        return Vec::new();
    }
    let values: Vec<[Option<f64>; 5]> = set.iter().map(|s| Axis::ALL.map(|a| a.value(s))).collect();
    let mut out = Vec::new();
    for (n, s) in set.iter().enumerate() {
        let mut found: [Option<Miss>; 2] = [None, None];
        for (k, axis) in Axis::ALL.into_iter().enumerate() {
            let slot = usize::from(!axis.horizontal());
            if found.get(slot).is_some_and(Option::is_some) {
                continue;
            }
            let column = |i: usize| values.get(i).and_then(|v| v.get(k).copied().flatten());
            let Some(v) = column(n) else {
                continue;
            };
            if let (Some(miss), Some(cell)) =
                (find_miss(set, n, v, axis, &column), found.get_mut(slot))
            {
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
fn find_miss(
    set: &[Sibling<'_>],
    n: usize,
    v: f64,
    axis: Axis,
    column: &dyn Fn(usize) -> Option<f64>,
) -> Option<Miss> {
    let others = |c: f64| -> Vec<usize> {
        (0..set.len())
            .filter(|&k| k != n)
            .filter(|&k| column(k).is_some_and(|w| (w - c).abs() <= SHARED))
            .collect()
    };
    let own = others(v).len();
    let me = set.get(n)?;
    let mut best: Option<Miss> = None;
    for m in (0..set.len()).filter(|&m| m != n) {
        let Some(c) = column(m) else {
            continue;
        };
        let off = (c - v).abs();
        if !(MIN_OFF..=MAX_OFF).contains(&off) {
            continue;
        }
        let members = others(c);
        if members.len() < MIN_SHARED || members.len() <= own {
            continue;
        }
        let close = members
            .iter()
            .filter_map(|&k| set.get(k))
            .any(|o| near(me, o, axis.horizontal()));
        if !close {
            continue;
        }
        // The shared value: the median of the members' values.
        let mut shared: Vec<f64> = members.iter().filter_map(|&k| column(k)).collect();
        shared.sort_by(f64::total_cmp);
        let Some(&c) = shared.get(shared.len() / 2) else {
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
