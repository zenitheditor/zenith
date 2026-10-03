//! Style block and style definition types.

use std::collections::BTreeMap;

use crate::ast::span::Span;
use crate::ast::value::PropertyValue;

/// An unknown property child encountered inside a `style` block.
///
/// The name was not in the recognized set of visual style keys, so it cannot
/// be applied as a visual cascade.  It is preserved here so that the
/// validator can emit `style.unknown_property` warnings.
#[derive(Debug, Clone, PartialEq)]
pub struct UnknownStyleProp {
    /// The raw property value (first positional argument of the child node).
    pub raw: String,
}

/// A named style definition, holding a map of recognized visual properties.
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    /// Globally unique style ID.
    pub id: String,
    /// Recognized visual properties, keyed by their canonical hyphenated name.
    ///
    /// Canonical keys: `fill`, `stroke`, `stroke-width`, `stroke-alignment`,
    /// `font-family`, `font-size`, `font-weight`, `letter-spacing`,
    /// `line-height`, `radius`,
    /// `padding`, `gap`, `align`, `v-align`, `shadow`.
    ///
    /// `align` and `v-align` hold a [`PropertyValue::Literal`] enum value (see
    /// [`STYLE_ENUM_KEYS`]); every other key holds a token reference.
    pub properties: BTreeMap<String, PropertyValue>,
    /// Unknown (unrecognized) child node names encountered in the style block.
    ///
    /// These are preserved so the validator can emit `style.unknown_property`
    /// warnings without re-parsing the source.
    pub unknown_props: BTreeMap<String, UnknownStyleProp>,
    /// Byte-range of this `style` node in the source (for diagnostics).
    pub source_span: Option<Span>,
}

/// The top-level `styles` block containing named style definitions.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StyleBlock {
    /// Ordered list of style definitions.
    pub styles: Vec<Style>,
    /// Byte-range of the `styles` block in the source (for diagnostics).
    pub source_span: Option<Span>,
}

/// Canonical hyphenated keys for recognized style visual properties.
///
/// Underscore variants are normalized to these forms by
/// [`canonicalize_style_key`] during parsing and transaction application.
pub const STYLE_RECOGNIZED_KEYS: &[&str] = &[
    "fill",
    "stroke",
    "stroke-width",
    "stroke-alignment",
    "font-family",
    "font-size",
    "font-weight",
    "letter-spacing",
    "line-height",
    "radius",
    "padding",
    "gap",
    "align",
    "v-align",
    "shadow",
];

/// Style keys whose value is an enum literal (`align "center"`), stored as
/// [`PropertyValue::Literal`]. Every other key takes a token reference.
///
/// Each key maps to its allowed values in [`crate::schema::enums`].
pub const STYLE_ENUM_KEYS: &[(&str, &[&str])] = &[
    ("align", crate::schema::enums::TEXT_ALIGNS),
    ("v-align", crate::schema::enums::V_ALIGNS),
];

/// The allowed values of enum-valued style key `key`, or `None` when the key
/// takes a token reference.
pub fn style_enum_values(key: &str) -> Option<&'static [&'static str]> {
    STYLE_ENUM_KEYS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, values)| *values)
}

/// Map underscore-spelled style child names to their canonical hyphenated form.
///
/// Normalizes underscore variants (`stroke_width` → `stroke-width`, etc.) and
/// returns the canonical key if it is in [`STYLE_RECOGNIZED_KEYS`], or `None`
/// if the name is unrecognized after normalization.
pub fn canonicalize_style_key(name: &str) -> Option<&'static str> {
    // Normalize underscore to hyphen for comparison.
    let normalized: &str = match name {
        "stroke_width" => "stroke-width",
        "stroke_alignment" => "stroke-alignment",
        "font_family" => "font-family",
        "font_size" => "font-size",
        "font_weight" => "font-weight",
        "letter_spacing" => "letter-spacing",
        "tracking" => "letter-spacing",
        "line_height" => "line-height",
        "v_align" => "v-align",
        other => other,
    };
    STYLE_RECOGNIZED_KEYS
        .iter()
        .copied()
        .find(|&k| k == normalized)
}
