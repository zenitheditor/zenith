//! Per-kind checks for the point-based leaf nodes: `polygon`, `polyline`,
//! `path` (plus the shared per-anchor geometry check).

use std::collections::BTreeSet;

use crate::ast::node::{AnchorKind, PathAnchor, PathNode, PolygonNode, PolylineNode};
use crate::diagnostics::Diagnostic;

use crate::validate::check::nodes::WalkCtx;
use crate::validate::check::nodes::node::shared::{
    check_dimension_geom, check_stroke_join_props, check_stroke_linecap_prop, check_style_ref,
};
use crate::validate::check::nodes::node::suggest::check_unknown_props;
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
