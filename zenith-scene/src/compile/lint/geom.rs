//! Rectangle math and the coverage shapes of opaque paint.

use crate::layout::LayoutBox;

/// The overlap of `a` and `b`, when it has positive area.
pub(super) fn intersect(a: LayoutBox, b: LayoutBox) -> Option<LayoutBox> {
    let left = a.x.max(b.x);
    let top = a.y.max(b.y);
    let right = (a.x + a.w).min(b.x + b.w);
    let bottom = (a.y + a.h).min(b.y + b.h);
    (right > left && bottom > top).then_some(LayoutBox {
        x: left,
        y: top,
        w: right - left,
        h: bottom - top,
    })
}

/// Every pair `(i, j)`, `i < j`, of `rects` that [`intersect`] accepts,
/// sorted, found by a sweep over the boxes sorted by left edge. With a
/// non-finite x edge every pair is tested.
pub(super) fn x_overlap_pairs(rects: &[LayoutBox]) -> Vec<(usize, usize)> {
    if !rects.iter().all(|r| r.x.is_finite() && r.w.is_finite()) {
        return (0..rects.len())
            .flat_map(|i| (i + 1..rects.len()).map(move |j| (i, j)))
            .filter(|&(i, j)| match (rects.get(i), rects.get(j)) {
                (Some(a), Some(b)) => intersect(*a, *b).is_some(),
                (None, _) | (_, None) => false,
            })
            .collect();
    }
    let mut order: Vec<(f64, f64, usize)> = rects
        .iter()
        .enumerate()
        .map(|(i, r)| (r.x, r.x + r.w, i))
        .collect();
    order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.2.cmp(&b.2)));
    let mut pairs = Vec::new();
    for (k, &(_, end, i)) in order.iter().enumerate() {
        for &(start, _, j) in order.iter().skip(k + 1) {
            // Sorted by left edge: no later box reaches back into `i`.
            if start >= end {
                break;
            }
            let (Some(a), Some(b)) = (rects.get(i), rects.get(j)) else {
                continue;
            };
            if intersect(*a, *b).is_some() {
                pairs.push((i.min(j), i.max(j)));
            }
        }
    }
    pairs.sort_unstable();
    pairs
}

/// The smallest box holding `a` and `b`.
pub(super) fn union(a: LayoutBox, b: LayoutBox) -> LayoutBox {
    let left = a.x.min(b.x);
    let top = a.y.min(b.y);
    LayoutBox {
        x: left,
        y: top,
        w: (a.x + a.w).max(b.x + b.w) - left,
        h: (a.y + a.h).max(b.y + b.h) - top,
    }
}

/// The union of `boxes`, or `None` when empty.
pub(super) fn bounds(boxes: &[LayoutBox]) -> Option<LayoutBox> {
    boxes.iter().copied().reduce(union)
}

/// Area of `b` in px².
pub(super) fn area(b: LayoutBox) -> f64 {
    b.w.max(0.0) * b.h.max(0.0)
}

/// The outline an opaque fill paints inside its box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Coverage {
    /// The whole box.
    Rect,
    /// The box with corners of this radius cut round.
    Rounded(f64),
    /// The ellipse inscribed in the box.
    Ellipse,
    /// The rhombus through the box's edge midpoints.
    Diamond,
}

impl Coverage {
    /// `true` when `(x, y)` lies inside the outline of `b`.
    pub(super) fn contains(self, b: LayoutBox, x: f64, y: f64) -> bool {
        if x < b.x || x > b.x + b.w || y < b.y || y > b.y + b.h {
            return false;
        }
        let (cx, cy) = (b.x + b.w / 2.0, b.y + b.h / 2.0);
        let (hw, hh) = (b.w / 2.0, b.h / 2.0);
        match self {
            Coverage::Rect => true,
            Coverage::Rounded(r) => {
                let r = r.min(hw).min(hh).max(0.0);
                let dx = ((x - cx).abs() - (hw - r)).max(0.0);
                let dy = ((y - cy).abs() - (hh - r)).max(0.0);
                dx * dx + dy * dy <= r * r
            }
            Coverage::Ellipse => {
                if hw <= 0.0 || hh <= 0.0 {
                    return false;
                }
                let (u, v) = ((x - cx) / hw, (y - cy) / hh);
                u * u + v * v <= 1.0
            }
            Coverage::Diamond => {
                if hw <= 0.0 || hh <= 0.0 {
                    return false;
                }
                (x - cx).abs() / hw + (y - cy).abs() / hh <= 1.0
            }
        }
    }
}

/// Sample grid side for curved coverage: 4 × 4 cell centres.
const GRID: usize = 4;

/// The fraction of `glyph` that the `shape` outline of `region` covers,
/// inside `clip` when set. Exact for a rectangle, sampled on a 4 × 4 grid
/// for a curved outline.
pub(super) fn covered_fraction(
    glyph: LayoutBox,
    region: LayoutBox,
    shape: Coverage,
    clip: Option<LayoutBox>,
) -> f64 {
    let glyph_area = area(glyph);
    if glyph_area <= 0.0 {
        return 0.0;
    }
    let visible = match clip {
        Some(c) => intersect(region, c),
        None => Some(region),
    };
    let Some(visible) = visible else {
        return 0.0;
    };
    match shape {
        Coverage::Rect => intersect(glyph, visible).map_or(0.0, |i| area(i) / glyph_area),
        Coverage::Rounded(_) | Coverage::Ellipse | Coverage::Diamond => {
            let mut inside = 0usize;
            for row in 0..GRID {
                for col in 0..GRID {
                    let x = glyph.x + glyph.w * (col as f64 + 0.5) / GRID as f64;
                    let y = glyph.y + glyph.h * (row as f64 + 0.5) / GRID as f64;
                    let in_clip = x >= visible.x
                        && x <= visible.x + visible.w
                        && y >= visible.y
                        && y <= visible.y + visible.h;
                    if in_clip && shape.contains(region, x, y) {
                        inside += 1;
                    }
                }
            }
            inside as f64 / (GRID * GRID) as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(x: f64, y: f64, w: f64, h: f64) -> LayoutBox {
        LayoutBox { x, y, w, h }
    }

    #[test]
    fn x_overlap_pairs_are_exactly_the_intersections() {
        let mut state = 3_u64;
        let mut next = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) as f64 / f64::from(u32::MAX >> 1)
        };
        for round in 0..30 {
            let rects: Vec<LayoutBox> = (0..40)
                .map(|_| {
                    b(
                        (next() * 300.0).round(),
                        (next() * 300.0).round(),
                        (next() * 80.0).round() - if round == 4 { 40.0 } else { 0.0 },
                        (next() * 80.0).round(),
                    )
                })
                .collect();
            let pairs = x_overlap_pairs(&rects);
            assert!(pairs.windows(2).all(|w| w[0] < w[1]), "sorted and unique");
            for i in 0..rects.len() {
                for j in i + 1..rects.len() {
                    let hit = intersect(rects[i], rects[j]).is_some();
                    assert_eq!(pairs.contains(&(i, j)), hit, "round {round}: ({i}, {j})");
                }
            }
        }
    }

    #[test]
    fn intersect_needs_positive_area() {
        assert_eq!(
            intersect(b(0.0, 0.0, 10.0, 10.0), b(5.0, 5.0, 10.0, 10.0)),
            Some(b(5.0, 5.0, 5.0, 5.0))
        );
        assert_eq!(
            intersect(b(0.0, 0.0, 10.0, 10.0), b(10.0, 0.0, 5.0, 5.0)),
            None
        );
    }

    #[test]
    fn rect_coverage_is_exact_and_clipped() {
        let glyph = b(0.0, 0.0, 10.0, 10.0);
        let cover = b(4.0, -5.0, 20.0, 20.0);
        assert!((covered_fraction(glyph, cover, Coverage::Rect, None) - 0.6).abs() < 1e-9);
        let clip = Some(b(0.0, 0.0, 7.0, 100.0));
        assert!((covered_fraction(glyph, cover, Coverage::Rect, clip) - 0.3).abs() < 1e-9);
    }

    #[test]
    fn curved_outlines_leave_corners_uncovered() {
        let region = b(0.0, 0.0, 100.0, 100.0);
        let corner = b(0.0, 0.0, 10.0, 10.0);
        assert_eq!(
            covered_fraction(corner, region, Coverage::Ellipse, None),
            0.0
        );
        assert_eq!(
            covered_fraction(corner, region, Coverage::Diamond, None),
            0.0
        );
        let centre = b(45.0, 45.0, 10.0, 10.0);
        assert_eq!(
            covered_fraction(centre, region, Coverage::Ellipse, None),
            1.0
        );
        assert!(Coverage::Rounded(10.0).contains(region, 50.0, 0.0));
        assert!(!Coverage::Rounded(10.0).contains(region, 0.5, 0.5));
    }
}
