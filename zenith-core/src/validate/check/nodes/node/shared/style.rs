//! Style/visual-property validators: text spans, font features/alternates,
//! style refs, the shared rect/pattern visual-property block, and stroke props.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::node::TextSpan;
use crate::ast::value::{Dimension, PropertyValue};
use crate::diagnostics::Diagnostic;
use crate::tokens::ResolvedToken;
use crate::validate::check::visual::{VisualExpect, check_visual_prop};

/// Whether `s` is one of the recognized `blend-mode` values. Unknown values
/// warn at validation time.
pub(in crate::validate::check) fn is_valid_blend_mode(s: &str) -> bool {
    crate::color::BlendMode::from_kebab(s).is_some()
}

/// Validate the `fill`, `font-weight`, `highlight`, `font-features`, and
/// `letter-spacing`
/// properties on a slice of [`TextSpan`]s, registering any token references so
/// they are not falsely flagged as unused.
///
/// Used by every node kind that carries a `spans` field (`text`, `shape`,
/// `footnote`). The `node_id` is the PARENT node's id (spans have no id of
/// their own).
pub(in crate::validate::check) fn check_spans(
    node_id: &str,
    spans: &[TextSpan],
    referenced_token_ids: &mut BTreeSet<String>,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for span in spans {
        check_visual_prop(
            node_id,
            "fill",
            span.fill.as_ref(),
            VisualExpect::Color,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
        check_visual_prop(
            node_id,
            "font-weight",
            span.font_weight.as_ref(),
            VisualExpect::FontWeight,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
        check_visual_prop(
            node_id,
            "highlight",
            span.highlight.as_ref(),
            VisualExpect::Color,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
        check_font_features(node_id, span.font_features.as_deref(), None, diagnostics);
        check_font_alternates(node_id, span.font_alternates.as_deref(), None, diagnostics);
        check_visual_prop(
            node_id,
            "letter-spacing",
            span.letter_spacing.as_ref(),
            VisualExpect::Dimension,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
    }
}

pub(in crate::validate::check) fn check_font_features(
    node_id: &str,
    raw: Option<&str>,
    span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(raw) = raw else {
        return;
    };

    for item in raw.split(',') {
        let spec = item.trim();
        if spec.is_empty() {
            continue;
        }

        let (tag, value) = match spec.split_once('=') {
            Some((tag, value_raw)) => (tag.trim(), Some(value_raw.trim())),
            None => (spec, None),
        };
        if tag.len() != 4 || !tag.as_bytes().iter().all(u8::is_ascii) {
            diagnostics.push(Diagnostic::warning(
                "font.invalid_feature",
                format!(
                    "node '{node_id}' has OpenType feature tag '{tag}', expected exactly four ASCII bytes"
                ),
                span,
                Some(node_id.to_owned()),
            ));
        }
        if let Some(value) = value
            && value.parse::<u32>().is_err()
        {
            diagnostics.push(Diagnostic::warning(
                "font.invalid_feature",
                format!("node '{node_id}' has OpenType feature '{spec}' with a non-u32 value"),
                span,
                Some(node_id.to_owned()),
            ));
        }
    }
}

pub(in crate::validate::check) fn check_font_alternates(
    node_id: &str,
    raw: Option<&str>,
    span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(raw) = raw else {
        return;
    };

    for item in raw.split(',') {
        let spec = item.trim();
        if spec.is_empty() {
            continue;
        }
        if let Err(err) = crate::font::parse_font_alternate_spec(spec) {
            diagnostics.push(Diagnostic::warning(
                "font.invalid_feature",
                format!(
                    "node '{node_id}' has OpenType alternate '{}': {}",
                    err.spec, err.message
                ),
                span,
                Some(node_id.to_owned()),
            ));
        }
    }
}

/// Check that a node's `style` attribute references a declared style id.
///
/// Called for every node kind that carries a `style` field.
pub(in crate::validate::check) fn check_style_ref(
    node_id: &str,
    style_opt: Option<&str>,
    declared_style_ids: &BTreeSet<String>,
    span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(sid) = style_opt
        && !declared_style_ids.contains(sid)
    {
        diagnostics.push(Diagnostic::error(
            "style.unknown_reference",
            format!(
                "node '{}': references style '{}' which is not declared in the styles block",
                node_id, sid
            ),
            span,
            Some(node_id.to_owned()),
        ));
    }
}

// ── Shared visual-property block ──────────────────────────────────────────────

/// A borrowed view of every visual property shared between `rect` and `pattern`.
///
/// The two node kinds carry an identical visual-property surface (fill, stroke,
/// per-side borders, blend-mode, shadow/filter/mask, blur), so the validation of
/// that surface lives in one place ([`check_visual_props`]). The only structural
/// difference is the corner-radius set: `rect` has `radius`/`radius-*`, while
/// `pattern` has none. A `PatternNode` therefore passes all radius fields as
/// `None`; `check_visual_prop` on a `None` value is a no-op and the per-corner
/// guard only fires on `Some(Dimension)`, so a pattern emits nothing for radius
/// while a rect keeps its exact ordering (radius between blend-mode and shadow).
///
/// All fields are `Copy` (`Option<&_>` / `Option<&str>`), so the whole view is
/// `Copy` and can be passed by value without bundling extra arguments.
#[derive(Clone, Copy)]
pub(in crate::validate::check) struct VisualProps<'a> {
    pub(in crate::validate::check) fill: Option<&'a PropertyValue>,
    pub(in crate::validate::check) stroke: Option<&'a PropertyValue>,
    pub(in crate::validate::check) stroke_width: Option<&'a PropertyValue>,
    pub(in crate::validate::check) stroke_dash: Option<&'a PropertyValue>,
    pub(in crate::validate::check) stroke_gap: Option<&'a PropertyValue>,
    pub(in crate::validate::check) stroke_linecap: Option<&'a str>,
    pub(in crate::validate::check) border_top: Option<&'a PropertyValue>,
    pub(in crate::validate::check) border_bottom: Option<&'a PropertyValue>,
    pub(in crate::validate::check) border_left: Option<&'a PropertyValue>,
    pub(in crate::validate::check) border_right: Option<&'a PropertyValue>,
    pub(in crate::validate::check) stroke_outer: Option<&'a PropertyValue>,
    pub(in crate::validate::check) border_width: Option<&'a PropertyValue>,
    pub(in crate::validate::check) stroke_outer_width: Option<&'a PropertyValue>,
    pub(in crate::validate::check) blend_mode: Option<&'a str>,
    /// Corner radius props: supplied by `rect`, all `None` for `pattern`.
    pub(in crate::validate::check) radius: Option<&'a PropertyValue>,
    pub(in crate::validate::check) radius_tl: Option<&'a PropertyValue>,
    pub(in crate::validate::check) radius_tr: Option<&'a PropertyValue>,
    pub(in crate::validate::check) radius_br: Option<&'a PropertyValue>,
    pub(in crate::validate::check) radius_bl: Option<&'a PropertyValue>,
    pub(in crate::validate::check) shadow: Option<&'a PropertyValue>,
    pub(in crate::validate::check) filter: Option<&'a PropertyValue>,
    pub(in crate::validate::check) mask: Option<&'a PropertyValue>,
    pub(in crate::validate::check) blur: Option<&'a Dimension>,
}

/// Validate the shared visual-property block for `rect` and `pattern`, pushing
/// diagnostics in exactly the order the original inline blocks did:
/// fill/stroke/stroke-width → stroke-dash (+ negative guard) → stroke-gap
/// (+ negative guard) → stroke-linecap → per-side borders → border widths →
/// blend-mode → radius (uniform + per-corner, each with a negative guard) →
/// shadow/filter/mask → blur (negative guard). `kind` is substituted into every
/// message ("rect" or "pattern").
pub(in crate::validate::check) fn check_visual_props(
    kind: &str,
    id: &str,
    source_span: Option<crate::ast::Span>,
    props: VisualProps<'_>,
    referenced_token_ids: &mut BTreeSet<String>,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_visual_prop(
        id,
        "fill",
        props.fill,
        VisualExpect::ColorOrGradient,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        id,
        "stroke",
        props.stroke,
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        id,
        "stroke-width",
        props.stroke_width,
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        id,
        "stroke-dash",
        props.stroke_dash,
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    if let Some(PropertyValue::Dimension(d)) = props.stroke_dash
        && d.value < 0.0
    {
        diagnostics.push(Diagnostic::error(
            "node.invalid_geometry",
            format!("{kind} '{id}': stroke-dash must be >= 0"),
            source_span,
            Some(id.to_owned()),
        ));
    }
    check_visual_prop(
        id,
        "stroke-gap",
        props.stroke_gap,
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    if let Some(PropertyValue::Dimension(d)) = props.stroke_gap
        && d.value < 0.0
    {
        diagnostics.push(Diagnostic::error(
            "node.invalid_geometry",
            format!("{kind} '{id}': stroke-gap must be >= 0"),
            source_span,
            Some(id.to_owned()),
        ));
    }
    if let Some(lc) = props.stroke_linecap
        && !matches!(lc, "butt" | "round" | "square")
    {
        check_stroke_linecap_prop(kind, id, Some(lc), source_span, diagnostics);
    }
    // Per-side border colors (token-required color props).
    for (prop_name, prop_val) in [
        ("border-top", props.border_top),
        ("border-bottom", props.border_bottom),
        ("border-left", props.border_left),
        ("border-right", props.border_right),
        ("stroke-outer", props.stroke_outer),
    ] {
        check_visual_prop(
            id,
            prop_name,
            prop_val,
            VisualExpect::Color,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
    }
    // Per-side border width + outer stroke width (token-required dimension props).
    for (prop_name, prop_val) in [
        ("border-width", props.border_width),
        ("stroke-outer-width", props.stroke_outer_width),
    ] {
        check_visual_prop(
            id,
            prop_name,
            prop_val,
            VisualExpect::Dimension,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
    }
    if let Some(bm) = props.blend_mode
        && !is_valid_blend_mode(bm)
    {
        diagnostics.push(Diagnostic::warning(
            "node.unknown_property",
            format!(
                "{kind} '{id}': blend-mode '{bm}' is not a recognized value; valid values are: {}",
                crate::color::BlendMode::joined_kebab(", ")
            ),
            source_span,
            Some(id.to_owned()),
        ));
    }
    // Corner radius: uniform then per-corner overrides. Absent for patterns
    // (all radius fields `None`), so a pattern emits nothing here while a rect
    // keeps radius ordered between blend-mode and shadow.
    check_visual_prop(
        id,
        "radius",
        props.radius,
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    for (prop_name, prop_val) in [
        ("radius-tl", props.radius_tl),
        ("radius-tr", props.radius_tr),
        ("radius-br", props.radius_br),
        ("radius-bl", props.radius_bl),
    ] {
        check_visual_prop(
            id,
            prop_name,
            prop_val,
            VisualExpect::Dimension,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
        if let Some(PropertyValue::Dimension(d)) = prop_val
            && d.value < 0.0
        {
            diagnostics.push(Diagnostic::error(
                "node.invalid_geometry",
                format!("{kind} '{id}': {prop_name} must be >= 0"),
                source_span,
                Some(id.to_owned()),
            ));
        }
    }
    check_visual_prop(
        id,
        "shadow",
        props.shadow,
        VisualExpect::Shadow,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        id,
        "filter",
        props.filter,
        VisualExpect::Filter,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        id,
        "mask",
        props.mask,
        VisualExpect::Mask,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    if let Some(d) = props.blur
        && d.value < 0.0
    {
        diagnostics.push(Diagnostic::error(
            "node.invalid_geometry",
            format!("{kind} '{id}': blur must be >= 0"),
            source_span,
            Some(id.to_owned()),
        ));
    }
}

pub(in crate::validate::check) fn check_stroke_join_props(
    kind: &str,
    id: &str,
    stroke_linejoin: Option<&str>,
    stroke_miter_limit: Option<f64>,
    source_span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(lj) = stroke_linejoin
        && !matches!(lj, "miter" | "round" | "bevel")
    {
        diagnostics.push(Diagnostic::warning(
            "node.unknown_property",
            format!("{kind} '{id}': stroke-linejoin '{lj}' is not one of miter/round/bevel"),
            source_span,
            Some(id.to_owned()),
        ));
    }
    if let Some(limit) = stroke_miter_limit
        && (!limit.is_finite() || limit <= 0.0)
    {
        diagnostics.push(Diagnostic::error(
            "node.invalid_geometry",
            format!("{kind} '{id}': stroke-miter-limit must be a positive finite number"),
            source_span,
            Some(id.to_owned()),
        ));
    }
}

pub(in crate::validate::check) fn check_stroke_linecap_prop(
    kind: &str,
    id: &str,
    stroke_linecap: Option<&str>,
    source_span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(lc) = stroke_linecap
        && !matches!(lc, "butt" | "round" | "square")
    {
        diagnostics.push(Diagnostic::warning(
            "node.unknown_property",
            format!("{kind} '{id}': stroke-linecap '{lc}' is not one of butt/round/square"),
            source_span,
            Some(id.to_owned()),
        ));
    }
}
