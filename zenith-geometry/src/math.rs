//! Transcendental functions with the same bits on every target.
//!
//! `f64::sin` and friends call the platform math library: glibc on Linux,
//! the C runtime on Windows, and musl-derived code on wasm. They agree to
//! about one ULP, not bit for bit, so the same edit could write different
//! numbers natively and in the browser. Every number Zenith derives with
//! these functions and writes to a source, a reply, or a render uses this
//! module instead. It wraps the pure-Rust `libm` crate, which compiles to
//! the same operations on every target.

/// Sine of `x` radians.
#[must_use]
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// Cosine of `x` radians.
#[must_use]
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}

/// `(sin(x), cos(x))` of `x` radians.
#[must_use]
pub fn sin_cos(x: f64) -> (f64, f64) {
    libm::sincos(x)
}

/// Tangent of `x` radians.
#[must_use]
pub fn tan(x: f64) -> f64 {
    libm::tan(x)
}

/// The angle in radians of the vector `(x, y)`: `atan2(y, x)`.
#[must_use]
pub fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

/// Arctangent of `x`, in radians.
#[must_use]
pub fn atan(x: f64) -> f64 {
    libm::atan(x)
}

/// Arcsine of `x`, in radians.
#[must_use]
pub fn asin(x: f64) -> f64 {
    libm::asin(x)
}

/// Arccosine of `x`, in radians.
#[must_use]
pub fn acos(x: f64) -> f64 {
    libm::acos(x)
}

/// `sqrt(x² + y²)` without undue overflow.
#[must_use]
pub fn hypot(x: f64, y: f64) -> f64 {
    libm::hypot(x, y)
}

/// `x` raised to the power `y`.
#[must_use]
pub fn powf(x: f64, y: f64) -> f64 {
    libm::pow(x, y)
}

/// `e` raised to the power `x`.
#[must_use]
pub fn exp(x: f64) -> f64 {
    libm::exp(x)
}

/// Natural logarithm of `x`.
#[must_use]
pub fn ln(x: f64) -> f64 {
    libm::log(x)
}

/// Base-10 logarithm of `x`.
#[must_use]
pub fn log10(x: f64) -> f64 {
    libm::log10(x)
}

/// Base-2 logarithm of `x`.
#[must_use]
pub fn log2(x: f64) -> f64 {
    libm::log2(x)
}

/// Cube root of `x`.
#[must_use]
pub fn cbrt(x: f64) -> f64 {
    libm::cbrt(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_values_hold() {
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
        assert_eq!(sin_cos(0.0), (0.0, 1.0));
        assert_eq!(atan2(0.0, 1.0), 0.0);
        assert_eq!(hypot(3.0, 4.0), 5.0);
        assert_eq!(powf(2.0, 10.0), 1024.0);
        assert_eq!(cbrt(27.0), 3.0);
        assert_eq!(log2(8.0), 3.0);
        assert_eq!(log10(1000.0), 3.0);
        assert!((sin(std::f64::consts::FRAC_PI_2) - 1.0).abs() < 1e-15);
    }

    /// Bits the `libm` algorithms give for a few angles where platform
    /// libraries are known to round differently: a target that compiled
    /// these functions differently would fail here.
    #[test]
    fn bits_are_the_libm_bits() {
        for deg in [-179.2722_f64, 17.3, 33.0, 123.456, 271.9] {
            let r = deg.to_radians();
            assert_eq!(sin(r).to_bits(), libm::sin(r).to_bits());
            assert_eq!(cos(r).to_bits(), libm::cos(r).to_bits());
            let (s, c) = sin_cos(r);
            assert_eq!(
                (s.to_bits(), c.to_bits()),
                (sin(r).to_bits(), cos(r).to_bits())
            );
        }
    }
}
