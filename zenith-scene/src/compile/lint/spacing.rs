//! `spacing.uneven_gap`: three or more siblings in a row or column with
//! nearly, but not exactly, equal gaps.
//!
//! Two checked siblings (see [`super::arrange`]) line up in a row when their
//! boxes overlap on y by more than 60% of the shorter height, their heights
//! differ by at most 25% of the taller, and their x ranges do not overlap. A
//! row is a connected set of such pairs with at least 3 members; its gaps
//! are measured between neighbours in x order, and a row whose neighbours
//! overlap is skipped. A column is the same with the axes swapped.
//!
//! The check fires when the gap spread (largest minus smallest) is over
//! 0.5px and at most max(2px, 25% of the median gap), capped at 12px. A
//! larger spread is deliberate spacing. The fix moves one node, and is set
//! only when exactly one node can move to make every gap equal. Children of
//! a `row` / `column` / `grid` frame are not checked: the frame `gap` spaces
//! them.

use std::collections::BTreeMap;

use zenith_core::Diagnostic;

use crate::layout::LayoutBox;

use super::arrange::{Sibling, direction, id_list, moved, num, set_px};
use super::paint::Authored;

/// Smallest member count of a row or column.
const MIN_MEMBERS: usize = 3;
/// Cross-axis overlap share of the smaller cross extent.
const MIN_CROSS_OVERLAP: f64 = 0.6;
/// Largest cross-extent difference, as a share of the larger extent.
const MAX_EXTENT_DIFF: f64 = 0.25;
/// Gap spread at or below which gaps count as equal, in px.
const EVEN: f64 = 0.5;
/// Floor of the largest reported spread, in px.
const SPREAD_FLOOR: f64 = 2.0;
/// Largest reported spread as a share of the median gap.
const SPREAD_SHARE: f64 = 0.25;
/// Cap of the largest reported spread, in px.
const SPREAD_CAP: f64 = 12.0;
/// Gaps within this distance are equal when finding the outlier, in px.
const SAME_GAP: f64 = 0.25;

/// Main-axis start and length, cross-axis start and length.
fn spans(b: LayoutBox, horizontal: bool) -> ((f64, f64), (f64, f64)) {
    if horizontal {
        ((b.x, b.w), (b.y, b.h))
    } else {
        ((b.y, b.h), (b.x, b.w))
    }
}

/// `true` when `a` and `b` line up along the main axis.
fn lined_up(a: LayoutBox, b: LayoutBox, horizontal: bool) -> bool {
    let ((am, aw), (ac, ah)) = spans(a, horizontal);
    let ((bm, bw), (bc, bh)) = spans(b, horizontal);
    let overlap = (ac + ah).min(bc + bh) - ac.max(bc);
    let separate = am + aw <= bm + SAME_GAP || bm + bw <= am + SAME_GAP;
    overlap > MIN_CROSS_OVERLAP * ah.min(bh)
        && (ah - bh).abs() <= MAX_EXTENT_DIFF * ah.max(bh)
        && separate
}

/// Every `spacing.uneven_gap` of one sibling set.
pub(super) fn uneven_gaps(
    set: &[Sibling<'_>],
    authored: &BTreeMap<String, Authored>,
) -> Vec<Diagnostic> {
    if set.len() < MIN_MEMBERS {
        return Vec::new();
    }
    let mut out = Vec::new();
    for horizontal in [true, false] {
        for mut run in lines(set, horizontal) {
            run.sort_by(|a, b| {
                spans(a.rect, horizontal)
                    .0
                    .0
                    .total_cmp(&spans(b.rect, horizontal).0.0)
                    .then(a.index.cmp(&b.index))
            });
            if let Some(d) = judge(&run, horizontal, authored) {
                out.push(d);
            }
        }
    }
    out
}

/// The connected sets of lined-up siblings with at least 3 members.
fn lines<'a>(set: &[Sibling<'a>], horizontal: bool) -> Vec<Vec<Sibling<'a>>> {
    // Union-find over the lined-up pairs.
    let mut root: Vec<usize> = (0..set.len()).collect();
    for (i, a) in set.iter().enumerate() {
        for (j, b) in set.iter().enumerate().skip(i + 1) {
            if lined_up(a.rect, b.rect, horizontal) {
                let (ri, rj) = (find(&root, i), find(&root, j));
                if let Some(slot) = root.get_mut(ri.max(rj)) {
                    *slot = ri.min(rj);
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<Sibling<'a>>> = BTreeMap::new();
    for (i, s) in set.iter().enumerate() {
        let r = find(&root, i);
        groups.entry(r).or_default().push(*s);
    }
    groups
        .into_values()
        .filter(|g| g.len() >= MIN_MEMBERS)
        .collect()
}

/// The set root of `i`.
fn find(root: &[usize], mut i: usize) -> usize {
    while let Some(&p) = root.get(i) {
        if p == i {
            break;
        }
        i = p;
    }
    i
}

/// The diagnostic of one sorted row or column, when its gaps are uneven.
fn judge(
    run: &[Sibling<'_>],
    horizontal: bool,
    authored: &BTreeMap<String, Authored>,
) -> Option<Diagnostic> {
    let mut gaps = Vec::with_capacity(run.len());
    for pair in run.windows(2) {
        let [a, b] = pair else {
            continue;
        };
        let ((am, aw), _) = spans(a.rect, horizontal);
        let ((bm, _), _) = spans(b.rect, horizontal);
        let gap = bm - (am + aw);
        if gap < -SAME_GAP {
            return None;
        }
        gaps.push(gap.max(0.0));
    }
    let lo = gaps.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = gaps.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let spread = hi - lo;
    let median = median(&gaps)?;
    let limit = SPREAD_FLOOR.max(SPREAD_SHARE * median).min(SPREAD_CAP);
    if !(spread > EVEN && spread <= limit) {
        return None;
    }
    let (word, property) = if horizontal {
        ("row", "x")
    } else {
        ("column", "y")
    };
    let names = id_list(run.iter().map(|s| s.entry.id.as_str()), 6);
    let listed: Vec<String> = gaps.iter().map(|g| num(*g)).collect();
    let head = format!(
        "{word} {names} has uneven gaps {}px (spread {}px, median {}px)",
        listed.join(","),
        num(spread),
        num(median)
    );
    let outlier = sole_outlier(&gaps);
    let subject = match outlier {
        Some((i, _, _)) => run.get(i),
        None => worst(&gaps, median).and_then(|i| run.get(i)),
    }?;
    let (message, fix) = match outlier {
        Some((_, delta, even)) => {
            let way = direction(horizontal, delta > 0.0);
            let id = &subject.entry.id;
            match moved(subject, authored, horizontal, delta) {
                Some(at) => (
                    format!(
                        "{head} — set {property}=(px){} on '{id}' for {}px gaps",
                        num(at),
                        num(even)
                    ),
                    Some(set_px(property, at)),
                ),
                None => (
                    format!(
                        "{head} — move '{id}' {way} {}px for {}px gaps",
                        num(delta.abs()),
                        num(even)
                    ),
                    None,
                ),
            }
        }
        None => (
            format!(
                "{head} — set every gap to {}px; no single node move evens them",
                num(median)
            ),
            None,
        ),
    };
    Some(
        Diagnostic::advisory(
            "spacing.uneven_gap",
            message,
            subject.entry.span,
            Some(subject.entry.id.clone()),
        )
        .with_fix(fix),
    )
}

fn median(values: &[f64]) -> Option<f64> {
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let mid = v.len() / 2;
    if v.len() % 2 == 1 {
        v.get(mid).copied()
    } else {
        Some((v.get(mid.checked_sub(1)?)? + v.get(mid)?) / 2.0)
    }
}

/// `true` when every gap except the `skip` ones equals `t`.
fn rest_equal(gaps: &[f64], skip: &[usize], t: f64) -> bool {
    gaps.iter()
        .enumerate()
        .filter(|(i, _)| !skip.contains(i))
        .all(|(_, g)| (g - t).abs() <= SAME_GAP)
}

/// The one node whose move evens every gap: `(node, delta, even gap)`.
/// Node `i` sits after gap `i - 1` and before gap `i`. `None` when no node
/// or more than one node can.
fn sole_outlier(gaps: &[f64]) -> Option<(usize, f64, f64)> {
    let n = gaps.len();
    let mut found: Vec<(usize, f64, f64)> = Vec::new();
    // The first node: only gap 0 changes.
    if let Some(&t) = gaps.get(1)
        && rest_equal(gaps, &[0], t)
        && let Some(&g) = gaps.first()
    {
        found.push((0, g - t, t));
    }
    // The last node: only the last gap changes.
    if let (Some(&g), Some(&t)) = (gaps.last(), n.checked_sub(2).and_then(|i| gaps.get(i)))
        && rest_equal(gaps, &[n - 1], t)
    {
        found.push((n, t - g, t));
    }
    // An inner node: the gaps on both sides change, their sum stays.
    for i in 1..n {
        let (Some(&before), Some(&after)) = (gaps.get(i - 1), gaps.get(i)) else {
            continue;
        };
        let t = (before + after) / 2.0;
        let others: Vec<f64> = gaps
            .iter()
            .enumerate()
            .filter(|(k, _)| *k != i - 1 && *k != i)
            .map(|(_, g)| *g)
            .collect();
        let t = others.first().copied().unwrap_or(t);
        if rest_equal(gaps, &[i - 1, i], t) && (before + after - 2.0 * t).abs() <= SAME_GAP {
            found.push((i, t - before, t));
        }
    }
    found.retain(|(_, delta, _)| delta.abs() > SAME_GAP);
    match found.as_slice() {
        [one] => Some(*one),
        _ => None,
    }
}

/// The node after the gap farthest from the median.
fn worst(gaps: &[f64], median: f64) -> Option<usize> {
    gaps.iter()
        .enumerate()
        .max_by(|(i, a), (j, b)| {
            (*a - median)
                .abs()
                .total_cmp(&(*b - median).abs())
                .then(j.cmp(i))
        })
        .map(|(i, _)| i + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_node_is_the_sole_outlier() {
        let (node, delta, even) = sole_outlier(&[24.0, 24.0, 27.0]).expect("outlier");
        assert_eq!(node, 3);
        assert!((delta + 3.0).abs() < 1e-9);
        assert!((even - 24.0).abs() < 1e-9);
    }

    #[test]
    fn an_inner_node_is_the_sole_outlier() {
        let (node, delta, _) = sole_outlier(&[24.0, 26.0, 22.0, 24.0]).expect("outlier");
        assert_eq!(node, 2);
        assert!((delta + 2.0).abs() < 1e-9);
    }

    #[test]
    fn three_nodes_have_no_sole_outlier() {
        assert_eq!(sole_outlier(&[24.0, 26.0]), None);
    }

    #[test]
    fn worst_names_the_node_after_the_widest_deviation() {
        assert_eq!(worst(&[24.0, 30.0, 24.0], 24.0), Some(2));
    }
}
