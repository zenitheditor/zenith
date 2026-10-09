//! Convex region tests for area selection: [`hit_region`], and the
//! polygon math under [`CompiledBox::touches_region`] and
//! [`HitShape::touches`](super::HitShape::touches).

use std::collections::BTreeMap;

use super::compiled::CompiledBox;

/// A point in page px.
pub(super) type Pt = (f64, f64);

/// Slack for edge tests, in px.
const EPS: f64 = 1e-9;

/// The ids of every box that meets the convex polygon `region` (page px,
/// either winding), topmost first.
///
/// A box counts when [`CompiledBox::touches_region`] holds. Order and the
/// `hidden` rule match [`hit_test`](super::hit_test): `paint_order` largest
/// first, `visible=false` boxes never count. Ids come back raw; map them
/// with [`selectable_id`](super::selectable_id). A region with fewer than
/// three points, a non-finite point, or no area counts nothing.
#[must_use]
pub fn hit_region<'b>(boxes: &'b BTreeMap<String, CompiledBox>, region: &[Pt]) -> Vec<&'b str> {
    if !usable(region) {
        return Vec::new();
    }
    let mut hits: Vec<(&str, usize)> = boxes
        .iter()
        .filter(|(_, b)| !b.hidden && b.touches_region(region))
        .map(|(id, b)| (id.as_str(), b.paint_order))
        .collect();
    hits.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    hits.into_iter().map(|(id, _)| id).collect()
}

/// `true` when `region` has at least three finite points and some area.
pub(super) fn usable(region: &[Pt]) -> bool {
    region.len() >= 3
        && region.iter().all(|p| p.0.is_finite() && p.1.is_finite())
        && signed_area(region).abs() > EPS
}

/// Twice the signed area of `poly` (positive when counter-clockwise in a
/// y-up frame).
fn signed_area(poly: &[Pt]) -> f64 {
    edges(poly).map(|(a, b)| a.0 * b.1 - b.0 * a.1).sum()
}

/// The edges of the closed polygon `poly`.
pub(super) fn edges(poly: &[Pt]) -> impl Iterator<Item = (Pt, Pt)> + '_ {
    let n = poly.len();
    (0..n).filter_map(move |i| Some((*poly.get(i)?, *poly.get((i + 1) % n)?)))
}

fn cross(o: Pt, a: Pt, b: Pt) -> f64 {
    (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
}

/// `true` when `p` lies inside the convex polygon `poly` (edges included).
pub(super) fn point_in_convex(p: Pt, poly: &[Pt]) -> bool {
    let sign = signed_area(poly).signum();
    sign != 0.0 && edges(poly).all(|(a, b)| sign * cross(a, b, p) >= -EPS)
}

/// The part of `subject` (any polygon) inside the convex polygon `clip`
/// (Sutherland–Hodgman). Empty when they do not meet.
pub(super) fn clip_convex(subject: &[Pt], clip: &[Pt]) -> Vec<Pt> {
    let sign = signed_area(clip).signum();
    if sign == 0.0 {
        return Vec::new();
    }
    let mut out: Vec<Pt> = subject.to_vec();
    for (a, b) in edges(clip) {
        let input = std::mem::take(&mut out);
        let inside = |p: Pt| sign * cross(a, b, p) >= -EPS;
        let n = input.len();
        for i in 0..n {
            let (Some(&cur), Some(&prev)) = (input.get(i), input.get((i + n - 1) % n)) else {
                continue;
            };
            match (inside(cur), inside(prev)) {
                (true, true) => out.push(cur),
                (true, false) => {
                    out.extend(intersection(prev, cur, a, b));
                    out.push(cur);
                }
                (false, true) => out.extend(intersection(prev, cur, a, b)),
                (false, false) => {}
            }
        }
        if out.is_empty() {
            break;
        }
    }
    out
}

/// Where segment `p`–`q` crosses the line through `a`–`b`.
fn intersection(p: Pt, q: Pt, a: Pt, b: Pt) -> Option<Pt> {
    let d1 = cross(a, b, p);
    let d2 = cross(a, b, q);
    let den = d1 - d2;
    if den.abs() < f64::MIN_POSITIVE {
        return None;
    }
    let t = d1 / den;
    Some((p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t))
}

/// `true` when the convex polygons `a` and `b` meet (separating axis test).
/// A polygon with no area (a segment or a point) counts as that segment
/// or point.
pub(super) fn convex_overlap(a: &[Pt], b: &[Pt]) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    for poly in [a, b] {
        for (p, q) in edges(poly) {
            let axis = (-(q.1 - p.1), q.0 - p.0);
            if axis.0 == 0.0 && axis.1 == 0.0 {
                continue;
            }
            let (amin, amax) = project(a, axis);
            let (bmin, bmax) = project(b, axis);
            if amax < bmin - EPS || bmax < amin - EPS {
                return false;
            }
        }
    }
    true
}

fn project(poly: &[Pt], axis: Pt) -> (f64, f64) {
    poly.iter()
        .map(|p| p.0 * axis.0 + p.1 * axis.1)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        })
}

/// The distance from segment `p`–`q` to the convex polygon `poly`: 0 when
/// it touches or lies inside.
pub(super) fn segment_polygon_distance(p: Pt, q: Pt, poly: &[Pt]) -> f64 {
    if point_in_convex(p, poly) || point_in_convex(q, poly) {
        return 0.0;
    }
    edges(poly)
        .map(|(a, b)| segment_distance(p, q, a, b))
        .fold(f64::INFINITY, f64::min)
}

/// The distance between segments `p`–`q` and `a`–`b`.
fn segment_distance(p: Pt, q: Pt, a: Pt, b: Pt) -> f64 {
    let d1 = cross(a, b, p);
    let d2 = cross(a, b, q);
    let d3 = cross(p, q, a);
    let d4 = cross(p, q, b);
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return 0.0;
    }
    [
        point_segment(p, a, b),
        point_segment(q, a, b),
        point_segment(a, p, q),
        point_segment(b, p, q),
    ]
    .into_iter()
    .fold(f64::INFINITY, f64::min)
}

/// The distance from `p` to segment `a`–`b`.
fn point_segment(p: Pt, a: Pt, b: Pt) -> f64 {
    let (vx, vy) = (b.0 - a.0, b.1 - a.1);
    let len2 = vx * vx + vy * vy;
    let t = if len2 > 0.0 {
        (((p.0 - a.0) * vx + (p.1 - a.1) * vy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.0 - (a.0 + t * vx)).hypot(p.1 - (a.1 + t * vy))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: [Pt; 4] = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];

    #[test]
    fn overlap_and_containment_in_either_winding() {
        let shifted: Vec<Pt> = SQUARE.iter().map(|p| (p.0 + 9.0, p.1 + 9.0)).collect();
        assert!(convex_overlap(&SQUARE, &shifted));
        let far: Vec<Pt> = SQUARE.iter().map(|p| (p.0 + 11.0, p.1)).collect();
        assert!(!convex_overlap(&SQUARE, &far));
        let reversed: Vec<Pt> = SQUARE.iter().rev().copied().collect();
        assert!(point_in_convex((5.0, 5.0), &reversed));
        assert!(point_in_convex((10.0, 5.0), &SQUARE));
        assert!(!point_in_convex((10.5, 5.0), &SQUARE));
        // A diamond whose bounds overlap the square but whose body does not.
        let diamond = [(16.0, 5.0), (21.0, 10.0), (16.0, 15.0), (11.0, 10.0)];
        assert!(!convex_overlap(&SQUARE, &diamond));
    }

    #[test]
    fn segments_count_by_distance() {
        let flat = [(2.0, 12.0), (8.0, 12.0), (8.0, 12.0), (2.0, 12.0)];
        assert!(!convex_overlap(&SQUARE, &flat));
        let crossing = [(5.0, -5.0), (5.0, 15.0), (5.0, 15.0), (5.0, -5.0)];
        assert!(convex_overlap(&SQUARE, &crossing));
        assert!((segment_polygon_distance((12.0, 0.0), (12.0, 10.0), &SQUARE) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn clipping_keeps_the_common_part() {
        let half = [(5.0, -1.0), (20.0, -1.0), (20.0, 20.0), (5.0, 20.0)];
        let part = clip_convex(&SQUARE, &half);
        assert!(
            (signed_area(&part).abs() / 2.0 - 50.0).abs() < 1e-9,
            "{part:?}"
        );
        let away = [(30.0, 30.0), (40.0, 30.0), (40.0, 40.0)];
        assert!(clip_convex(&SQUARE, &away).is_empty());
        assert!(!usable(&[(0.0, 0.0), (1.0, 1.0)]));
        assert!(!usable(&[(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)]));
    }
}
