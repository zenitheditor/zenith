//! Small pure helpers shared across the scene-compilation submodules:
//! rotation-angle extraction, unsupported-unit diagnostics, and dimension
//! property resolution.

use std::collections::BTreeMap;

use zenith_core::{
    Diagnostic, Dimension, PropertyValue, ResolvedToken, ResolvedValue, Span, Unit, dim_to_px,
};

use crate::ir::BlendMode;

// ── Rotation helper ───────────────────────────────────────────────────────────

/// If `rotate` is a non-zero angle, returns the degrees to rotate the node's
/// commands around its center; else None. (deg unit; value read directly.)
pub(super) fn rotation_degrees(rotate: Option<&Dimension>) -> Option<f64> {
    rotate.map(|d| d.value).filter(|a| *a != 0.0)
}

// ── Blend-mode helper ───────────────────────────────────────────────────────

/// Map a `blend-mode` attribute string to a non-`Normal` [`BlendMode`], or
/// `None` when no compositing layer is needed.
///
/// `None`, `"normal"`, and any unrecognized value all return `None` — those
/// nodes compile to a plain (layer-free) command stream, byte-identical to
/// before blend-mode existed. Only non-normal recognized blends open a layer.
pub(super) fn blend_mode_ir(s: Option<&str>) -> Option<BlendMode> {
    match s {
        Some("multiply") => Some(BlendMode::Multiply),
        Some("screen") => Some(BlendMode::Screen),
        Some("overlay") => Some(BlendMode::Overlay),
        Some("darken") => Some(BlendMode::Darken),
        Some("lighten") => Some(BlendMode::Lighten),
        Some("color-dodge") => Some(BlendMode::ColorDodge),
        Some("color-burn") => Some(BlendMode::ColorBurn),
        Some("hard-light") => Some(BlendMode::HardLight),
        Some("soft-light") => Some(BlendMode::SoftLight),
        Some("difference") => Some(BlendMode::Difference),
        Some("exclusion") => Some(BlendMode::Exclusion),
        Some("hue") => Some(BlendMode::Hue),
        Some("saturation") => Some(BlendMode::Saturation),
        Some("color") => Some(BlendMode::Color),
        Some("luminosity") => Some(BlendMode::Luminosity),
        // "normal", None, and unrecognized values: no layer.
        _ => None,
    }
}

/// Build a `scene.unsupported_unit` advisory for a named geometry field.
///
/// `kind` is the human-readable node kind (e.g. `"rect"`, `"ellipse"`,
/// `"line"`, `"text node"`) used in the diagnostic message.
pub(super) fn unsupported_unit_diag(
    kind: &str,
    node_id: &str,
    field: &str,
    span: Option<Span>,
) -> Diagnostic {
    Diagnostic::advisory(
        "scene.unsupported_unit",
        format!(
            "{} '{}' field '{}' uses an unsupported unit; the {} is skipped",
            kind, node_id, field, kind
        ),
        span,
        Some(node_id.to_owned()),
    )
}

/// Build a `scene.missing_geometry` advisory for a node missing one or more of
/// its `x`/`y`/`w`/`h` geometry properties.
pub(super) fn missing_geometry_diag(kind: &str, node_id: &str, span: Option<Span>) -> Diagnostic {
    Diagnostic::advisory(
        "scene.missing_geometry",
        format!(
            "{kind} '{node_id}' is missing one or more geometry properties (x, y, w, h); \
             skipped"
        ),
        span,
        Some(node_id.to_owned()),
    )
}

/// Identity fields for a geometry axis resolution: the node kind, its id, and
/// the axis name (`"x"` or `"y"`). Bundled to keep [`resolve_anchored_axis`]
/// under the 7-argument Clippy limit.
#[derive(Clone, Copy)]
pub(super) struct AxisTarget<'a> {
    pub(super) kind: &'a str,
    pub(super) node_id: &'a str,
    pub(super) axis: &'a str,
}

/// Resolve a single position axis (`x` or `y`) to pixels, honoring a
/// page-relative anchor fallback.
///
/// - `dim = Some` (an explicitly-authored value): a raw dimension or a
///   dimension token ref resolves to px via [`resolve_geometry_px`]; when it
///   cannot resolve (unsupported unit, or a token that isn't a dimension /
///   doesn't resolve), pushes `scene.unsupported_unit` and returns `None`.
/// - `dim = None`: uses `anchor_val` when present (anchor-derived); otherwise
///   pushes `scene.missing_geometry` and returns `None`.
///
/// A `None` return always means a diagnostic was pushed and the caller must
/// skip the node. An explicit value always wins over the anchor. The raw-`px`
/// path is byte-identical to the prior `Dimension`-only behavior.
pub(super) fn resolve_anchored_axis(
    target: AxisTarget<'_>,
    dim: Option<&PropertyValue>,
    resolved: &BTreeMap<String, ResolvedToken>,
    anchor_val: Option<f64>,
    span: Option<Span>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<f64> {
    match dim {
        Some(prop) => match resolve_geometry_px(Some(prop), resolved) {
            Some(v) => Some(v),
            None => {
                diagnostics.push(unsupported_unit_diag(
                    target.kind,
                    target.node_id,
                    target.axis,
                    span,
                ));
                None
            }
        },
        None => match anchor_val {
            Some(v) => Some(v),
            None => {
                diagnostics.push(missing_geometry_diag(target.kind, target.node_id, span));
                None
            }
        },
    }
}

/// Build a `(px)`-unit [`Dimension`] from a raw pixel value.
///
/// Shared by `field` and `footnote` to synthesize geometry for their
/// constructed [`zenith_core::TextNode`]s.
pub(super) fn px(v: f64) -> Dimension {
    Dimension {
        value: v,
        unit: Unit::Px,
    }
}

/// Build a `(px)`-unit geometry [`PropertyValue`] from a raw pixel value.
///
/// Used to synthesize the `x`/`y`/`w`/`h` of constructed nodes (footnote text,
/// shape labels, connector labels, TOC rows) now that geometry fields are typed
/// `Option<PropertyValue>`. The produced value is `PropertyValue::Dimension`, so
/// it resolves through [`resolve_geometry_px`] to exactly `v` — byte-identical to
/// the prior raw-`Dimension` synthesis.
pub(super) fn px_prop(v: f64) -> PropertyValue {
    PropertyValue::Dimension(px(v))
}

/// Resolve an optional dimension-valued property to pixels.
///
/// Returns `default` when the property is absent, is a raw literal, references
/// a non-dimension (or unresolved) token, or carries an unsupported unit. The
/// idiomatic path is a token ref resolving to a `Dimension`. Shared by
/// font-size and stroke-width resolution.
pub(super) fn resolve_property_dimension_px(
    prop: Option<&PropertyValue>,
    resolved: &BTreeMap<String, ResolvedToken>,
    default: f64,
) -> f64 {
    match prop {
        Some(PropertyValue::TokenRef(token_id)) => match resolved.get(token_id.as_str()) {
            Some(rt) => match &rt.value {
                ResolvedValue::Dimension(dim) => dim_to_px(dim.value, &dim.unit).unwrap_or(default),
                ResolvedValue::Color(_)
                | ResolvedValue::CmykColor { .. }
                | ResolvedValue::Number(_)
                | ResolvedValue::FontFamily(_)
                | ResolvedValue::FontWeight(_)
                | ResolvedValue::Gradient(_)
                | ResolvedValue::Shadow(_)
                | ResolvedValue::Filter(_)
                | ResolvedValue::Mask(_) => default,
            },
            None => default,
        },
        // A literal dimension (e.g. `font-size=(px)24`) resolves directly,
        // bringing literal visual dimensions to parity with token-backed ones.
        Some(PropertyValue::Dimension(dim)) => dim_to_px(dim.value, &dim.unit).unwrap_or(default),
        Some(PropertyValue::Literal(_)) | Some(PropertyValue::DataRef(_)) | None => default,
    }
}

/// Resolve an optional geometry property (`x`/`y`/`w`/`h`) to pixels.
///
/// Geometry fields on box nodes are typed `Option<PropertyValue>`: a raw
/// dimension (`(px)120`) resolves directly, and a dimension token ref
/// (`(token)"dim.h"`) resolves through the token table to px. Any other shape —
/// an absent value, a literal, a data ref, an unresolved/non-dimension token, or
/// an unsupported unit — yields `None`, matching the prior "missing or non-px
/// dimension" behavior exactly. The raw-`Dimension` path is byte-identical to
/// the old `dim_to_px(d.value, &d.unit)` read.
pub(super) fn resolve_geometry_px(
    prop: Option<&PropertyValue>,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Option<f64> {
    match prop? {
        PropertyValue::TokenRef(id) => match resolved.get(id.as_str()) {
            Some(rt) => match &rt.value {
                ResolvedValue::Dimension(d) => dim_to_px(d.value, &d.unit),
                ResolvedValue::Color(_)
                | ResolvedValue::CmykColor { .. }
                | ResolvedValue::Number(_)
                | ResolvedValue::FontFamily(_)
                | ResolvedValue::FontWeight(_)
                | ResolvedValue::Gradient(_)
                | ResolvedValue::Shadow(_)
                | ResolvedValue::Filter(_)
                | ResolvedValue::Mask(_) => None,
            },
            None => None,
        },
        PropertyValue::Dimension(d) => dim_to_px(d.value, &d.unit),
        PropertyValue::Literal(_) | PropertyValue::DataRef(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_mode_ir_maps_nonseparable_modes() {
        assert_eq!(blend_mode_ir(Some("hue")), Some(BlendMode::Hue));
        assert_eq!(
            blend_mode_ir(Some("saturation")),
            Some(BlendMode::Saturation)
        );
        assert_eq!(blend_mode_ir(Some("color")), Some(BlendMode::Color));
        assert_eq!(
            blend_mode_ir(Some("luminosity")),
            Some(BlendMode::Luminosity)
        );
    }

    #[test]
    fn blend_mode_ir_keeps_normal_and_unknown_layer_free() {
        assert_eq!(blend_mode_ir(None), None);
        assert_eq!(blend_mode_ir(Some("normal")), None);
        assert_eq!(blend_mode_ir(Some("unknown")), None);
    }
}
