//! Pure one-dimensional layout math: fill distribution with min/max freezing,
//! line breaking, main-axis justification, and cross-axis alignment.
//!
//! Every function is deterministic f64 arithmetic in source order.

use super::model::{Align, Justify};

/// Tolerance in px before laid-out content counts as overflowing its box.
pub(super) const OVERFLOW_EPSILON: f64 = 0.5;

/// Tolerance in px for fitting an item on a wrapped line.
const BREAK_EPSILON: f64 = 1e-6;

/// Split `free` px equally over the fill items, each clamped to its
/// `(min, max)`.
///
/// CSS-style freeze loop: each round gives every unfrozen item an equal share,
/// clamps it, and freezes the min violators (when the clamps added space) or
/// the max violators (when they removed space). It ends when no clamp moves a
/// share, after at most `fills.len()` rounds.
pub(super) fn distribute_fill(free: f64, fills: &[(f64, f64)]) -> Vec<f64> {
    let mut sizes = vec![0.0_f64; fills.len()];
    let mut frozen = vec![false; fills.len()];
    for _ in 0..=fills.len() {
        let open = frozen.iter().filter(|f| !**f).count();
        if open == 0 {
            break;
        }
        let used: f64 = sizes
            .iter()
            .zip(&frozen)
            .filter(|(_, f)| **f)
            .map(|(s, _)| *s)
            .sum();
        let share = (free - used) / open as f64;
        let mut violations = vec![0.0_f64; fills.len()];
        let mut total = 0.0;
        for (((size, f), (min, max)), v) in sizes
            .iter_mut()
            .zip(&frozen)
            .zip(fills)
            .zip(violations.iter_mut())
        {
            if *f {
                continue;
            }
            let clamped = share.min(*max).max(*min);
            *v = clamped - share;
            total += *v;
            *size = clamped;
        }
        if total.abs() < 1e-9 {
            break;
        }
        for (f, v) in frozen.iter_mut().zip(&violations) {
            if !*f && ((total > 0.0 && *v > 0.0) || (total < 0.0 && *v < 0.0)) {
                *f = true;
            }
        }
    }
    sizes
}

/// Greedy line breaking: each line takes items while they fit in `limit`
/// with `gap` between them. A line always takes at least one item.
///
/// Returns half-open `(start, end)` index ranges in source order.
pub(super) fn break_lines(sizes: &[f64], gap: f64, limit: f64) -> Vec<(usize, usize)> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut used = 0.0;
    for (i, size) in sizes.iter().enumerate() {
        if i == start {
            used = *size;
        } else if used + gap + size > limit + BREAK_EPSILON {
            lines.push((start, i));
            start = i;
            used = *size;
        } else {
            used = used + gap + size;
        }
    }
    if start < sizes.len() {
        lines.push((start, sizes.len()));
    }
    lines
}

/// The summed size of `sizes` with `gap` between neighbours.
pub(super) fn span(sizes: &[f64], gap: f64) -> f64 {
    let mut total = 0.0;
    for (i, s) in sizes.iter().enumerate() {
        if i > 0 {
            total += gap;
        }
        total += s;
    }
    total
}

/// Positions of a line's items along the main axis, from `start`.
///
/// `inner` is the content size when fixed. A hugging axis has no free space,
/// so every `justify` packs at `start`. `space-between` with one item or
/// negative free space packs at `start`; `center` and `end` may start before
/// `start` when the items overflow.
pub(super) fn justify_positions(
    start: f64,
    sizes: &[f64],
    gap: f64,
    inner: Option<f64>,
    justify: Justify,
) -> Vec<f64> {
    let free = inner.map_or(0.0, |c| c - span(sizes, gap));
    let (lead, spacing) = match justify {
        Justify::Start => (0.0, gap),
        Justify::Center => (free / 2.0, gap),
        Justify::End => (free, gap),
        Justify::SpaceBetween if sizes.len() > 1 && free > 0.0 => {
            (0.0, gap + free / (sizes.len() - 1) as f64)
        }
        Justify::SpaceBetween => (0.0, gap),
    };
    let mut cursor = if lead == 0.0 { start } else { start + lead };
    let mut out = Vec::with_capacity(sizes.len());
    let last = sizes.len().saturating_sub(1);
    for (i, s) in sizes.iter().enumerate() {
        out.push(cursor);
        cursor += s;
        if i != last {
            cursor += spacing;
        }
    }
    out
}

/// The offset of an item `size` px tall inside a line `line` px tall.
pub(super) fn align_offset(size: f64, line: f64, align: Align) -> f64 {
    match align {
        Align::Start | Align::Stretch => 0.0,
        Align::Center => (line - size) / 2.0,
        Align::End => line - size,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_splits_equally() {
        assert_eq!(
            distribute_fill(90.0, &[(0.0, f64::INFINITY); 3]),
            vec![30.0; 3]
        );
    }

    #[test]
    fn fill_freezes_max_violators_and_redistributes() {
        let sizes = distribute_fill(100.0, &[(0.0, 20.0), (0.0, f64::INFINITY)]);
        assert_eq!(sizes, vec![20.0, 80.0]);
    }

    #[test]
    fn fill_freezes_min_violators() {
        let sizes = distribute_fill(100.0, &[(70.0, f64::INFINITY), (0.0, f64::INFINITY)]);
        assert_eq!(sizes, vec![70.0, 30.0]);
    }

    #[test]
    fn negative_free_space_clamps_to_zero() {
        assert_eq!(
            distribute_fill(-10.0, &[(0.0, f64::INFINITY); 2]),
            vec![0.0, 0.0]
        );
    }

    #[test]
    fn min_wins_over_max() {
        assert_eq!(distribute_fill(10.0, &[(50.0, 20.0)]), vec![50.0]);
    }

    #[test]
    fn lines_break_greedily() {
        assert_eq!(
            break_lines(&[40.0, 40.0, 40.0, 100.0, 10.0], 10.0, 100.0),
            vec![(0, 2), (2, 3), (3, 4), (4, 5)]
        );
        assert_eq!(break_lines(&[], 10.0, 100.0), Vec::<(usize, usize)>::new());
        assert_eq!(break_lines(&[200.0], 10.0, 100.0), vec![(0, 1)]);
    }

    #[test]
    fn justify_matrix() {
        let sizes = [10.0, 20.0];
        assert_eq!(
            justify_positions(5.0, &sizes, 4.0, Some(100.0), Justify::Start),
            vec![5.0, 19.0]
        );
        assert_eq!(
            justify_positions(0.0, &sizes, 4.0, Some(100.0), Justify::Center),
            vec![33.0, 47.0]
        );
        assert_eq!(
            justify_positions(0.0, &sizes, 4.0, Some(100.0), Justify::End),
            vec![66.0, 80.0]
        );
        assert_eq!(
            justify_positions(0.0, &sizes, 4.0, Some(100.0), Justify::SpaceBetween),
            vec![0.0, 80.0]
        );
        assert_eq!(
            justify_positions(0.0, &sizes, 4.0, None, Justify::End),
            vec![0.0, 14.0]
        );
        assert_eq!(
            justify_positions(0.0, &[10.0], 4.0, Some(100.0), Justify::SpaceBetween),
            vec![0.0]
        );
    }

    #[test]
    fn align_matrix() {
        assert_eq!(align_offset(10.0, 30.0, Align::Start), 0.0);
        assert_eq!(align_offset(10.0, 30.0, Align::Center), 10.0);
        assert_eq!(align_offset(10.0, 30.0, Align::End), 20.0);
        assert_eq!(align_offset(10.0, 30.0, Align::Stretch), 0.0);
    }
}
