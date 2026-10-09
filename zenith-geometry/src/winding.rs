//! Point-in-fill queries over arbitrary rings, self-intersecting ones
//! included, with the rules the backends fill by.

use crate::{CompoundFillRule, Point2};

/// The winding number of `ring` around `point`: the signed count of times
/// the ring turns around it. `ring` lists the vertices of one closed
/// polygon, its closing edge implied. Any ring is accepted: open lists,
/// repeated points, and self-intersections included. A ring with fewer than
/// two points, a non-finite point, or a point on the ring may give either
/// side; callers that need the boundary test it separately.
///
/// The sign follows the ring's direction; only zero versus non-zero and
/// parity carry meaning for a fill rule.
#[must_use]
pub fn winding_number(ring: &[Point2], point: Point2) -> i32 {
    let Some(&last) = ring.last() else {
        return 0;
    };
    let mut winding = 0_i32;
    let mut start = last;
    for &end in ring {
        // Upward or downward crossing of the horizontal ray to +x.
        let cross =
            (end.x - start.x) * (point.y - start.y) - (point.x - start.x) * (end.y - start.y);
        if start.y <= point.y {
            if end.y > point.y && cross > 0.0 {
                winding = winding.saturating_add(1);
            }
        } else if end.y <= point.y && cross < 0.0 {
            winding = winding.saturating_sub(1);
        }
        start = end;
    }
    winding
}

/// `true` when `point` lies in the region `rings` fill under `rule`, all
/// rings taken as one compound shape, as a renderer fills one path:
/// `NonZero` sums the winding numbers, `EvenOdd` takes their parity.
/// Points on an edge may fall either side.
#[must_use]
pub fn fill_contains_point(rings: &[Vec<Point2>], point: Point2, rule: CompoundFillRule) -> bool {
    let total = rings.iter().fold(0_i32, |sum, ring| {
        sum.saturating_add(winding_number(ring, point))
    });
    match rule {
        CompoundFillRule::NonZero => total != 0,
        CompoundFillRule::EvenOdd => total % 2 != 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(points: &[(f64, f64)]) -> Vec<Point2> {
        points
            .iter()
            .map(|&(x, y)| Point2::new_unchecked(x, y))
            .collect()
    }

    fn at(x: f64, y: f64) -> Point2 {
        Point2::new_unchecked(x, y)
    }

    #[test]
    fn square_winds_once_and_reversal_flips_the_sign() {
        let square = ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
        let inside = winding_number(&square, at(5.0, 5.0));
        assert_eq!(inside.abs(), 1);
        let mut reversed = square.clone();
        reversed.reverse();
        assert_eq!(winding_number(&reversed, at(5.0, 5.0)), -inside);
        assert_eq!(winding_number(&square, at(15.0, 5.0)), 0);
        assert_eq!(winding_number(&[], at(0.0, 0.0)), 0);
    }

    #[test]
    fn pentagram_centre_is_filled_by_nonzero_only() {
        // A five-point star drawn as one self-crossing ring: the centre
        // pentagon winds twice.
        let star: Vec<Point2> = (0..5)
            .map(|i| {
                let a = (f64::from(i) * 144.0 - 90.0).to_radians();
                at(50.0 + 40.0 * a.cos(), 50.0 + 40.0 * a.sin())
            })
            .collect();
        let rings = vec![star];
        assert_eq!(winding_number(&rings[0], at(50.0, 50.0)).abs(), 2);
        assert!(fill_contains_point(
            &rings,
            at(50.0, 50.0),
            CompoundFillRule::NonZero
        ));
        assert!(!fill_contains_point(
            &rings,
            at(50.0, 50.0),
            CompoundFillRule::EvenOdd
        ));
        // A star tip is filled under both rules.
        assert!(fill_contains_point(
            &rings,
            at(50.0, 15.0),
            CompoundFillRule::EvenOdd
        ));
    }

    #[test]
    fn hole_ring_cuts_by_rule() {
        let outer = ring(&[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)]);
        let same = ring(&[(25.0, 25.0), (75.0, 25.0), (75.0, 75.0), (25.0, 75.0)]);
        let mut opposite = same.clone();
        opposite.reverse();
        let centre = at(50.0, 50.0);
        let both = vec![outer.clone(), opposite];
        assert!(!fill_contains_point(
            &both,
            centre,
            CompoundFillRule::NonZero
        ));
        assert!(!fill_contains_point(
            &both,
            centre,
            CompoundFillRule::EvenOdd
        ));
        let same_dir = vec![outer, same];
        assert!(fill_contains_point(
            &same_dir,
            centre,
            CompoundFillRule::NonZero
        ));
        assert!(!fill_contains_point(
            &same_dir,
            centre,
            CompoundFillRule::EvenOdd
        ));
        assert!(fill_contains_point(
            &same_dir,
            at(10.0, 10.0),
            CompoundFillRule::EvenOdd
        ));
    }
}
