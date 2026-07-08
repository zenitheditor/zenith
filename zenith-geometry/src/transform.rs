use crate::{CubicBezier, GeometryError, Point2};

const MAX_RADIAL_SYMMETRY_COUNT: usize = 72;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AffineTransform {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl AffineTransform {
    #[must_use]
    pub const fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    pub fn translation(dx: f64, dy: f64) -> Result<Self, GeometryError> {
        if !dx.is_finite() || !dy.is_finite() {
            return Err(GeometryError::NonFiniteParameter);
        }

        Self::new(1.0, 0.0, 0.0, 1.0, dx, dy)
    }

    pub fn rotation(angle_degrees: f64, pivot: Point2) -> Result<Self, GeometryError> {
        if !angle_degrees.is_finite() {
            return Err(GeometryError::NonFiniteParameter);
        }
        pivot.validate()?;

        let angle = angle_degrees.to_radians();
        let sin = angle.sin();
        let cos = angle.cos();
        let e = pivot.x - cos.mul_add(pivot.x, -sin * pivot.y);
        let f = pivot.y - sin.mul_add(pivot.x, cos * pivot.y);

        Self::new(cos, sin, -sin, cos, e, f)
    }

    pub fn reflection_across_line(start: Point2, end: Point2) -> Result<Self, GeometryError> {
        start.validate()?;
        end.validate()?;

        let dx = end.x - start.x;
        let dy = end.y - start.y;
        if !dx.is_finite() || !dy.is_finite() {
            return Err(GeometryError::NonFiniteTransform);
        }
        let length = dx.hypot(dy);
        if !length.is_finite() {
            return Err(GeometryError::NonFiniteTransform);
        }
        if length == 0.0 {
            return Err(GeometryError::DegenerateLine);
        }

        let ux = dx / length;
        let uy = dy / length;
        let a = 2.0 * ux * ux - 1.0;
        let b = 2.0 * ux * uy;
        let c = b;
        let d = 2.0 * uy * uy - 1.0;
        let e = start.x - (a * start.x + c * start.y);
        let f = start.y - (b * start.x + d * start.y);

        Self::new(a, b, c, d, e, f)
    }

    pub fn radial_symmetry(
        count: usize,
        center: Point2,
        start_angle_degrees: f64,
    ) -> Result<Vec<Self>, GeometryError> {
        if count == 0 {
            return Err(GeometryError::NonPositiveCount);
        }
        if count > MAX_RADIAL_SYMMETRY_COUNT {
            return Err(GeometryError::CountOutOfRange);
        }
        if !start_angle_degrees.is_finite() {
            return Err(GeometryError::NonFiniteParameter);
        }
        center.validate()?;

        let step_degrees = 360.0 / count as f64;
        let mut transforms = Vec::with_capacity(count);
        for index in 0..count {
            transforms.push(Self::rotation(
                start_angle_degrees + step_degrees * index as f64,
                center,
            )?);
        }

        Ok(transforms)
    }

    /// Returns a transform that applies `self` first, then applies `next`.
    pub fn compose(self, next: Self) -> Result<Self, GeometryError> {
        self.validate()?;
        next.validate()?;

        Self::new(
            next.a * self.a + next.c * self.b,
            next.b * self.a + next.d * self.b,
            next.a * self.c + next.c * self.d,
            next.b * self.c + next.d * self.d,
            next.a * self.e + next.c * self.f + next.e,
            next.b * self.e + next.d * self.f + next.f,
        )
    }

    pub fn inverse(self) -> Result<Self, GeometryError> {
        self.validate()?;

        let determinant = self.a * self.d - self.b * self.c;
        if !determinant.is_finite() {
            return Err(GeometryError::NonFiniteTransform);
        }
        if determinant == 0.0 {
            return Err(GeometryError::SingularTransform);
        }

        let inv_det = 1.0 / determinant;
        let a = self.d * inv_det;
        let b = -self.b * inv_det;
        let c = -self.c * inv_det;
        let d = self.a * inv_det;
        let e = (self.c * self.f - self.d * self.e) * inv_det;
        let f = (self.b * self.e - self.a * self.f) * inv_det;

        Self::new(a, b, c, d, e, f)
    }

    pub fn apply_point(self, point: Point2) -> Result<Point2, GeometryError> {
        self.validate()?;
        point.validate()?;

        let transformed = Point2::new_unchecked(
            self.a.mul_add(point.x, self.c.mul_add(point.y, self.e)),
            self.b.mul_add(point.x, self.d.mul_add(point.y, self.f)),
        );
        if transformed.is_finite() {
            Ok(transformed)
        } else {
            Err(GeometryError::NonFiniteTransform)
        }
    }

    pub fn apply_cubic_bezier(self, curve: CubicBezier) -> Result<CubicBezier, GeometryError> {
        CubicBezier::new(
            self.apply_point(curve.p0)?,
            self.apply_point(curve.p1)?,
            self.apply_point(curve.p2)?,
            self.apply_point(curve.p3)?,
        )
    }

    /// The six affine coefficients in `(a, b, c, d, e, f)` order, matching the
    /// `apply_point` mapping `x' = a·x + c·y + e`, `y' = b·x + d·y + f`. This is
    /// exactly the row order consumed by tiny-skia `Transform::from_row` and the
    /// PDF `cm` operator, so a caller can push this transform onto either backend
    /// without re-deriving the matrix.
    #[must_use]
    pub const fn coefficients(self) -> (f64, f64, f64, f64, f64, f64) {
        (self.a, self.b, self.c, self.d, self.e, self.f)
    }

    /// Generate the dihedral (mirror) symmetry set for `count` axes about
    /// `center`, with the primary mirror axis at `axis_angle_degrees`.
    ///
    /// Returns `2 · count` transforms: for each rotational step `k` the plain
    /// rotation `r^k` and the reflected-then-rotated copy `s∘r^k`. `count == 1`
    /// yields `[identity, reflection]` (simple bilateral mirror); larger counts
    /// tile the plane into `2 · count` mirrored sectors (the MirrorMe / dihedral
    /// kaleidoscope). The order is fixed, so compositing is deterministic.
    pub fn dihedral_symmetry(
        count: usize,
        center: Point2,
        axis_angle_degrees: f64,
    ) -> Result<Vec<Self>, GeometryError> {
        if count == 0 {
            return Err(GeometryError::NonPositiveCount);
        }
        if count > MAX_RADIAL_SYMMETRY_COUNT {
            return Err(GeometryError::CountOutOfRange);
        }
        if !axis_angle_degrees.is_finite() {
            return Err(GeometryError::NonFiniteParameter);
        }
        center.validate()?;

        let axis_radians = axis_angle_degrees.to_radians();
        let axis_end = Point2::new(center.x + axis_radians.cos(), center.y + axis_radians.sin())?;
        let reflection = Self::reflection_across_line(center, axis_end)?;

        let step_degrees = 360.0 / count as f64;
        let mut transforms = Vec::with_capacity(count * 2);
        for index in 0..count {
            let rotation = Self::rotation(step_degrees * index as f64, center)?;
            transforms.push(rotation);
            transforms.push(reflection.compose(rotation)?);
        }

        Ok(transforms)
    }

    fn new(a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) -> Result<Self, GeometryError> {
        let transform = Self { a, b, c, d, e, f };
        transform.validate()?;
        Ok(transform)
    }

    fn validate(self) -> Result<(), GeometryError> {
        if self.a.is_finite()
            && self.b.is_finite()
            && self.c.is_finite()
            && self.d.is_finite()
            && self.e.is_finite()
            && self.f.is_finite()
        {
            Ok(())
        } else {
            Err(GeometryError::NonFiniteTransform)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1.0e-10;

    fn point(x: f64, y: f64) -> Point2 {
        Point2::new_unchecked(x, y)
    }

    fn curve() -> CubicBezier {
        CubicBezier::new_unchecked(
            point(0.0, 0.0),
            point(1.0, 2.0),
            point(3.0, 4.0),
            point(5.0, 6.0),
        )
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= EPSILON,
            "expected {actual} to be within {EPSILON} of {expected}"
        );
    }

    fn assert_point_close(actual: Point2, expected: Point2) {
        assert_close(actual.x, expected.x);
        assert_close(actual.y, expected.y);
    }

    #[test]
    fn identity_leaves_point_and_curve_unchanged() {
        let transform = AffineTransform::identity();
        let point = point(3.0, -7.0);
        let curve = curve();

        assert_eq!(transform.apply_point(point), Ok(point));
        assert_eq!(transform.apply_cubic_bezier(curve), Ok(curve));
    }

    #[test]
    fn translation_maps_point_and_inverse_round_trips() {
        let transform = AffineTransform::translation(5.0, -2.0).expect("valid transform");
        let inverse = transform.inverse().expect("invertible transform");
        let source = point(1.0, 4.0);
        let mapped = transform.apply_point(source).expect("valid output");

        assert_eq!(mapped, point(6.0, 2.0));
        assert_eq!(inverse.apply_point(mapped), Ok(source));
    }

    #[test]
    fn rotation_around_origin_maps_representative_point() {
        let transform = AffineTransform::rotation(90.0, point(0.0, 0.0)).expect("valid transform");

        assert_point_close(
            transform
                .apply_point(point(2.0, 0.0))
                .expect("valid output"),
            point(0.0, 2.0),
        );
    }

    #[test]
    fn rotation_around_pivot_maps_representative_point() {
        let transform = AffineTransform::rotation(90.0, point(1.0, 1.0)).expect("valid transform");

        assert_point_close(
            transform
                .apply_point(point(2.0, 1.0))
                .expect("valid output"),
            point(1.0, 2.0),
        );
    }

    #[test]
    fn reflection_across_vertical_line_is_involutive_and_maps_expected_point() {
        let transform = AffineTransform::reflection_across_line(point(2.0, -1.0), point(2.0, 5.0))
            .expect("valid transform");
        let source = point(5.0, 7.0);
        let mapped = transform.apply_point(source).expect("valid output");

        assert_point_close(mapped, point(-1.0, 7.0));
        assert_point_close(transform.apply_point(mapped).expect("valid output"), source);
    }

    #[test]
    fn reflection_across_horizontal_line_is_involutive_and_maps_expected_point() {
        let transform = AffineTransform::reflection_across_line(point(-1.0, 3.0), point(5.0, 3.0))
            .expect("valid transform");
        let source = point(5.0, 7.0);
        let mapped = transform.apply_point(source).expect("valid output");

        assert_point_close(mapped, point(5.0, -1.0));
        assert_point_close(transform.apply_point(mapped).expect("valid output"), source);
    }

    #[test]
    fn reflection_across_arbitrary_line_is_involutive_and_maps_expected_point() {
        let transform = AffineTransform::reflection_across_line(point(0.0, 0.0), point(1.0, 1.0))
            .expect("valid transform");
        let source = point(3.0, 1.0);
        let mapped = transform.apply_point(source).expect("valid output");

        assert_point_close(mapped, point(1.0, 3.0));
        assert_point_close(transform.apply_point(mapped).expect("valid output"), source);
    }

    #[test]
    fn composition_applies_self_then_next() {
        let translate = AffineTransform::translation(2.0, 0.0).expect("valid transform");
        let rotate = AffineTransform::rotation(90.0, point(0.0, 0.0)).expect("valid transform");
        let composed = translate.compose(rotate).expect("valid transform");

        assert_point_close(
            composed.apply_point(point(1.0, 0.0)).expect("valid output"),
            point(0.0, 3.0),
        );
    }

    #[test]
    fn inverse_round_trips_composed_rotation_and_translation() {
        let rotate = AffineTransform::rotation(45.0, point(2.0, -1.0)).expect("valid transform");
        let translate = AffineTransform::translation(-3.0, 9.0).expect("valid transform");
        let composed = rotate.compose(translate).expect("valid transform");
        let inverse = composed.inverse().expect("invertible transform");
        let source = point(12.0, -4.0);
        let mapped = composed.apply_point(source).expect("valid output");

        assert_point_close(inverse.apply_point(mapped).expect("valid output"), source);
    }

    #[test]
    fn radial_symmetry_uses_count_spacing_and_start_angle() {
        let transforms =
            AffineTransform::radial_symmetry(4, point(0.0, 0.0), 45.0).expect("valid transforms");
        let source = point(1.0, 0.0);

        assert_eq!(transforms.len(), 4);
        assert_point_close(
            transforms[0].apply_point(source).expect("valid output"),
            point(2.0_f64.sqrt() / 2.0, 2.0_f64.sqrt() / 2.0),
        );
        assert_point_close(
            transforms[1].apply_point(source).expect("valid output"),
            point(-2.0_f64.sqrt() / 2.0, 2.0_f64.sqrt() / 2.0),
        );
    }

    #[test]
    fn dihedral_symmetry_single_axis_is_identity_plus_reflection() {
        let transforms =
            AffineTransform::dihedral_symmetry(1, point(0.0, 0.0), 90.0).expect("valid transforms");
        assert_eq!(transforms.len(), 2);
        let source = point(3.0, 5.0);
        // r^0 = identity.
        assert_point_close(
            transforms[0].apply_point(source).expect("valid output"),
            source,
        );
        // Reflection across the vertical axis (angle 90°) flips x.
        assert_point_close(
            transforms[1].apply_point(source).expect("valid output"),
            point(-3.0, 5.0),
        );
    }

    #[test]
    fn dihedral_symmetry_yields_two_copies_per_axis_and_is_deterministic() {
        let center = point(0.0, 0.0);
        let a = AffineTransform::dihedral_symmetry(3, center, 0.0).expect("valid transforms");
        let b = AffineTransform::dihedral_symmetry(3, center, 0.0).expect("valid transforms");
        assert_eq!(a.len(), 6);
        assert_eq!(
            a, b,
            "same inputs must produce byte-identical transform order"
        );
        // Reflected copies have negative determinant; plain rotations positive.
        for (index, transform) in a.iter().enumerate() {
            let (ca, cb, cc, cd, _, _) = transform.coefficients();
            let determinant = ca * cd - cb * cc;
            if index % 2 == 0 {
                assert!(determinant > 0.0, "even index {index} should be a rotation");
            } else {
                assert!(
                    determinant < 0.0,
                    "odd index {index} should be a reflection"
                );
            }
        }
    }

    #[test]
    fn dihedral_symmetry_rejects_bad_count_and_angle() {
        assert_eq!(
            AffineTransform::dihedral_symmetry(0, point(0.0, 0.0), 0.0),
            Err(GeometryError::NonPositiveCount)
        );
        assert_eq!(
            AffineTransform::dihedral_symmetry(MAX_RADIAL_SYMMETRY_COUNT + 1, point(0.0, 0.0), 0.0),
            Err(GeometryError::CountOutOfRange)
        );
        assert_eq!(
            AffineTransform::dihedral_symmetry(2, point(0.0, 0.0), f64::NAN),
            Err(GeometryError::NonFiniteParameter)
        );
    }

    #[test]
    fn rejects_non_finite_translation_angle_and_points() {
        assert_eq!(
            AffineTransform::translation(f64::NAN, 0.0),
            Err(GeometryError::NonFiniteParameter)
        );
        assert_eq!(
            AffineTransform::rotation(f64::INFINITY, point(0.0, 0.0)),
            Err(GeometryError::NonFiniteParameter)
        );
        assert_eq!(
            AffineTransform::rotation(0.0, point(f64::NAN, 0.0)),
            Err(GeometryError::NonFinitePoint)
        );
        assert_eq!(
            AffineTransform::reflection_across_line(point(0.0, 0.0), point(f64::NAN, 0.0)),
            Err(GeometryError::NonFinitePoint)
        );
    }

    #[test]
    fn rejects_zero_radial_count_and_degenerate_reflection_line() {
        assert_eq!(
            AffineTransform::radial_symmetry(0, point(0.0, 0.0), 0.0),
            Err(GeometryError::NonPositiveCount)
        );
        assert_eq!(
            AffineTransform::radial_symmetry(MAX_RADIAL_SYMMETRY_COUNT + 1, point(0.0, 0.0), 0.0,),
            Err(GeometryError::CountOutOfRange)
        );
        assert_eq!(
            AffineTransform::reflection_across_line(point(1.0, 1.0), point(1.0, 1.0)),
            Err(GeometryError::DegenerateLine)
        );
    }

    #[test]
    fn reflection_handles_tiny_finite_distinct_line() {
        let transform =
            AffineTransform::reflection_across_line(point(0.0, 0.0), point(f64::MIN_POSITIVE, 0.0))
                .expect("tiny but distinct line should be valid");

        assert_point_close(
            transform
                .apply_point(point(2.0, 3.0))
                .expect("valid output"),
            point(2.0, -3.0),
        );
    }

    #[test]
    fn reflection_rejects_non_finite_delta() {
        assert_eq!(
            AffineTransform::reflection_across_line(point(-f64::MAX, 0.0), point(f64::MAX, 0.0)),
            Err(GeometryError::NonFiniteTransform)
        );
    }

    #[test]
    fn rejects_non_finite_radial_inputs() {
        assert_eq!(
            AffineTransform::radial_symmetry(1, point(0.0, 0.0), f64::NAN),
            Err(GeometryError::NonFiniteParameter)
        );
        assert_eq!(
            AffineTransform::radial_symmetry(1, point(0.0, f64::INFINITY), 0.0),
            Err(GeometryError::NonFinitePoint)
        );
    }

    #[test]
    fn applying_transform_that_creates_non_finite_output_returns_error() {
        let transform = AffineTransform {
            a: f64::MAX,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: f64::MAX,
            f: 0.0,
        };

        assert_eq!(
            transform.apply_point(point(2.0, 0.0)),
            Err(GeometryError::NonFiniteTransform)
        );
    }

    #[test]
    fn inverse_rejects_singular_transform() {
        let transform = AffineTransform {
            a: 1.0,
            b: 2.0,
            c: 2.0,
            d: 4.0,
            e: 0.0,
            f: 0.0,
        };

        assert_eq!(transform.inverse(), Err(GeometryError::SingularTransform));
    }

    #[test]
    fn inverse_rejects_non_finite_determinant() {
        let transform = AffineTransform {
            a: f64::MAX,
            b: 0.0,
            c: 0.0,
            d: f64::MAX,
            e: 0.0,
            f: 0.0,
        };

        assert_eq!(transform.inverse(), Err(GeometryError::NonFiniteTransform));
    }
}
