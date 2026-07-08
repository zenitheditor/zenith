//! Per-kind checks for the simple leaf nodes: `rect`, `ellipse`, `line`, and
//! `code`. Each mirrors the shape of the other `check_*` helpers and pushes
//! diagnostics in the same order the original inline match arms did.

use std::collections::BTreeSet;

use crate::ast::node::{CodeNode, EllipseNode, LineNode, RectNode};
use crate::ast::value::PropertyValue;
use crate::diagnostics::Diagnostic;

use super::kerning::check_kerning_pairs;
use super::shared::{
    AnchorParentCtx, AnchorProps, TokenEnv, VisualProps, check_anchor, check_dimension_geom,
    check_font_features, check_optional_dim, check_style_ref, check_visual_props,
    is_valid_blend_mode,
};
use super::suggest::check_unknown_props;
use crate::validate::check::nodes::WalkCtx;
use crate::validate::check::register_id;
use crate::validate::check::visual::{VisualExpect, check_visual_prop};

pub(in crate::validate::check) fn check_rect(
    r: &RectNode,
    ctx: WalkCtx,
    seen_ids: &mut BTreeSet<String>,
    referenced_token_ids: &mut BTreeSet<String>,
    geom_required: bool,
    parent_ctx: AnchorParentCtx,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let WalkCtx {
        resolved_tokens,
        declared_style_ids,
        zone_ids,
        ..
    } = ctx;
    register_id(&r.id, seen_ids, diagnostics);
    check_style_ref(
        &r.id,
        r.style.as_deref(),
        declared_style_ids,
        r.source_span,
        diagnostics,
    );

    // A recognized anchor supplies both x and y; x/y are not required
    // even outside a flow parent when a recognized anchor is present.
    let anchor_active = check_anchor(
        &r.id,
        AnchorProps {
            anchor: r.anchor.as_deref(),
            anchor_zone: r.anchor_zone.as_deref(),
            anchor_sibling: r.anchor_sibling.as_deref(),
            anchor_parent: r.anchor_parent == Some(true),
            anchor_edge: r.anchor_edge.as_deref(),
            anchor_gap: r.anchor_gap.as_ref(),
        },
        parent_ctx,
        zone_ids,
        r.source_span,
        diagnostics,
    );
    let xy_required = geom_required && !anchor_active;

    // Required geometry: x, y, w, h must all be present.
    {
        let mut tokens = TokenEnv {
            referenced: referenced_token_ids,
            resolved: resolved_tokens,
        };
        check_optional_dim(
            &r.id,
            "x",
            r.x.as_ref(),
            xy_required,
            r.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &r.id,
            "y",
            r.y.as_ref(),
            xy_required,
            r.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &r.id,
            "w",
            r.w.as_ref(),
            geom_required,
            r.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &r.id,
            "h",
            r.h.as_ref(),
            geom_required,
            r.source_span,
            &mut tokens,
            diagnostics,
        );
    }

    // Visual properties — shared with `pattern`; rect supplies the radius set.
    let props = VisualProps {
        fill: r.fill.as_ref(),
        stroke: r.stroke.as_ref(),
        stroke_width: r.stroke_width.as_ref(),
        stroke_dash: r.stroke_dash.as_ref(),
        stroke_gap: r.stroke_gap.as_ref(),
        stroke_linecap: r.stroke_linecap.as_deref(),
        border_top: r.border_top.as_ref(),
        border_bottom: r.border_bottom.as_ref(),
        border_left: r.border_left.as_ref(),
        border_right: r.border_right.as_ref(),
        stroke_outer: r.stroke_outer.as_ref(),
        border_width: r.border_width.as_ref(),
        stroke_outer_width: r.stroke_outer_width.as_ref(),
        blend_mode: r.blend_mode.as_deref(),
        radius: r.radius.as_ref(),
        radius_tl: r.radius_tl.as_ref(),
        radius_tr: r.radius_tr.as_ref(),
        radius_br: r.radius_br.as_ref(),
        radius_bl: r.radius_bl.as_ref(),
        shadow: r.shadow.as_ref(),
        filter: r.filter.as_ref(),
        mask: r.mask.as_ref(),
        blur: r.blur.as_ref(),
    };
    check_visual_props(
        "rect",
        &r.id,
        r.source_span,
        props,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    // Unknown properties.
    check_unknown_props("rect", &r.id, &r.unknown_props, r.source_span, diagnostics);
}

pub(in crate::validate::check) fn check_ellipse(
    e: &EllipseNode,
    ctx: WalkCtx,
    seen_ids: &mut BTreeSet<String>,
    referenced_token_ids: &mut BTreeSet<String>,
    geom_required: bool,
    parent_ctx: AnchorParentCtx,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let WalkCtx {
        resolved_tokens,
        declared_style_ids,
        zone_ids,
        ..
    } = ctx;
    register_id(&e.id, seen_ids, diagnostics);
    check_style_ref(
        &e.id,
        e.style.as_deref(),
        declared_style_ids,
        e.source_span,
        diagnostics,
    );

    // A recognized anchor supplies both x and y.
    let anchor_active = check_anchor(
        &e.id,
        AnchorProps {
            anchor: e.anchor.as_deref(),
            anchor_zone: e.anchor_zone.as_deref(),
            anchor_sibling: e.anchor_sibling.as_deref(),
            anchor_parent: e.anchor_parent == Some(true),
            anchor_edge: e.anchor_edge.as_deref(),
            anchor_gap: e.anchor_gap.as_ref(),
        },
        parent_ctx,
        zone_ids,
        e.source_span,
        diagnostics,
    );
    let xy_required = geom_required && !anchor_active;

    // Required geometry: x, y, w, h must all be present.
    {
        let mut tokens = TokenEnv {
            referenced: referenced_token_ids,
            resolved: resolved_tokens,
        };
        check_optional_dim(
            &e.id,
            "x",
            e.x.as_ref(),
            xy_required,
            e.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &e.id,
            "y",
            e.y.as_ref(),
            xy_required,
            e.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &e.id,
            "w",
            e.w.as_ref(),
            geom_required,
            e.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &e.id,
            "h",
            e.h.as_ref(),
            geom_required,
            e.source_span,
            &mut tokens,
            diagnostics,
        );
    }

    // Visual properties.
    check_visual_prop(
        &e.id,
        "fill",
        e.fill.as_ref(),
        VisualExpect::ColorOrGradient,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &e.id,
        "stroke",
        e.stroke.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &e.id,
        "stroke-width",
        e.stroke_width.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &e.id,
        "stroke-dash",
        e.stroke_dash.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    if let Some(PropertyValue::Dimension(d)) = e.stroke_dash.as_ref()
        && d.value < 0.0
    {
        diagnostics.push(Diagnostic::error(
            "node.invalid_geometry",
            format!("ellipse '{}': stroke-dash must be >= 0", e.id),
            e.source_span,
            Some(e.id.clone()),
        ));
    }
    check_visual_prop(
        &e.id,
        "stroke-gap",
        e.stroke_gap.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    if let Some(PropertyValue::Dimension(d)) = e.stroke_gap.as_ref()
        && d.value < 0.0
    {
        diagnostics.push(Diagnostic::error(
            "node.invalid_geometry",
            format!("ellipse '{}': stroke-gap must be >= 0", e.id),
            e.source_span,
            Some(e.id.clone()),
        ));
    }
    if let Some(lc) = e.stroke_linecap.as_deref()
        && !matches!(lc, "butt" | "round" | "square")
    {
        diagnostics.push(Diagnostic::warning(
            "node.unknown_property",
            format!(
                "ellipse '{}': stroke-linecap '{}' is not one of butt/round/square",
                e.id, lc
            ),
            e.source_span,
            Some(e.id.clone()),
        ));
    }
    if let Some(bm) = e.blend_mode.as_deref()
        && !is_valid_blend_mode(bm)
    {
        diagnostics.push(Diagnostic::warning(
            "node.unknown_property",
            format!(
                "ellipse '{}': blend-mode '{bm}' is not a recognized value; valid values are: {}",
                e.id,
                crate::color::BlendMode::joined_kebab(", ")
            ),
            e.source_span,
            Some(e.id.clone()),
        ));
    }
    // Independent axis radii: same validation as dimension props.
    for (prop_name, prop_val) in [("rx", e.rx.as_ref()), ("ry", e.ry.as_ref())] {
        check_visual_prop(
            &e.id,
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
                format!("ellipse '{}': {} must be >= 0", e.id, prop_name),
                e.source_span,
                Some(e.id.clone()),
            ));
        }
    }
    check_visual_prop(
        &e.id,
        "shadow",
        e.shadow.as_ref(),
        VisualExpect::Shadow,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &e.id,
        "filter",
        e.filter.as_ref(),
        VisualExpect::Filter,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &e.id,
        "mask",
        e.mask.as_ref(),
        VisualExpect::Mask,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    if let Some(d) = e.blur.as_ref()
        && d.value < 0.0
    {
        diagnostics.push(Diagnostic::error(
            "node.invalid_geometry",
            format!("ellipse '{}': blur must be >= 0", e.id),
            e.source_span,
            Some(e.id.clone()),
        ));
    }

    // Unknown properties.
    check_unknown_props(
        "ellipse",
        &e.id,
        &e.unknown_props,
        e.source_span,
        diagnostics,
    );
}

pub(in crate::validate::check) fn check_line(
    l: &LineNode,
    ctx: WalkCtx,
    seen_ids: &mut BTreeSet<String>,
    referenced_token_ids: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let WalkCtx {
        resolved_tokens,
        declared_style_ids,
        ..
    } = ctx;
    register_id(&l.id, seen_ids, diagnostics);
    check_style_ref(
        &l.id,
        l.style.as_deref(),
        declared_style_ids,
        l.source_span,
        diagnostics,
    );

    // Required geometry: x1, y1, x2, y2 must all be present.
    check_dimension_geom(&l.id, "x1", l.x1.as_ref(), true, l.source_span, diagnostics);
    check_dimension_geom(&l.id, "y1", l.y1.as_ref(), true, l.source_span, diagnostics);
    check_dimension_geom(&l.id, "x2", l.x2.as_ref(), true, l.source_span, diagnostics);
    check_dimension_geom(&l.id, "y2", l.y2.as_ref(), true, l.source_span, diagnostics);

    // Visual properties (stroke-only; no fill for line).
    // stroke is optional — only type-checked if present (a stroke-less
    // line draws nothing, but it is not an error to omit stroke).
    check_visual_prop(
        &l.id,
        "stroke",
        l.stroke.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &l.id,
        "stroke-width",
        l.stroke_width.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &l.id,
        "stroke-dash",
        l.stroke_dash.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    if let Some(PropertyValue::Dimension(d)) = l.stroke_dash.as_ref()
        && d.value < 0.0
    {
        diagnostics.push(Diagnostic::error(
            "node.invalid_geometry",
            format!("line '{}': stroke-dash must be >= 0", l.id),
            l.source_span,
            Some(l.id.clone()),
        ));
    }
    check_visual_prop(
        &l.id,
        "stroke-gap",
        l.stroke_gap.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    if let Some(PropertyValue::Dimension(d)) = l.stroke_gap.as_ref()
        && d.value < 0.0
    {
        diagnostics.push(Diagnostic::error(
            "node.invalid_geometry",
            format!("line '{}': stroke-gap must be >= 0", l.id),
            l.source_span,
            Some(l.id.clone()),
        ));
    }
    if let Some(lc) = l.stroke_linecap.as_deref()
        && !matches!(lc, "butt" | "round" | "square")
    {
        diagnostics.push(Diagnostic::warning(
            "node.unknown_property",
            format!(
                "line '{}': stroke-linecap '{}' is not one of butt/round/square",
                l.id, lc
            ),
            l.source_span,
            Some(l.id.clone()),
        ));
    }

    // Unknown properties.
    check_unknown_props("line", &l.id, &l.unknown_props, l.source_span, diagnostics);
}

pub(in crate::validate::check) fn check_code(
    c: &CodeNode,
    ctx: WalkCtx,
    seen_ids: &mut BTreeSet<String>,
    referenced_token_ids: &mut BTreeSet<String>,
    geom_required: bool,
    parent_ctx: AnchorParentCtx,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let WalkCtx {
        resolved_tokens,
        declared_style_ids,
        zone_ids,
        ..
    } = ctx;
    register_id(&c.id, seen_ids, diagnostics);
    check_style_ref(
        &c.id,
        c.style.as_deref(),
        declared_style_ids,
        c.source_span,
        diagnostics,
    );

    // A recognized anchor supplies both x and y.
    let anchor_active = check_anchor(
        &c.id,
        AnchorProps {
            anchor: c.anchor.as_deref(),
            anchor_zone: c.anchor_zone.as_deref(),
            anchor_sibling: c.anchor_sibling.as_deref(),
            anchor_parent: c.anchor_parent == Some(true),
            anchor_edge: c.anchor_edge.as_deref(),
            anchor_gap: c.anchor_gap.as_ref(),
        },
        parent_ctx,
        zone_ids,
        c.source_span,
        diagnostics,
    );
    let xy_required = geom_required && !anchor_active;

    // Geometry (advisory box for v0; only unit-checked if present).
    {
        let mut tokens = TokenEnv {
            referenced: referenced_token_ids,
            resolved: resolved_tokens,
        };
        check_optional_dim(
            &c.id,
            "x",
            c.x.as_ref(),
            xy_required,
            c.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &c.id,
            "y",
            c.y.as_ref(),
            xy_required,
            c.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &c.id,
            "w",
            c.w.as_ref(),
            geom_required,
            c.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &c.id,
            "h",
            c.h.as_ref(),
            geom_required,
            c.source_span,
            &mut tokens,
            diagnostics,
        );
    }

    // Visual properties (mirror text; overflow is not enum-validated,
    // matching how text.overflow is currently handled).
    check_visual_prop(
        &c.id,
        "fill",
        c.fill.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &c.id,
        "font-family",
        c.font_family.as_ref(),
        VisualExpect::FontFamily,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &c.id,
        "font-size",
        c.font_size.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &c.id,
        "font-weight",
        c.font_weight.as_ref(),
        VisualExpect::FontWeight,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_font_features(
        &c.id,
        c.font_features.as_deref(),
        c.source_span,
        diagnostics,
    );
    check_visual_prop(
        &c.id,
        "letter-spacing",
        c.letter_spacing.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_kerning_pairs(
        "code",
        &c.id,
        &c.kerning_pairs,
        c.source_span,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    // Unknown properties.
    check_unknown_props("code", &c.id, &c.unknown_props, c.source_span, diagnostics);
}
