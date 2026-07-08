//! Per-kind checks for the "special" leaf nodes that were already extracted as
//! helpers: `polygon`, `polyline`, `path`, `instance`, `field`, `toc`, and `footnote`.
//! None of these recurse into laid-out children at this site.

use std::collections::BTreeSet;

use crate::ast::node::{
    AnchorKind, FieldNode, FootnoteNode, InstanceNode, PathAnchor, PathNode, PolygonNode,
    PolylineNode, TocNode,
};
use crate::diagnostics::Diagnostic;

use super::shared::{
    AnchorParentCtx, AnchorProps, check_anchor, check_dimension_geom, check_spans,
    check_stroke_join_props, check_stroke_linecap_prop, check_style_ref,
};
use super::suggest::check_unknown_props;
use crate::validate::check::nodes::WalkCtx;
use crate::validate::check::register_id;
use crate::validate::check::visual::{VisualExpect, check_visual_prop};

// ── polygon / polyline validation ─────────────────────────────────────────────

pub(in crate::validate::check) fn check_polygon(
    poly: &PolygonNode,
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
    register_id(&poly.id, seen_ids, diagnostics);
    check_style_ref(
        &poly.id,
        poly.style.as_deref(),
        declared_style_ids,
        poly.source_span,
        diagnostics,
    );

    // Validate each point's x and y (both must be present with a known unit).
    for (idx, pt) in poly.points.iter().enumerate() {
        let x_label = format!("point[{idx}].x");
        let y_label = format!("point[{idx}].y");
        check_dimension_geom(
            &poly.id,
            &x_label,
            pt.x.as_ref(),
            true,
            poly.source_span,
            diagnostics,
        );
        check_dimension_geom(
            &poly.id,
            &y_label,
            pt.y.as_ref(),
            true,
            poly.source_span,
            diagnostics,
        );
    }

    // polygon requires at least 3 points.
    if poly.points.len() < 3 {
        diagnostics.push(Diagnostic::error(
            "shape.insufficient_points",
            format!(
                "polygon '{}': requires at least 3 points, got {}",
                poly.id,
                poly.points.len()
            ),
            poly.source_span,
            Some(poly.id.clone()),
        ));
    }

    // Visual properties. Fill accepts a color OR a gradient token (the scene
    // paints any geometry uniformly); stroke is color-only.
    check_visual_prop(
        &poly.id,
        "fill",
        poly.fill.as_ref(),
        VisualExpect::ColorOrGradient,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &poly.id,
        "stroke",
        poly.stroke.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &poly.id,
        "stroke-width",
        poly.stroke_width.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    // fill-rule: only "nonzero" and "evenodd" are valid.
    if let Some(fr) = &poly.fill_rule
        && !matches!(fr.as_str(), "nonzero" | "evenodd")
    {
        diagnostics.push(Diagnostic::warning(
            "node.unknown_property",
            format!(
                "polygon '{}': unrecognized fill-rule '{}' (version-relative; \
                 allowed values are nonzero, evenodd)",
                poly.id, fr
            ),
            poly.source_span,
            Some(poly.id.clone()),
        ));
    }

    // stroke-alignment: only "inside", "center", "outside" are valid.
    if let Some(sa) = &poly.stroke_alignment
        && !matches!(sa.as_str(), "inside" | "center" | "outside")
    {
        diagnostics.push(Diagnostic::warning(
            "node.unknown_property",
            format!(
                "polygon '{}': unrecognized stroke-alignment '{}' (version-relative; \
                 allowed values are inside, center, outside)",
                poly.id, sa
            ),
            poly.source_span,
            Some(poly.id.clone()),
        ));
    }

    // Unknown properties.
    check_unknown_props(
        "polygon",
        &poly.id,
        &poly.unknown_props,
        poly.source_span,
        diagnostics,
    );
    // polygon is a LEAF: no child-node recursion (points are sub-data).
}

pub(in crate::validate::check) fn check_polyline(
    poly: &PolylineNode,
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
    register_id(&poly.id, seen_ids, diagnostics);
    check_style_ref(
        &poly.id,
        poly.style.as_deref(),
        declared_style_ids,
        poly.source_span,
        diagnostics,
    );

    // Validate each point's x and y.
    for (idx, pt) in poly.points.iter().enumerate() {
        let x_label = format!("point[{idx}].x");
        let y_label = format!("point[{idx}].y");
        check_dimension_geom(
            &poly.id,
            &x_label,
            pt.x.as_ref(),
            true,
            poly.source_span,
            diagnostics,
        );
        check_dimension_geom(
            &poly.id,
            &y_label,
            pt.y.as_ref(),
            true,
            poly.source_span,
            diagnostics,
        );
    }

    // polyline requires at least 2 points.
    if poly.points.len() < 2 {
        diagnostics.push(Diagnostic::error(
            "shape.insufficient_points",
            format!(
                "polyline '{}': requires at least 2 points, got {}",
                poly.id,
                poly.points.len()
            ),
            poly.source_span,
            Some(poly.id.clone()),
        ));
    }

    // Visual properties. Fill accepts a color OR a gradient token (the scene
    // paints any geometry uniformly); stroke is color-only.
    check_visual_prop(
        &poly.id,
        "fill",
        poly.fill.as_ref(),
        VisualExpect::ColorOrGradient,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &poly.id,
        "stroke",
        poly.stroke.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &poly.id,
        "stroke-width",
        poly.stroke_width.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    // fill-rule: only "nonzero" and "evenodd" are valid.
    if let Some(fr) = &poly.fill_rule
        && !matches!(fr.as_str(), "nonzero" | "evenodd")
    {
        diagnostics.push(Diagnostic::warning(
            "node.unknown_property",
            format!(
                "polyline '{}': unrecognized fill-rule '{}' (version-relative; \
                 allowed values are nonzero, evenodd)",
                poly.id, fr
            ),
            poly.source_span,
            Some(poly.id.clone()),
        ));
    }

    // Unknown properties.
    check_unknown_props(
        "polyline",
        &poly.id,
        &poly.unknown_props,
        poly.source_span,
        diagnostics,
    );
    // polyline is a LEAF: no child-node recursion (points are sub-data).
}

pub(in crate::validate::check) fn check_path(
    path: &PathNode,
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
    register_id(&path.id, seen_ids, diagnostics);
    check_style_ref(
        &path.id,
        path.style.as_deref(),
        declared_style_ids,
        path.source_span,
        diagnostics,
    );

    if !path.subpaths.is_empty() {
        if !path.anchors.is_empty() {
            diagnostics.push(Diagnostic::error(
                "node.invalid_geometry",
                format!(
                    "path '{}': cannot mix direct anchor children with subpath children",
                    path.id
                ),
                path.source_span,
                Some(path.id.clone()),
            ));
        }
        if path.closed.is_some() {
            diagnostics.push(Diagnostic::error(
                "node.invalid_geometry",
                format!(
                    "path '{}': parent closed is invalid when subpath children are present",
                    path.id
                ),
                path.source_span,
                Some(path.id.clone()),
            ));
        }
    }

    let compound = !path.subpaths.is_empty();
    for (subpath_index, subpath) in path.effective_subpaths().enumerate() {
        for (idx, anchor) in subpath.anchors.iter().enumerate() {
            check_path_anchor(&path.id, idx, anchor, path.source_span, diagnostics);
        }

        let required_anchors = if subpath.closed == Some(true) { 3 } else { 2 };
        if subpath.anchors.len() < required_anchors {
            let message = if compound {
                format!(
                    "path '{}': subpath[{}] requires at least {} anchors, got {}",
                    path.id,
                    subpath_index,
                    required_anchors,
                    subpath.anchors.len()
                )
            } else {
                format!(
                    "path '{}': requires at least {} anchors, got {}",
                    path.id,
                    required_anchors,
                    subpath.anchors.len()
                )
            };
            diagnostics.push(Diagnostic::error(
                "shape.insufficient_points",
                message,
                path.source_span,
                Some(path.id.clone()),
            ));
        }
    }

    check_visual_prop(
        &path.id,
        "fill",
        path.fill.as_ref(),
        VisualExpect::ColorOrGradient,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &path.id,
        "stroke",
        path.stroke.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &path.id,
        "stroke-width",
        path.stroke_width.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    if let Some(fr) = &path.fill_rule
        && !matches!(fr.as_str(), "nonzero" | "evenodd")
    {
        diagnostics.push(Diagnostic::warning(
            "node.unknown_property",
            format!(
                "path '{}': unrecognized fill-rule '{}' (version-relative; \
                 allowed values are nonzero, evenodd)",
                path.id, fr
            ),
            path.source_span,
            Some(path.id.clone()),
        ));
    }

    if let Some(sa) = &path.stroke_alignment
        && !matches!(sa.as_str(), "inside" | "center" | "outside")
    {
        diagnostics.push(Diagnostic::warning(
            "node.unknown_property",
            format!(
                "path '{}': unrecognized stroke-alignment '{}' (version-relative; \
                 allowed values are inside, center, outside)",
                path.id, sa
            ),
            path.source_span,
            Some(path.id.clone()),
        ));
    }
    check_stroke_join_props(
        "path",
        &path.id,
        path.stroke_linejoin.as_deref(),
        path.stroke_miter_limit,
        path.source_span,
        diagnostics,
    );
    check_stroke_linecap_prop(
        "path",
        &path.id,
        path.stroke_linecap.as_deref(),
        path.source_span,
        diagnostics,
    );

    check_unknown_props(
        "path",
        &path.id,
        &path.unknown_props,
        path.source_span,
        diagnostics,
    );
}

fn check_path_anchor(
    path_id: &str,
    idx: usize,
    anchor: &PathAnchor,
    source_span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let node_id = Some(path_id.to_owned());
    let x_label = format!("anchor[{idx}].x");
    let y_label = format!("anchor[{idx}].y");
    check_dimension_geom(
        path_id,
        &x_label,
        anchor.x.as_ref(),
        true,
        source_span,
        diagnostics,
    );
    check_dimension_geom(
        path_id,
        &y_label,
        anchor.y.as_ref(),
        true,
        source_span,
        diagnostics,
    );
    check_dimension_geom(
        path_id,
        &format!("anchor[{idx}].in-x"),
        anchor.in_x.as_ref(),
        false,
        source_span,
        diagnostics,
    );
    check_dimension_geom(
        path_id,
        &format!("anchor[{idx}].in-y"),
        anchor.in_y.as_ref(),
        false,
        source_span,
        diagnostics,
    );
    check_dimension_geom(
        path_id,
        &format!("anchor[{idx}].out-x"),
        anchor.out_x.as_ref(),
        false,
        source_span,
        diagnostics,
    );
    check_dimension_geom(
        path_id,
        &format!("anchor[{idx}].out-y"),
        anchor.out_y.as_ref(),
        false,
        source_span,
        diagnostics,
    );

    if let Some(AnchorKind::Unknown(kind)) = &anchor.kind {
        diagnostics.push(Diagnostic::warning(
            "node.unknown_property",
            format!(
                "path '{path_id}': anchor[{idx}] has unrecognized kind '{}' \
                 (version-relative; allowed values are corner, smooth, symmetric)",
                kind
            ),
            source_span,
            node_id.clone(),
        ));
    }

    if anchor.in_x.is_some() != anchor.in_y.is_some() {
        diagnostics.push(Diagnostic::error(
            "node.invalid_geometry",
            format!("path '{path_id}': anchor[{idx}] in handle requires both in-x and in-y"),
            source_span,
            node_id.clone(),
        ));
    }
    if anchor.out_x.is_some() != anchor.out_y.is_some() {
        diagnostics.push(Diagnostic::error(
            "node.invalid_geometry",
            format!("path '{path_id}': anchor[{idx}] out handle requires both out-x and out-y"),
            source_span,
            node_id,
        ));
    }
}

// ── instance validation ───────────────────────────────────────────────────────

/// Validate an `instance` node:
/// - its own `id` participates in GLOBAL uniqueness;
/// - `component` must reference a declared component → else
///   `component.unknown_reference` (Error);
/// - each override `ref` must match a LOCAL descendant id of the referenced
///   component → else `component.unknown_override_target` (Warning).
///
/// The instance is a container-ish node but it does NOT recurse here: its
/// expanded subtree (and the component definition's own ids) are validated at
/// the component definition site, not per-instance, so token/asset refs are
/// checked once.
pub(in crate::validate::check) fn check_instance(
    inst: &InstanceNode,
    ctx: WalkCtx,
    seen_ids: &mut BTreeSet<String>,
    referenced_token_ids: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let WalkCtx {
        resolved_tokens,
        declared_component_ids,
        component_local_ids,
        ..
    } = ctx;
    register_id(&inst.id, seen_ids, diagnostics);

    let has_component = inst.component.is_some();
    let has_source = inst.source.is_some();
    if !has_component && !has_source {
        diagnostics.push(Diagnostic::error(
            "instance.missing_reference",
            format!(
                "instance '{}': exactly one of component or source is required",
                inst.id
            ),
            inst.source_span,
            Some(inst.id.clone()),
        ));
    }
    if has_component && has_source {
        diagnostics.push(Diagnostic::error(
            "instance.multiple_references",
            format!(
                "instance '{}': component and source are mutually exclusive",
                inst.id
            ),
            inst.source_span,
            Some(inst.id.clone()),
        ));
    }

    let component_known = match &inst.component {
        Some(component) => declared_component_ids.contains(component),
        None => false,
    };
    if let Some(component) = &inst.component
        && !component_known
    {
        diagnostics.push(Diagnostic::error(
            "component.unknown_reference",
            format!(
                "instance '{}': references component '{}' which is not declared in the \
                 components block",
                inst.id, component
            ),
            inst.source_span,
            Some(inst.id.clone()),
        ));
    }

    // Override targets are only checkable when the component is known. Look up the
    // referenced component's local-id set; an override `ref` that matches no local
    // descendant id → warning.
    let local_ids = inst
        .component
        .as_ref()
        .and_then(|component| component_local_ids.get(component));
    for ov in &inst.overrides {
        // Validate (and register as referenced) any token refs the override
        // carries, so an override-only token is not falsely flagged unused and
        // a bad override fill/span fill is type-checked like a node fill.
        check_visual_prop(
            &inst.id,
            "fill",
            ov.fill.as_ref(),
            VisualExpect::Color,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
        check_visual_prop(
            &inst.id,
            "svg-stroke",
            ov.svg_stroke.as_ref(),
            VisualExpect::Color,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
        check_visual_prop(
            &inst.id,
            "svg-fill",
            ov.svg_fill.as_ref(),
            VisualExpect::Color,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
        check_visual_prop(
            &inst.id,
            "svg-stroke-width",
            ov.svg_stroke_width.as_ref(),
            VisualExpect::Dimension,
            referenced_token_ids,
            resolved_tokens,
            diagnostics,
        );
        if let Some(spans) = &ov.spans {
            for span in spans {
                check_visual_prop(
                    &inst.id,
                    "fill",
                    span.fill.as_ref(),
                    VisualExpect::Color,
                    referenced_token_ids,
                    resolved_tokens,
                    diagnostics,
                );
            }
        }

        let target_known = local_ids
            .map(|ids| ids.contains(&ov.ref_id))
            .unwrap_or(false);
        if component_known
            && !target_known
            && let Some(component) = &inst.component
        {
            diagnostics.push(Diagnostic::warning(
                "component.unknown_override_target",
                format!(
                    "instance '{}': override ref '{}' matches no descendant id in component '{}'",
                    inst.id, ov.ref_id, component
                ),
                ov.source_span.or(inst.source_span),
                Some(inst.id.clone()),
            ));
        }
    }

    // Unknown properties on the instance node.
    check_unknown_props(
        "instance",
        &inst.id,
        &inst.unknown_props,
        inst.source_span,
        diagnostics,
    );
}

// ── field validation ──────────────────────────────────────────────────────────

/// The known v0 field types.
const KNOWN_FIELD_TYPES: &[&str] = &[
    "running-head",
    "page-number",
    "page-ref",
    "page-count",
    "section-page-number",
    "section-page-count",
    "section-name",
];

/// Validate a `field` node:
/// - its own `id` participates in GLOBAL uniqueness;
/// - `type` must be one of the known field types → else `field.unknown_type`
///   (Warning);
/// - a `page-ref` field whose `target` matches no node id anywhere in the
///   document → `field.unresolved_ref` (Warning);
/// - `style`/`fill`/`font-family`/`font-size` are validated like a text node's,
///   and any token refs are registered so they are not flagged unused.
///
/// A field is a leaf — it does not recurse. Geometry is optional (an absent
/// x/w defaults to the page live area at compile time), so no missing-geometry
/// error is raised here.
pub(in crate::validate::check) fn check_field(
    field: &FieldNode,
    ctx: WalkCtx,
    seen_ids: &mut BTreeSet<String>,
    referenced_token_ids: &mut BTreeSet<String>,
    parent_ctx: AnchorParentCtx,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let WalkCtx {
        resolved_tokens,
        declared_style_ids,
        all_node_ids,
        zone_ids,
        ..
    } = ctx;
    register_id(&field.id, seen_ids, diagnostics);
    check_style_ref(
        &field.id,
        field.style.as_deref(),
        declared_style_ids,
        field.source_span,
        diagnostics,
    );

    // Validate the anchor value (geometry is all-optional for fields anyway).
    check_anchor(
        &field.id,
        AnchorProps {
            anchor: field.anchor.as_deref(),
            anchor_zone: field.anchor_zone.as_deref(),
            anchor_sibling: field.anchor_sibling.as_deref(),
            anchor_parent: field.anchor_parent == Some(true),
            anchor_edge: field.anchor_edge.as_deref(),
            anchor_gap: field.anchor_gap.as_ref(),
        },
        parent_ctx,
        zone_ids,
        field.source_span,
        diagnostics,
    );

    // Unknown field type → Warning (never a hard error; the field simply renders
    // nothing at compile time).
    if !KNOWN_FIELD_TYPES.contains(&field.field_type.as_str()) {
        diagnostics.push(Diagnostic::warning(
            "field.unknown_type",
            format!(
                "field '{}': unknown type '{}'; expected one of {}",
                field.id,
                field.field_type,
                KNOWN_FIELD_TYPES.join(", ")
            ),
            field.source_span,
            Some(field.id.clone()),
        ));
    }

    // A page-ref field with an unresolvable target → Warning. A page-ref with no
    // target at all is also unresolved (nothing to point at).
    if field.field_type == "page-ref" {
        let resolved = field
            .target
            .as_ref()
            .map(|t| all_node_ids.contains(t))
            .unwrap_or(false);
        if !resolved {
            diagnostics.push(Diagnostic::warning(
                "field.unresolved_ref",
                format!(
                    "field '{}': page-ref target {} matches no node id in the document",
                    field.id,
                    field
                        .target
                        .as_deref()
                        .map(|t| format!("'{t}'"))
                        .unwrap_or_else(|| "(absent)".to_owned())
                ),
                field.source_span,
                Some(field.id.clone()),
            ));
        }
    }

    // Visual properties (mirror the text-node checks).
    check_visual_prop(
        &field.id,
        "fill",
        field.fill.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &field.id,
        "font-family",
        field.font_family.as_ref(),
        VisualExpect::FontFamily,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &field.id,
        "font-size",
        field.font_size.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    // Unknown properties on the field node.
    check_unknown_props(
        "field",
        &field.id,
        &field.unknown_props,
        field.source_span,
        diagnostics,
    );
}

/// Validate a [`TocNode`]: id uniqueness, style ref, visual properties, and
/// the `toc.no_selector` advisory when both `match_role` and `match_style` are
/// absent (the toc would collect no entries at compile time).
pub(in crate::validate::check) fn check_toc(
    toc: &TocNode,
    ctx: WalkCtx,
    seen_ids: &mut BTreeSet<String>,
    referenced_token_ids: &mut BTreeSet<String>,
    parent_ctx: AnchorParentCtx,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let WalkCtx {
        resolved_tokens,
        declared_style_ids,
        zone_ids,
        ..
    } = ctx;
    register_id(&toc.id, seen_ids, diagnostics);
    check_style_ref(
        &toc.id,
        toc.style.as_deref(),
        declared_style_ids,
        toc.source_span,
        diagnostics,
    );

    // Validate the anchor value (geometry is all-optional for toc anyway).
    check_anchor(
        &toc.id,
        AnchorProps {
            anchor: toc.anchor.as_deref(),
            anchor_zone: toc.anchor_zone.as_deref(),
            anchor_sibling: toc.anchor_sibling.as_deref(),
            anchor_parent: toc.anchor_parent == Some(true),
            anchor_edge: toc.anchor_edge.as_deref(),
            anchor_gap: toc.anchor_gap.as_ref(),
        },
        parent_ctx,
        zone_ids,
        toc.source_span,
        diagnostics,
    );

    // Warn when neither selector is set: the toc will collect no entries.
    if toc.match_role.is_none() && toc.match_style.is_none() {
        diagnostics.push(Diagnostic::warning(
            "toc.no_selector",
            format!(
                "toc '{}' has neither match-role nor match-style; it will collect no entries",
                toc.id
            ),
            toc.source_span,
            Some(toc.id.clone()),
        ));
    }

    // Visual properties (mirror the field-node checks).
    check_visual_prop(
        &toc.id,
        "fill",
        toc.fill.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &toc.id,
        "font-family",
        toc.font_family.as_ref(),
        VisualExpect::FontFamily,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &toc.id,
        "font-size",
        toc.font_size.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    // Unknown properties on the toc node.
    check_unknown_props(
        "toc",
        &toc.id,
        &toc.unknown_props,
        toc.source_span,
        diagnostics,
    );
}

/// Validate a [`FootnoteNode`]: id uniqueness, style ref, the content-span and
/// node visual properties (fill/font-family/font-size, plus per-span fill/weight
/// so raw visual literals are surfaced like any text), and unknown properties.
///
/// The structural `footnote.unresolved_ref` check (a span `footnote-ref` that
/// names no footnote on the same page) is done at the PAGE level (it needs the
/// page's footnote-id set), not here. A footnote has no geometry, so there are
/// no geometry checks.
pub(in crate::validate::check) fn check_footnote(
    footnote: &FootnoteNode,
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
    register_id(&footnote.id, seen_ids, diagnostics);
    check_style_ref(
        &footnote.id,
        footnote.style.as_deref(),
        declared_style_ids,
        footnote.source_span,
        diagnostics,
    );

    check_visual_prop(
        &footnote.id,
        "fill",
        footnote.fill.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &footnote.id,
        "font-family",
        footnote.font_family.as_ref(),
        VisualExpect::FontFamily,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &footnote.id,
        "font-size",
        footnote.font_size.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    // Per-span visual props (mirror the text-node span checks) so token refs are
    // registered and raw visual literals are flagged `token.raw_visual_literal`.
    check_spans(
        &footnote.id,
        &footnote.spans,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    check_unknown_props(
        "footnote",
        &footnote.id,
        &footnote.unknown_props,
        footnote.source_span,
        diagnostics,
    );
}
