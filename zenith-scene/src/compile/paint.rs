//! Paint resolvers: turn `PropertyValue`s into concrete `Color`,
//! `GradientPaint`, and `ShadowSpec` values, plus the gradient opacity cascade.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, GradientKind, PropertyValue, ResolvedToken, ResolvedValue};

use crate::color::{parse_color, parse_srgb_hex};
use crate::ir::{
    Color, FilterSpec, GradientPaint, GradientStop, MaskShape, MaskSpec, SceneCommand, ShadowSpec,
};

/// Build an [`ir::Color`](Color) from a resolved color token, preserving its
/// CMYK origin when present. Returns `None` only when the resolved value is not
/// a color or its stored hex is somehow unparseable (which token resolution
/// already guarantees not to happen).
fn color_from_resolved(rv: &ResolvedValue) -> Option<Color> {
    let hex = rv.as_color_hex()?;
    let mut color = parse_srgb_hex(hex)?;
    if let Some((c, m, y, k)) = rv.cmyk() {
        color.cmyk = Some([c, m, y, k]);
    }
    Some(color)
}

/// Resolve a `PropertyValue` to a `Color`, or push a diagnostic and return
/// `None`.
///
/// Accepts:
/// - `TokenRef(id)` → looks up in `resolved`, must be a `ResolvedValue::Color`.
/// - `Literal(hex)` → parses as sRGB hex string directly.
pub(super) fn resolve_property_color(
    prop: &PropertyValue,
    resolved: &BTreeMap<String, ResolvedToken>,
    diagnostics: &mut Vec<Diagnostic>,
    subject_id: &str,
) -> Option<Color> {
    match prop {
        PropertyValue::TokenRef(token_id) => {
            match resolved.get(token_id.as_str()) {
                Some(rt) if rt.value.as_color_hex().is_some() => {
                    match color_from_resolved(&rt.value) {
                        Some(c) => Some(c),
                        None => {
                            // Should not happen — token resolution validates the
                            // hex / cmyk literal — but be robust.
                            diagnostics.push(Diagnostic::advisory(
                                "scene.invalid_color",
                                format!(
                                    "token '{}' resolved to an invalid color; skipped",
                                    token_id
                                ),
                                None,
                                Some(subject_id.to_owned()),
                            ));
                            None
                        }
                    }
                }
                Some(rt) => {
                    diagnostics.push(Diagnostic::advisory(
                        "scene.wrong_token_type",
                        format!(
                            "node '{}' references token '{}' which resolved to a \
                             non-color value ({:?}); skipped",
                            subject_id, token_id, &rt.value
                        ),
                        None,
                        Some(subject_id.to_owned()),
                    ));
                    None
                }
                None => {
                    diagnostics.push(Diagnostic::advisory(
                        "scene.unresolved_token",
                        format!(
                            "node '{}' references token '{}' which did not resolve \
                             (check token diagnostics); skipped",
                            subject_id, token_id
                        ),
                        None,
                        Some(subject_id.to_owned()),
                    ));
                    None
                }
            }
        }
        PropertyValue::Literal(literal) => match parse_color(literal) {
            Some(c) => Some(c),
            None => {
                diagnostics.push(Diagnostic::advisory(
                    "scene.invalid_color",
                    format!(
                        "node '{}' has a fill literal '{}' that is not a valid \
                         sRGB hex or cmyk(...) color; skipped",
                        subject_id, literal
                    ),
                    None,
                    Some(subject_id.to_owned()),
                ));
                None
            }
        },
        // A dimension is not a color; advise and skip (mirrors wrong-type tokens).
        PropertyValue::Dimension(_) => {
            diagnostics.push(Diagnostic::advisory(
                "scene.wrong_token_type",
                format!(
                    "node '{}' has a dimension value where a color is expected; skipped",
                    subject_id
                ),
                None,
                Some(subject_id.to_owned()),
            ));
            None
        }
    }
}

/// Resolve a fill `PropertyValue` into a [`GradientPaint`], or `None`.
///
/// Returns `Some` only when `prop` is a `TokenRef` whose token resolved to a
/// `ResolvedValue::Gradient`. Each stop's color is resolved from its
/// `color_token_id` via the resolved token map (must be `ResolvedValue::Color`);
/// stops whose color cannot resolve are skipped. Returns `None` (so the caller
/// falls back to the solid path) for non-gradient props, or when fewer than two
/// valid stops survive.
pub(super) fn resolve_property_gradient(
    prop: &PropertyValue,
    resolved: &BTreeMap<String, ResolvedToken>,
    _subject_id: &str,
) -> Option<GradientPaint> {
    let PropertyValue::TokenRef(token_id) = prop else {
        return None;
    };
    let ResolvedValue::Gradient(g) = &resolved.get(token_id.as_str())?.value else {
        return None;
    };

    let mut stops: Vec<GradientStop> = Vec::with_capacity(g.stops.len());
    for (offset, color_token_id) in &g.stops {
        let Some(rt) = resolved.get(color_token_id.as_str()) else {
            continue;
        };
        let Some(color) = color_from_resolved(&rt.value) else {
            continue;
        };
        stops.push(GradientStop {
            offset: *offset,
            color,
        });
    }

    if stops.len() < 2 {
        return None;
    }
    Some(GradientPaint {
        angle_deg: g.angle_deg,
        stops,
        radial: matches!(g.kind, GradientKind::Radial),
        center_x: g.center_x,
        center_y: g.center_y,
        radius_frac: g.radius,
    })
}

/// Resolve a `shadow` `PropertyValue` into a list of [`ShadowSpec`] layers, or
/// `None`.
///
/// Mirrors [`resolve_property_gradient`]: returns `Some` only when `prop` is a
/// `TokenRef` whose token resolved to a `ResolvedValue::Shadow`. Each layer's
/// color is resolved from its `color_token` via the resolved token map (must be
/// `ResolvedValue::Color`); layers whose color cannot resolve are skipped.
/// Returns `None` for non-shadow props, or when zero valid layers survive.
pub(super) fn resolve_property_shadow(
    prop: &PropertyValue,
    resolved: &BTreeMap<String, ResolvedToken>,
    _subject_id: &str,
) -> Option<Vec<ShadowSpec>> {
    let PropertyValue::TokenRef(token_id) = prop else {
        return None;
    };
    let ResolvedValue::Shadow(s) = &resolved.get(token_id.as_str())?.value else {
        return None;
    };

    let mut layers: Vec<ShadowSpec> = Vec::with_capacity(s.layers.len());
    for layer in &s.layers {
        let Some(rt) = resolved.get(layer.color_token.as_str()) else {
            continue;
        };
        let Some(color) = color_from_resolved(&rt.value) else {
            continue;
        };
        layers.push(ShadowSpec {
            dx: layer.dx,
            dy: layer.dy,
            blur: layer.blur,
            color,
        });
    }

    if layers.is_empty() {
        return None;
    }
    Some(layers)
}

/// Resolve a `filter` `PropertyValue` into a list of [`FilterSpec`] operations,
/// or `None`.
///
/// Mirrors [`resolve_property_shadow`]: returns `Some` only when `prop` is a
/// `TokenRef` whose token resolved to a `ResolvedValue::Filter`. Each core op is
/// mapped to a [`FilterSpec`] variant carrying its resolved scalar payload; the
/// per-kind default `amount` is substituted when the op leaves it unspecified.
/// For `Duotone`, the op's shadow/highlight color-token ids are resolved to
/// concrete [`Color`]s (the same lookup [`resolve_property_shadow`] uses); if
/// either color is missing or unresolvable, that op is SKIPPED. Returns `None`
/// for non-filter props, or when no op survives.
pub(super) fn resolve_property_filter(
    prop: &PropertyValue,
    resolved: &BTreeMap<String, ResolvedToken>,
    _subject_id: &str,
) -> Option<Vec<FilterSpec>> {
    let PropertyValue::TokenRef(token_id) = prop else {
        return None;
    };
    let ResolvedValue::Filter(f) = &resolved.get(token_id.as_str())?.value else {
        return None;
    };

    let mut ops: Vec<FilterSpec> = Vec::with_capacity(f.ops.len());
    for op in &f.ops {
        // Default amounts match the historical scalar defaults: 1.0 for every
        // kind except hue-rotate (0.0 degrees) and duotone (1.0 mix factor).
        let spec = match op.kind {
            zenith_core::FilterKind::Grayscale => FilterSpec::Grayscale(op.amount.unwrap_or(1.0)),
            zenith_core::FilterKind::Invert => FilterSpec::Invert(op.amount.unwrap_or(1.0)),
            zenith_core::FilterKind::Sepia => FilterSpec::Sepia(op.amount.unwrap_or(1.0)),
            zenith_core::FilterKind::Saturate => FilterSpec::Saturate(op.amount.unwrap_or(1.0)),
            zenith_core::FilterKind::Brightness => FilterSpec::Brightness(op.amount.unwrap_or(1.0)),
            zenith_core::FilterKind::Contrast => FilterSpec::Contrast(op.amount.unwrap_or(1.0)),
            zenith_core::FilterKind::HueRotate => FilterSpec::HueRotate(op.amount.unwrap_or(0.0)),
            zenith_core::FilterKind::Duotone => {
                // Resolve both color tokens; skip the op if either is missing
                // or not a color, mirroring how unresolvable shadow layers skip.
                let (Some(shadow_id), Some(highlight_id)) =
                    (op.shadow.as_deref(), op.highlight.as_deref())
                else {
                    continue;
                };
                let Some(shadow) = resolved
                    .get(shadow_id)
                    .and_then(|rt| color_from_resolved(&rt.value))
                else {
                    continue;
                };
                let Some(highlight) = resolved
                    .get(highlight_id)
                    .and_then(|rt| color_from_resolved(&rt.value))
                else {
                    continue;
                };
                FilterSpec::Duotone {
                    amount: op.amount.unwrap_or(1.0),
                    shadow,
                    highlight,
                }
            }
            zenith_core::FilterKind::Noise => FilterSpec::Noise {
                amount: op.amount.unwrap_or(1.0),
                seed: op.seed.unwrap_or(0),
                scale: op.scale.unwrap_or(1.0),
            },
        };
        ops.push(spec);
    }

    if ops.is_empty() {
        return None;
    }
    Some(ops)
}

/// Map a `zenith_core::MaskShape` to the scene-IR [`MaskShape`].
///
/// Exhaustive match keeps the scene IR decoupled from the core enum and surfaces
/// a compile error if a new core variant is ever added.
fn map_shape(shape: zenith_core::MaskShape) -> MaskShape {
    match shape {
        zenith_core::MaskShape::Rect => MaskShape::Rect,
        zenith_core::MaskShape::RoundedRect => MaskShape::RoundedRect,
        zenith_core::MaskShape::Ellipse => MaskShape::Ellipse,
    }
}

/// Resolve a `mask` `PropertyValue` into a [`MaskSpec`] carrying the node box,
/// or `None`.
///
/// Mirrors [`resolve_property_filter`] but additionally needs the node's
/// page-absolute box `(x, y, w, h)` so the resolved spec can describe the mask
/// coverage geometry. Returns `Some` only when `prop` is a `TokenRef` whose
/// token resolved to a `ResolvedValue::Mask`; the resolved corner radius defaults
/// to `0.0` when the mask token leaves it unspecified. Returns `None` for any
/// non-mask prop.
pub(super) fn resolve_property_mask(
    prop: &PropertyValue,
    resolved: &BTreeMap<String, ResolvedToken>,
    node_box: (f64, f64, f64, f64),
) -> Option<MaskSpec> {
    let PropertyValue::TokenRef(token_id) = prop else {
        return None;
    };
    let ResolvedValue::Mask(m) = &resolved.get(token_id.as_str())?.value else {
        return None;
    };
    let (x, y, w, h) = node_box;
    Some(MaskSpec {
        shape: map_shape(m.shape),
        radius: m.radius.unwrap_or(0.0),
        feather: m.feather,
        invert: m.invert,
        x,
        y,
        w,
        h,
    })
}

/// The single winning post-draw effect a leaf node carries, in precedence order
/// blur > shadow > filter (exactly the precedence the leaf resolution applies).
///
/// Used by [`emit_node_with_effects`] to emit the correct Begin*/End* bracket
/// around a node's draws.
pub(super) enum NodeEffect {
    Blur(f64),
    Shadow(Vec<ShadowSpec>),
    Filter(Vec<FilterSpec>),
}

/// Emit `draws` into `out`, wrapped by the node's effect and/or mask.
///
/// - no effect, no mask  → draws verbatim (BYTE-IDENTICAL to the pre-mask path).
/// - effect, no mask     → BeginEffect, draws, EndEffect (BYTE-IDENTICAL).
/// - mask, no effect     → BeginMask, draws, EndMask (soft reveal).
/// - effect AND mask     → draws (sharp base), then
///   BeginMask, BeginEffect, draws, EndEffect, EndMask (sharp center where
///   coverage = 0, effected feathered edges where coverage = 1).
pub(super) fn emit_node_with_effects(
    out: &mut Vec<SceneCommand>,
    draws: Vec<SceneCommand>,
    effect: Option<NodeEffect>,
    mask: Option<MaskSpec>,
) {
    let (begin, end): (Option<SceneCommand>, Option<SceneCommand>) = match &effect {
        Some(NodeEffect::Blur(r)) => (
            Some(SceneCommand::BeginBlur { radius: *r }),
            Some(SceneCommand::EndBlur),
        ),
        Some(NodeEffect::Shadow(s)) => (
            Some(SceneCommand::BeginShadow { shadows: s.clone() }),
            Some(SceneCommand::EndShadow),
        ),
        Some(NodeEffect::Filter(f)) => (
            Some(SceneCommand::BeginFilter { filters: f.clone() }),
            Some(SceneCommand::EndFilter),
        ),
        None => (None, None),
    };
    match (mask, begin) {
        (None, None) => out.extend(draws),
        (None, Some(b)) => {
            out.push(b);
            out.extend(draws);
            if let Some(e) = end {
                out.push(e);
            }
        }
        (Some(m), None) => {
            out.push(SceneCommand::BeginMask { mask: m });
            out.extend(draws);
            out.push(SceneCommand::EndMask);
        }
        (Some(m), Some(b)) => {
            out.extend(draws.clone()); // sharp base
            out.push(SceneCommand::BeginMask { mask: m });
            out.push(b);
            out.extend(draws);
            if let Some(e) = end {
                out.push(e);
            }
            out.push(SceneCommand::EndMask);
        }
    }
}

/// Apply the cascaded opacity multiplier to every stop's alpha, matching the
/// solid path's `color.a = (color.a * node_opacity * ctx.opacity).round()`.
pub(super) fn apply_gradient_opacity(
    gradient: &mut GradientPaint,
    node_opacity: f64,
    ctx_opacity: f64,
) {
    for stop in &mut gradient.stops {
        stop.color.a = (stop.color.a as f64 * node_opacity * ctx_opacity).round() as u8;
    }
}
