//! Low-level KDL formatting primitives shared by the document writer.
//!
//! Property writers, number/string formatters, and escape helpers. The public
//! entry point and document-structure orchestration live in the parent module.

use crate::ast::{Dimension, ObjectPosition, PropertyValue, UnknownProperty, UnknownValue};

// ---------------------------------------------------------------------------
// Unknown property value formatting
// ---------------------------------------------------------------------------

/// Produce a KDL-valid serialization for an `UnknownValue`, preserving the
/// original KDL type so that parse→format→parse is a perfect round-trip:
///
/// - `String(s)` → a double-quoted, escaped KDL string (`"hello"`)
/// - `Integer(n)` → a bare decimal integer (`42`)
/// - `Float(f)` → a bare number via the canonical f64 formatter (integral
///   floats emit without `.0`: `1` not `1.0`)
/// - `Bool(b)` → KDL v2 boolean keyword (`#true` / `#false`)
/// - `Null` → KDL v2 null keyword (`#null`)
fn fmt_unknown_value(v: &UnknownValue) -> String {
    match v {
        UnknownValue::String(s) => {
            let mut out = String::with_capacity(s.len() + 2);
            out.push('"');
            out.push_str(&escape_kdl_string(s));
            out.push('"');
            out
        }
        UnknownValue::Integer(n) => n.to_string(),
        UnknownValue::Float(f) => fmt_f64(*f),
        UnknownValue::Bool(b) => (if *b { "#true" } else { "#false" }).to_owned(),
        UnknownValue::Null => "#null".to_owned(),
    }
}

/// Serialize an [`UnknownProperty`]'s value, including its KDL type annotation
/// when present, so that an annotated value round-trips byte-identically.
///
/// The annotation is emitted as a `(ty)` prefix in the value position, matching
/// KDL v2 syntax `name=(type)value`:
///
/// - annotated → `(px)10`, `(token)"color.navy"`
/// - unannotated → identical to [`fmt_unknown_value`]
pub(in crate::format::writer) fn fmt_unknown_property(p: &UnknownProperty) -> String {
    match &p.ty {
        Some(ty) => format!("({}){}", ty, fmt_unknown_value(&p.value)),
        None => fmt_unknown_value(&p.value),
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Append `count * 2` spaces of indentation.
pub(in crate::format::writer) fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth * 2 {
        out.push(' ');
    }
}

/// Format a `f64` canonically: no trailing `.0` for integral values.
pub(in crate::format::writer) fn fmt_f64(v: f64) -> String {
    if v.fract() == 0.0 && v.is_finite() {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Format a dimension annotation + value, e.g. `(px)640` or `(pt)10.5`.
pub(in crate::format::writer) fn fmt_dimension(d: &Dimension) -> String {
    d.to_kdl_string()
}

/// Format a `PropertyValue` as a KDL value.
///
/// - `TokenRef("color.bg")`  →  `(token)"color.bg"`
/// - `Literal("center")`     →  `"center"`
/// - `Dimension((px)24)`     →  `(px)24`
pub(in crate::format::writer) fn fmt_property_value(pv: &PropertyValue) -> String {
    match pv {
        PropertyValue::TokenRef(id) => format!("(token)\"{id}\""),
        PropertyValue::Literal(s) => format!("\"{s}\""),
        PropertyValue::Dimension(d) => fmt_dimension(d),
        PropertyValue::DataRef(path) => format!("(data)\"{path}\""),
    }
}

/// Emit `key=value` for a `PropertyValue` property (if present).
pub(in crate::format::writer) fn write_opt_property_value(
    out: &mut String,
    key: &str,
    opt: &Option<PropertyValue>,
) {
    if let Some(pv) = opt {
        out.push(' ');
        out.push_str(key);
        out.push('=');
        out.push_str(&fmt_property_value(pv));
    }
}

/// Emit `key=(unit)N` for an optional `Dimension`.
pub(in crate::format::writer) fn write_opt_dimension(
    out: &mut String,
    key: &str,
    opt: &Option<Dimension>,
) {
    if let Some(d) = opt {
        out.push(' ');
        out.push_str(key);
        out.push('=');
        out.push_str(&fmt_dimension(d));
    }
}

/// Emit `key="string"` for an optional string (quoted, no escaping).
pub(in crate::format::writer) fn write_opt_str(out: &mut String, key: &str, opt: &Option<String>) {
    if let Some(s) = opt {
        out.push(' ');
        out.push_str(key);
        out.push_str("=\"");
        out.push_str(s);
        out.push('"');
    }
}

/// Emit `key="string"` for an optional string, running the value through
/// [`escape_kdl_string`] so that backslashes, quotes, and whitespace control
/// characters survive as a single-line KDL string.
pub(in crate::format::writer) fn write_opt_str_escaped(
    out: &mut String,
    key: &str,
    opt: &Option<String>,
) {
    if let Some(s) = opt {
        out.push(' ');
        out.push_str(key);
        out.push_str("=\"");
        out.push_str(&escape_kdl_string(s));
        out.push('"');
    }
}

/// Emit `key=#true` or `key=#false` for an optional bool.
pub(in crate::format::writer) fn write_opt_bool(out: &mut String, key: &str, opt: &Option<bool>) {
    if let Some(b) = opt {
        out.push(' ');
        out.push_str(key);
        out.push('=');
        out.push_str(if *b { "#true" } else { "#false" });
    }
}

/// Emit `key="anchor"` (string) or `key=(pct)N` (annotated number) for an
/// optional [`ObjectPosition`].
pub(in crate::format::writer) fn write_opt_object_position(
    out: &mut String,
    key: &str,
    opt: &Option<ObjectPosition>,
) {
    if let Some(pos) = opt {
        out.push(' ');
        out.push_str(key);
        out.push('=');
        match pos {
            ObjectPosition::Start => out.push_str("\"start\""),
            ObjectPosition::Center => out.push_str("\"center\""),
            ObjectPosition::End => out.push_str("\"end\""),
            ObjectPosition::Pct(n) => {
                out.push_str("(pct)");
                out.push_str(&fmt_f64(*n));
            }
        }
    }
}

/// Emit `key=N` for an optional `f64` (bare number, no unit).
pub(in crate::format::writer) fn write_opt_f64(out: &mut String, key: &str, opt: &Option<f64>) {
    if let Some(v) = opt {
        out.push(' ');
        out.push_str(key);
        out.push('=');
        out.push_str(&fmt_f64(*v));
    }
}

/// Escape a string for emission as a single-line KDL v2 quoted string.
///
/// Unlike the inline span/unknown-prop escapers (which only handle `\` and `"`),
/// this also encodes the whitespace control characters `\n`, `\r`, and `\t` as
/// backslash escapes so that a multi-line `code` blob survives as ONE physical
/// line. All other characters pass through verbatim. This is the inverse of the
/// `kdl` crate's decode on parse, guaranteeing a byte-exact content round-trip.
pub(in crate::format::writer) fn escape_kdl_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out
}
