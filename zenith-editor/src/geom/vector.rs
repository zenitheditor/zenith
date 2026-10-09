//! Points, vectors, and the linear part of an affine map.

use zenith_scene::Affine2;

/// A point or vector in px.
pub(crate) type Pt = (f64, f64);

/// The linear part of `m` applied to vector `v` (no translation).
pub(crate) fn linear(m: Affine2, v: Pt) -> Pt {
    (m.a * v.0 + m.c * v.1, m.b * v.0 + m.d * v.1)
}

/// The vector whose image under `m`'s linear part is `v`: a page delta
/// taken back into `m`'s source space. `None` for a singular map.
pub(crate) fn unmap_vector(m: Affine2, v: Pt) -> Option<Pt> {
    m.inverse().map(|inv| linear(inv, v))
}

/// The linear part of `m` only.
pub(crate) fn linear_part(m: Affine2) -> Affine2 {
    Affine2 {
        e: 0.0,
        f: 0.0,
        ..m
    }
}

/// The determinant of `m`'s linear part. Negative for a mirror.
pub(crate) fn det(m: Affine2) -> f64 {
    m.a * m.d - m.b * m.c
}

/// The page angle in degrees of `m`'s x axis, clockwise on screen.
pub(crate) fn angle_deg(m: Affine2) -> f64 {
    m.b.atan2(m.a).to_degrees()
}

/// `a + b`.
pub(crate) fn add(a: Pt, b: Pt) -> Pt {
    (a.0 + b.0, a.1 + b.1)
}

/// `a - b`.
pub(crate) fn sub(a: Pt, b: Pt) -> Pt {
    (a.0 - b.0, a.1 - b.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unmap_undoes_the_linear_part() {
        let m = Affine2::rotate_at(30.0, 5.0, 7.0);
        let v = (3.0, -2.0);
        let back = unmap_vector(m, linear(m, v)).expect("invertible");
        assert!((back.0 - v.0).abs() < 1e-12 && (back.1 - v.1).abs() < 1e-12);
        assert!((angle_deg(m) - 30.0).abs() < 1e-12);
        assert!((det(m) - 1.0).abs() < 1e-12);
        assert_eq!(linear_part(Affine2::translate(4.0, 4.0)), Affine2::IDENTITY);
    }

    #[test]
    fn add_and_sub_are_inverse() {
        assert_eq!(add((1.0, 2.0), sub((5.0, 5.0), (1.0, 1.0))), (5.0, 6.0));
    }
}
