use crate::{GeometryError, Point2, RectBounds, validation::validate_tolerance};

const MAX_FLATTEN_DEPTH: usize = 18;
const EXTREMA_DEDUP_EPSILON: f64 = 1.0e-12;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    pub p0: Point2,
    pub p1: Point2,
    pub p2: Point2,
    pub p3: Point2,
}

impl CubicBezier {
    pub fn new(p0: Point2, p1: Point2, p2: Point2, p3: Point2) -> Result<Self, GeometryError> {
        Self::validate_points(p0, p1, p2, p3)?;
        Ok(Self { p0, p1, p2, p3 })
    }

    #[must_use]
    pub const fn new_unchecked(p0: Point2, p1: Point2, p2: Point2, p3: Point2) -> Self {
        Self { p0, p1, p2, p3 }
    }

    #[must_use]
    pub fn evaluate(self, t: f64) -> Point2 {
        let ab = self.p0.lerp(self.p1, t);
        let bc = self.p1.lerp(self.p2, t);
        let cd = self.p2.lerp(self.p3, t);
        let abbc = ab.lerp(bc, t);
        let bccd = bc.lerp(cd, t);
        abbc.lerp(bccd, t)
    }

    #[must_use]
    pub fn derivative(self, t: f64) -> Point2 {
        let one_minus_t = 1.0 - t;
        let a = 3.0 * one_minus_t * one_minus_t;
        let b = 6.0 * one_minus_t * t;
        let c = 3.0 * t * t;

        Point2::new_unchecked(
            a * (self.p1.x - self.p0.x) + b * (self.p2.x - self.p1.x) + c * (self.p3.x - self.p2.x),
            a * (self.p1.y - self.p0.y) + b * (self.p2.y - self.p1.y) + c * (self.p3.y - self.p2.y),
        )
    }

    pub fn bounds(self) -> Result<RectBounds, GeometryError> {
        Self::validate_points(self.p0, self.p1, self.p2, self.p3)?;

        let mut bounds = RectBounds::from_point(self.p0).include_point(self.p3);

        for t in axis_extrema(self.p0.x, self.p1.x, self.p2.x, self.p3.x) {
            if is_unit_interval(t) {
                bounds = bounds.include_point(self.evaluate(t));
            }
        }

        for t in axis_extrema(self.p0.y, self.p1.y, self.p2.y, self.p3.y) {
            if is_unit_interval(t) {
                bounds = bounds.include_point(self.evaluate(t));
            }
        }

        Ok(bounds)
    }

    pub fn extrema(self) -> Result<Vec<f64>, GeometryError> {
        Self::validate_points(self.p0, self.p1, self.p2, self.p3)?;

        let mut extrema = Vec::new();
        push_axis_extrema(&mut extrema, self.p0.x, self.p1.x, self.p2.x, self.p3.x);
        push_axis_extrema(&mut extrema, self.p0.y, self.p1.y, self.p2.y, self.p3.y);
        extrema.sort_by(f64::total_cmp);

        let mut deduped = Vec::with_capacity(extrema.len());
        for t in extrema {
            match deduped.last() {
                Some(previous) if t - *previous <= EXTREMA_DEDUP_EPSILON => {}
                Some(_) | None => deduped.push(t),
            }
        }

        Ok(deduped)
    }

    pub fn flatten(self, tolerance: f64) -> Result<Vec<Point2>, GeometryError> {
        validate_tolerance(tolerance)?;
        Self::validate_points(self.p0, self.p1, self.p2, self.p3)?;

        let tolerance_squared = tolerance * tolerance;
        let mut points = Vec::new();
        points.push(self.p0);

        let mut stack = Vec::new();
        stack.push((self, 0_usize));

        while let Some((curve, depth)) = stack.pop() {
            if depth >= MAX_FLATTEN_DEPTH || curve.is_flat_enough(tolerance_squared) {
                points.push(curve.p3);
            } else {
                let (left, right) = curve.split_midpoint();
                stack.push((right, depth + 1));
                stack.push((left, depth + 1));
            }
        }

        Ok(points)
    }

    pub fn length(self, tolerance: f64) -> Result<f64, GeometryError> {
        let points = self.flatten(tolerance)?;
        Ok(points
            .windows(2)
            .filter_map(|segment| {
                let [start, end] = segment else {
                    return None;
                };
                Some(start.distance_squared(*end).sqrt())
            })
            .sum())
    }

    #[must_use]
    fn split_midpoint(self) -> (Self, Self) {
        let p01 = self.p0.midpoint(self.p1);
        let p12 = self.p1.midpoint(self.p2);
        let p23 = self.p2.midpoint(self.p3);
        let p012 = p01.midpoint(p12);
        let p123 = p12.midpoint(p23);
        let p0123 = p012.midpoint(p123);

        (
            Self::new_unchecked(self.p0, p01, p012, p0123),
            Self::new_unchecked(p0123, p123, p23, self.p3),
        )
    }

    #[must_use]
    fn is_flat_enough(self, tolerance_squared: f64) -> bool {
        self.p1.distance_squared_to_line(self.p0, self.p3) <= tolerance_squared
            && self.p2.distance_squared_to_line(self.p0, self.p3) <= tolerance_squared
    }

    fn validate_points(
        p0: Point2,
        p1: Point2,
        p2: Point2,
        p3: Point2,
    ) -> Result<(), GeometryError> {
        p0.validate()?;
        p1.validate()?;
        p2.validate()?;
        p3.validate()?;
        Ok(())
    }
}

fn axis_extrema(p0: f64, p1: f64, p2: f64, p3: f64) -> [f64; 2] {
    let a = -p0 + 3.0 * p1 - 3.0 * p2 + p3;
    let b = 2.0 * (p0 - 2.0 * p1 + p2);
    let c = p1 - p0;

    solve_quadratic(a, b, c)
}

fn push_axis_extrema(extrema: &mut Vec<f64>, p0: f64, p1: f64, p2: f64, p3: f64) {
    for t in axis_extrema(p0, p1, p2, p3) {
        if is_unit_interval(t) {
            extrema.push(t);
        }
    }
}

fn solve_quadratic(a: f64, b: f64, c: f64) -> [f64; 2] {
    if a == 0.0 {
        if b == 0.0 {
            [f64::NAN, f64::NAN]
        } else {
            [-c / b, f64::NAN]
        }
    } else {
        let discriminant = b.mul_add(b, -4.0 * a * c);
        if discriminant < 0.0 {
            [f64::NAN, f64::NAN]
        } else {
            let root = discriminant.sqrt();
            [(-b + root) / (2.0 * a), (-b - root) / (2.0 * a)]
        }
    }
}

fn is_unit_interval(t: f64) -> bool {
    t > 0.0 && t < 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_curve() -> CubicBezier {
        CubicBezier::new_unchecked(
            Point2::new_unchecked(0.0, 0.0),
            Point2::new_unchecked(0.0, 10.0),
            Point2::new_unchecked(10.0, 10.0),
            Point2::new_unchecked(10.0, 0.0),
        )
    }

    #[test]
    fn evaluates_endpoints() {
        let curve = sample_curve();

        assert_eq!(curve.evaluate(0.0), curve.p0);
        assert_eq!(curve.evaluate(1.0), curve.p3);
    }

    #[test]
    fn derivative_matches_endpoint_tangents() {
        let curve = sample_curve();

        assert_eq!(curve.derivative(0.0), Point2::new_unchecked(0.0, 30.0));
        assert_eq!(curve.derivative(1.0), Point2::new_unchecked(0.0, -30.0));
    }

    #[test]
    fn bounds_include_interior_extrema() {
        let curve = sample_curve();

        assert_eq!(
            curve.bounds(),
            Ok(RectBounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 10.0,
                max_y: 7.5
            })
        );
    }

    #[test]
    fn extrema_returns_sorted_deduplicated_interior_roots() {
        let curve = CubicBezier::new_unchecked(
            Point2::new_unchecked(0.0, 0.0),
            Point2::new_unchecked(1.0, 3.0),
            Point2::new_unchecked(0.0, -2.0),
            Point2::new_unchecked(1.0, 1.0),
        );

        assert_eq!(curve.extrema(), Ok(vec![0.25, 0.5, 0.75]));
    }

    #[test]
    fn extrema_excludes_endpoint_roots() {
        let curve = CubicBezier::new_unchecked(
            Point2::new_unchecked(0.0, 0.0),
            Point2::new_unchecked(0.0, 10.0),
            Point2::new_unchecked(10.0, 10.0),
            Point2::new_unchecked(10.0, 0.0),
        );

        assert_eq!(curve.extrema(), Ok(vec![0.5]));
    }

    #[test]
    fn flatten_preserves_endpoints() {
        let curve = sample_curve();
        let points = curve.flatten(0.5).expect("valid flattening");

        assert_eq!(points.first(), Some(&curve.p0));
        assert_eq!(points.last(), Some(&curve.p3));
    }

    #[test]
    fn tighter_flatten_tolerance_produces_at_least_as_many_points() {
        let curve = sample_curve();
        let coarse = curve.flatten(2.0).expect("valid flattening");
        let fine = curve.flatten(0.25).expect("valid flattening");

        assert!(fine.len() >= coarse.len());
    }

    #[test]
    fn degenerate_curve_flattens_to_endpoints() {
        let point = Point2::new_unchecked(3.0, 4.0);
        let curve = CubicBezier::new_unchecked(point, point, point, point);

        assert_eq!(curve.flatten(0.1), Ok(vec![point, point]));
        assert_eq!(curve.bounds(), Ok(RectBounds::from_point(point)));
        assert_eq!(curve.extrema(), Ok(Vec::new()));
    }

    #[test]
    fn length_of_straight_line_matches_chord() {
        let curve = CubicBezier::new_unchecked(
            Point2::new_unchecked(0.0, 0.0),
            Point2::new_unchecked(1.0, 0.0),
            Point2::new_unchecked(2.0, 0.0),
            Point2::new_unchecked(3.0, 0.0),
        );

        assert_eq!(curve.length(0.1), Ok(3.0));
    }

    #[test]
    fn curved_length_is_longer_than_chord() {
        let curve = sample_curve();
        let chord = curve.p0.distance_squared(curve.p3).sqrt();
        let length = curve.length(0.25).expect("valid length");

        assert!(length > chord);
    }

    #[test]
    fn length_matches_flattened_polyline_length() {
        let curve = sample_curve();
        let tolerance = 0.25;
        let points = curve.flatten(tolerance).expect("valid flattening");
        let flattened_length = points
            .windows(2)
            .filter_map(|segment| {
                let [start, end] = segment else {
                    return None;
                };
                Some(start.distance_squared(*end).sqrt())
            })
            .sum::<f64>();

        assert_eq!(curve.length(tolerance), Ok(flattened_length));
    }

    #[test]
    fn rejects_invalid_inputs() {
        let invalid_point = Point2::new_unchecked(f64::NAN, 0.0);
        assert_eq!(
            CubicBezier::new(invalid_point, invalid_point, invalid_point, invalid_point),
            Err(GeometryError::NonFinitePoint)
        );

        let invalid_curve = CubicBezier::new_unchecked(
            invalid_point,
            Point2::new_unchecked(0.0, 0.0),
            Point2::new_unchecked(1.0, 1.0),
            Point2::new_unchecked(2.0, 0.0),
        );
        assert_eq!(invalid_curve.bounds(), Err(GeometryError::NonFinitePoint));
        assert_eq!(invalid_curve.extrema(), Err(GeometryError::NonFinitePoint));
        assert_eq!(
            invalid_curve.flatten(0.5),
            Err(GeometryError::NonFinitePoint)
        );
        assert_eq!(
            invalid_curve.length(0.5),
            Err(GeometryError::NonFinitePoint)
        );

        let curve = sample_curve();
        assert_eq!(
            curve.flatten(f64::NAN),
            Err(GeometryError::NonFiniteTolerance)
        );
        assert_eq!(curve.flatten(0.0), Err(GeometryError::NonPositiveTolerance));
        assert_eq!(
            curve.length(f64::NAN),
            Err(GeometryError::NonFiniteTolerance)
        );
        assert_eq!(curve.length(0.0), Err(GeometryError::NonPositiveTolerance));
    }
}
