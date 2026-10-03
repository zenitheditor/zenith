//! Small pure helpers shared across the scene-compilation submodules:
//! point-list bounds, rotation-angle extraction, unsupported-unit diagnostics,
//! and dimension property resolution.

use std::collections::BTreeMap;

use zenith_core::{
    Diagnostic, Dimension, Point, PropertyValue, ResolvedToken, ResolvedValue, Span, Unit,
    dim_to_px,
};

use crate::ir::BlendMode;

// ── Point-list bounds ─────────────────────────────────────────────────────────

/// Axis-aligned bounding box `(x_min, y_min, w, h)` of a `Point` list in
/// authored (local) coordinates.
///
/// Skips points whose x/y is missing or uses an unsupported unit. Returns
/// `None` when no finite point remains. Shared by group child-union bounds and
/// connector free-form outline boxes so both walks use the same filter.
pub(super) fn points_bbox(pts: &[Point]) -> Option<(f64, f64, f64, f64)> {
    let mut px_min = f64::INFINITY;
    let mut py_min = f64::INFINITY;
    let mut px_max = f64::NEG_INFINITY;
    let mut py_max = f64::NEG_INFINITY;
    for pt in pts {
        let (Some(xd), Some(yd)) = (&pt.x, &pt.y) else {
            continue;
        };
        let (Some(px), Some(py)) = (dim_to_px(xd.value, &xd.unit), dim_to_px(yd.value, &yd.unit))
        else {
            continue;
        };
        px_min = px_min.min(px);
        py_min = py_min.min(py);
        px_max = px_max.max(px);
        py_max = py_max.max(py);
    }
    if px_min.is_finite() {
        Some((px_min, py_min, px_max - px_min, py_max - py_min))
    } else {
        None
    }
}

// ── Rotation helper ───────────────────────────────────────────────────────────

/// If `rotate` is a non-zero angle, returns the degrees to rotate the node's
/// commands around its center; else None. (deg unit; value read directly.)
pub(super) fn rotation_degrees(rotate: Option<&Dimension>) -> Option<f64> {
    rotate.map(|d| d.value).filter(|a| *a != 0.0)
}

// ── Transform helper ──────────────────────────────────────────────────────────

/// `true` when a `PushScaleTranslate { sx, sy, tx, ty }` would be the identity
/// map (each term within `f64::EPSILON` of 1 or 0), so the push is left out.
pub(super) fn is_identity_transform(sx: f64, sy: f64, tx: f64, ty: f64) -> bool {
    let near = |value: f64, target: f64| (value - target).abs() <= f64::EPSILON;
    near(sx, 1.0) && near(sy, 1.0) && near(tx, 0.0) && near(ty, 0.0)
}

// ── Blend-mode helper ───────────────────────────────────────────────────────

/// Map a `blend-mode` attribute string to a non-`Normal` [`BlendMode`], or
/// `None` when no compositing layer is needed.
///
/// `None`, `"normal"`, and any unrecognized value all return `None` — those
/// nodes compile to a plain (layer-free) command stream, byte-identical to
/// before blend-mode existed. Only non-normal recognized blends open a layer.
pub(super) fn blend_mode_ir(s: Option<&str>) -> Option<BlendMode> {
    match s.and_then(BlendMode::from_kebab) {
        // "normal", None, and unrecognized values: no layer.
        Some(BlendMode::Normal) | None => None,
        Some(mode) => Some(mode),
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
pub(crate) fn px(v: f64) -> Dimension {
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
pub(crate) fn px_prop(v: f64) -> PropertyValue {
    PropertyValue::Dimension(px(v))
}

/// Resolve an optional dimension-valued property to pixels.
///
/// Returns `default` when the property is absent, is a raw literal, references
/// a non-dimension (or unresolved) token, or carries an unsupported unit. The
/// idiomatic path is a token ref resolving to a `Dimension`. Shared by
/// font-size and stroke-width resolution.
pub(crate) fn resolve_property_dimension_px(
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

/// Resolve an optional geometry property (`x`/`y`/`w`/`h`) to pixels: the
/// core resolver, shared with validation and the layout writer.
pub(crate) use zenith_core::resolve_geometry_px;

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
