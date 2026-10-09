//! [`Affine2`]: the 2-D affine map the box records and hit tests use.

/// A 2-D affine map: `x' = a·x + c·y + e`, `y' = b·x + d·y + f`.
///
/// The convention matches `SceneCommand::PushTransformMatrix`. Compose with
/// [`Affine2::then`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine2 {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Default for Affine2 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Affine2 {
    /// The map that moves no point.
    pub const IDENTITY: Affine2 = Affine2 {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    /// Translation by `(tx, ty)`.
    #[must_use]
    pub fn translate(tx: f64, ty: f64) -> Affine2 {
        Affine2 {
            e: tx,
            f: ty,
            ..Affine2::IDENTITY
        }
    }

    /// Rotation by `deg` about `(cx, cy)`, as `SceneCommand::PushTransform`
    /// draws it. Positive degrees turn clockwise on screen (y down).
    #[must_use]
    pub fn rotate_at(deg: f64, cx: f64, cy: f64) -> Affine2 {
        let (sin, cos) = deg.to_radians().sin_cos();
        Affine2 {
            a: cos,
            b: sin,
            c: -sin,
            d: cos,
            e: cx - cos * cx + sin * cy,
            f: cy - sin * cx - cos * cy,
        }
    }

    /// `self ∘ l`: `l` maps a point first, then `self` (the backends'
    /// `pre_concat`).
    #[must_use]
    pub fn then(self, l: Affine2) -> Affine2 {
        Affine2 {
            a: self.a * l.a + self.c * l.b,
            b: self.b * l.a + self.d * l.b,
            c: self.a * l.c + self.c * l.d,
            d: self.b * l.c + self.d * l.d,
            e: self.a * l.e + self.c * l.f + self.e,
            f: self.b * l.e + self.d * l.f + self.f,
        }
    }

    /// The image of `(x, y)`.
    #[must_use]
    pub fn apply(self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    /// The inverse map. `None` when the map is singular (a zero scale) or
    /// not finite.
    #[must_use]
    pub fn inverse(self) -> Option<Affine2> {
        let det = self.a * self.d - self.b * self.c;
        if !det.is_finite() || det == 0.0 {
            return None;
        }
        let a = self.d / det;
        let b = -self.b / det;
        let c = -self.c / det;
        let d = self.a / det;
        Some(Affine2 {
            a,
            b,
            c,
            d,
            e: -(a * self.e + c * self.f),
            f: -(b * self.e + d * self.f),
        })
    }

    /// The coefficients `[a, b, c, d, e, f]`.
    #[must_use]
    pub fn coefficients(self) -> [f64; 6] {
        [self.a, self.b, self.c, self.d, self.e, self.f]
    }

    /// `true` when the map keeps axis-aligned boxes axis-aligned: no
    /// rotation off a quarter turn, no shear.
    #[must_use]
    pub fn is_axis_aligned(self) -> bool {
        const EPS: f64 = 1e-9;
        (self.b.abs() < EPS && self.c.abs() < EPS) || (self.a.abs() < EPS && self.d.abs() < EPS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(p: (f64, f64), q: (f64, f64)) -> bool {
        (p.0 - q.0).abs() < 1e-9 && (p.1 - q.1).abs() < 1e-9
    }

    #[test]
    fn rotation_about_a_center_keeps_the_center() {
        let m = Affine2::rotate_at(90.0, 10.0, 20.0);
        assert!(close(m.apply(10.0, 20.0), (10.0, 20.0)));
        // y-down: +90° turns +x into +y.
        assert!(close(m.apply(11.0, 20.0), (10.0, 21.0)));
    }

    #[test]
    fn then_applies_local_first() {
        let scale = Affine2 {
            a: 2.0,
            d: 2.0,
            ..Affine2::IDENTITY
        };
        let shift = Affine2::translate(5.0, 0.0);
        assert_eq!(shift.then(scale).apply(1.0, 1.0), (7.0, 2.0));
    }

    #[test]
    fn inverse_undoes_the_map() {
        let m = Affine2::translate(3.0, -4.0)
            .then(Affine2::rotate_at(33.0, 7.0, 9.0))
            .then(Affine2 {
                a: 2.0,
                d: 0.5,
                ..Affine2::IDENTITY
            });
        let inv = m.inverse().expect("invertible");
        let (x, y) = m.apply(12.5, -3.0);
        assert!(close(inv.apply(x, y), (12.5, -3.0)));
        let zero = Affine2 {
            a: 0.0,
            ..Affine2::IDENTITY
        };
        assert_eq!(zero.inverse(), None);
    }
}
