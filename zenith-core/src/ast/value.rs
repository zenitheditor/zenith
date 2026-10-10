//! Dimension, unit, and property-value types.

/// A unit of measurement used in `.zen` documents.
#[derive(Debug, Clone, PartialEq)]
pub enum Unit {
    /// Document pixel units — `(px)`.
    Px,
    /// Point units — `(pt)`.
    Pt,
    /// Percentage — `(pct)`.
    Pct,
    /// Degrees — `(deg)`.
    Deg,
    /// An unrecognized unit annotation (forward-compat).
    Unknown(String),
}

impl Unit {
    /// Parse a unit annotation string (without the enclosing parentheses).
    pub fn from_annotation(s: &str) -> Self {
        match s {
            "px" => Self::Px,
            "pt" => Self::Pt,
            "pct" => Self::Pct,
            "deg" => Self::Deg,
            other => Self::Unknown(other.to_owned()),
        }
    }

    /// The canonical annotation string (without parentheses) — the inverse of
    /// [`Unit::from_annotation`].
    pub fn as_annotation(&self) -> &str {
        match self {
            Self::Px => "px",
            Self::Pt => "pt",
            Self::Pct => "pct",
            Self::Deg => "deg",
            Self::Unknown(s) => s.as_str(),
        }
    }
}

/// A value that carries a numeric magnitude and a measurement unit.
#[derive(Debug, Clone, PartialEq)]
pub struct Dimension {
    /// The numeric magnitude.
    pub value: f64,
    /// The unit of the magnitude.
    pub unit: Unit,
}

impl Dimension {
    /// Format as canonical KDL value syntax, e.g. `(px)640` or `(pt)10.5`.
    ///
    /// An integral, finite magnitude renders without a fractional part. This is
    /// the single source of the dimension string used by the formatter and by
    /// the CLI's transaction/inspect output, so all three agree byte-for-byte.
    pub fn to_kdl_string(&self) -> String {
        format!(
            "({}){}",
            self.unit.as_annotation(),
            format_number(self.value)
        )
    }
}

/// Format `v` as a KDL number, exactly.
///
/// An integral value has no fractional part (`640`, `-0` as `0`), at any
/// magnitude: `1e19` writes all 20 digits, never a saturated `i64`. A
/// fraction takes the shortest text that reads back as `v`. A non-finite
/// value takes its KDL keyword: `#nan`, `#inf`, `#-inf`.
pub fn format_number(v: f64) -> String {
    /// 2^63: every integral `f64` in `[-2^63, 2^63)` is an exact `i64`.
    const I64_END: f64 = 9_223_372_036_854_775_808.0;
    if v.is_nan() {
        return "#nan".to_owned();
    }
    if v.is_infinite() {
        return if v > 0.0 { "#inf" } else { "#-inf" }.to_owned();
    }
    if v.fract() == 0.0 && (-I64_END..I64_END).contains(&v) {
        // In range, the cast is exact; it also writes -0 as 0.
        return format!("{}", v as i64);
    }
    format!("{v}")
}

/// Convert a dimension value + unit to pixels.
///
/// Returns `Some(px)` for `Px` (identity) and `Pt` (×96/72).
/// Returns `None` for `Pct`, `Deg`, and `Unknown` — the caller decides
/// whether to resolve against an axis basis or emit an advisory.
///
/// This is the canonical conversion used by both the scene compiler and the
/// validator; keeping it here ensures both agree on the arithmetic.
pub fn dim_to_px(value: f64, unit: &Unit) -> Option<f64> {
    match unit {
        Unit::Px => Some(value),
        Unit::Pt => Some(value * 96.0 / 72.0),
        Unit::Pct | Unit::Deg | Unit::Unknown(_) => None,
    }
}

/// A property value that is either a token reference or a raw literal string.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    /// A reference to a design token, e.g. `(token)"color.text.primary"`.
    TokenRef(String),
    /// A raw literal value stored as a string (e.g. a hex color `"#ff0000"`).
    Literal(String),
    /// A literal dimension with an explicit unit, e.g. `(px)24` or `(pt)13`.
    Dimension(Dimension),
    /// A typed reference to a runtime data field, e.g. `(data)"revenue.total"`.
    DataRef(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_format_exactly_at_any_magnitude() {
        for (v, text) in [
            (640.0, "640"),
            (10.5, "10.5"),
            (-0.0, "0"),
            (-3.0, "-3"),
            (1e19, "10000000000000000000"),
            (-1e19, "-10000000000000000000"),
            (9_007_199_254_740_992.0, "9007199254740992"),
            (f64::NAN, "#nan"),
            (f64::INFINITY, "#inf"),
            (f64::NEG_INFINITY, "#-inf"),
        ] {
            assert_eq!(format_number(v), text, "{v}");
        }
        let d = Dimension {
            value: 1e19,
            unit: Unit::Px,
        };
        assert_eq!(d.to_kdl_string(), "(px)10000000000000000000");
    }
}
