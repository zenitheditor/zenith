//! `line`, `polygon`, and `polyline` leaf-node compilation, plus the shared
//! flat-point resolution / centroid helpers reused by the connector compiler.

use std::collections::BTreeMap;

use zenith_core::{
    Diagnostic, LineNode, Point, PolygonNode, PolylineNode, ResolvedToken, Span, Style, dim_to_px,
};

use crate::ir::{SceneCommand, StrokeAlign};

use super::super::RenderCtx;
use super::super::paint::resolve_property_color;
use super::super::style_prop;
use super::super::util::{resolve_property_dimension_px, rotation_degrees, unsupported_unit_diag};
use super::common::resolve_dash_params;

/// Compile a `line` leaf node.
pub(in crate::compile) fn compile_line(
    line: &LineNode,
    resolved: &BTreeMap<String, ResolvedToken>,
    style_map: &BTreeMap<&str, &Style>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) {
    // Skip invisible lines.
    if line.visible == Some(false) {
        return;
    }

    // Require all four endpoints; skip if any is absent or bad unit.
    let (Some(x1d), Some(y1d), Some(x2d), Some(y2d)) = (&line.x1, &line.y1, &line.x2, &line.y2)
    else {
        diagnostics.push(Diagnostic::advisory(
            "scene.missing_geometry",
            format!(
                "line '{}' is missing one or more endpoint properties (x1, y1, x2, y2); \
                 skipped",
                line.id
            ),
            line.source_span,
            Some(line.id.clone()),
        ));
        return;
    };

    let Some(x1_raw) = dim_to_px(x1d.value, &x1d.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "line",
            &line.id,
            "x1",
            line.source_span,
        ));
        return;
    };
    let Some(y1_raw) = dim_to_px(y1d.value, &y1d.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "line",
            &line.id,
            "y1",
            line.source_span,
        ));
        return;
    };
    let Some(x2_raw) = dim_to_px(x2d.value, &x2d.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "line",
            &line.id,
            "x2",
            line.source_span,
        ));
        return;
    };
    let Some(y2_raw) = dim_to_px(y2d.value, &y2d.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "line",
            &line.id,
            "y2",
            line.source_span,
        ));
        return;
    };

    // Apply group translation offset.
    let x1 = x1_raw + ctx.dx;
    let y1 = y1_raw + ctx.dy;
    let x2 = x2_raw + ctx.dx;
    let y2 = y2_raw + ctx.dy;

    // Stroke is optional in validation, but a stroke-less line draws nothing.
    // Cascade: node-local stroke overrides style stroke.
    let stroke_prop = line
        .stroke
        .as_ref()
        .or_else(|| style_prop(&line.style, style_map, "stroke"));
    let Some(stroke_prop) = stroke_prop else {
        return;
    };
    let Some(mut color) = resolve_property_color(stroke_prop, resolved, diagnostics, &line.id)
    else {
        return;
    };

    // Apply node opacity then cascade ctx.opacity on top.
    let node_opacity = line.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
    color.a = (color.a as f64 * node_opacity * ctx.opacity).round() as u8;

    // Resolve stroke_width to px with style cascade; default 1.0 when absent.
    let sw = line
        .stroke_width
        .clone()
        .or_else(|| style_prop(&line.style, style_map, "stroke-width").cloned());
    let stroke_width: f64 = resolve_property_dimension_px(&sw, resolved, 1.0);

    // Resolve dashed stroke parameters.
    let (stroke_dash, stroke_gap, stroke_linecap) = resolve_dash_params(
        &line.stroke_dash,
        &line.stroke_gap,
        line.stroke_linecap.as_deref(),
        resolved,
    );

    commands.push(SceneCommand::StrokeLine {
        x1,
        y1,
        x2,
        y2,
        color,
        stroke_width,
        stroke_dash,
        stroke_gap,
        stroke_linecap,
    });
}

/// Resolve an ordered point list into a flat `[x0, y0, x1, y1, …]` pixel-
/// coordinate vector, applying `ctx.dx`/`ctx.dy`.
///
/// Returns `None` on the first point with a missing or unsupported-unit
/// coordinate, after pushing a diagnostic. The minimum-count check is the
/// caller's responsibility (polygon requires ≥ 6 coords, polyline ≥ 4).
fn resolve_flat_points(
    points: &[Point],
    node_kind: &str,
    node_id: &str,
    source_span: Option<Span>,
    ctx: RenderCtx,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<f64>> {
    let mut flat: Vec<f64> = Vec::with_capacity(points.len() * 2);
    for (idx, pt) in points.iter().enumerate() {
        let (Some(xd), Some(yd)) = (&pt.x, &pt.y) else {
            diagnostics.push(Diagnostic::advisory(
                "scene.missing_geometry",
                format!(
                    "{} '{}' point[{}] is missing x or y coordinate; skipped",
                    node_kind, node_id, idx
                ),
                source_span,
                Some(node_id.to_owned()),
            ));
            return None;
        };
        let Some(px) = dim_to_px(xd.value, &xd.unit) else {
            diagnostics.push(unsupported_unit_diag(
                node_kind,
                node_id,
                "point x",
                source_span,
            ));
            return None;
        };
        let Some(py) = dim_to_px(yd.value, &yd.unit) else {
            diagnostics.push(unsupported_unit_diag(
                node_kind,
                node_id,
                "point y",
                source_span,
            ));
            return None;
        };
        flat.push(px + ctx.dx);
        flat.push(py + ctx.dy);
    }
    Some(flat)
}

/// Compile a `polygon` leaf node.
///
/// Emits `FillPolygon` (if fill is present) THEN `StrokePolyline { closed: true }`
/// (if stroke is present) so the stroke draws on top of the fill.
///
/// Points are in absolute document coordinates — `ctx.dx`/`ctx.dy` are added
/// exactly as for `line` endpoints.
pub(in crate::compile) fn compile_polygon(
    poly: &PolygonNode,
    resolved: &BTreeMap<String, ResolvedToken>,
    style_map: &BTreeMap<&str, &Style>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) {
    if poly.visible == Some(false) {
        return;
    }

    // Build the flat point list: require both x and y for every point.
    let Some(flat_points) = resolve_flat_points(
        &poly.points,
        "polygon",
        &poly.id,
        poly.source_span,
        ctx,
        diagnostics,
    ) else {
        return;
    };

    // Need at least 3 points (6 coordinates) — validate already errors, skip emit.
    if flat_points.len() < 6 {
        return;
    }

    let node_opacity = poly.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
    let even_odd = poly.fill_rule.as_deref() == Some("evenodd");

    // Rotation bracket: compute centroid-bbox center from the flat point vec.
    // PushTransform only when rotate is non-zero; unrotated polys are unchanged.
    let rot = rotation_degrees(poly.rotate.as_ref());
    if let Some(angle) = rot {
        let (cx, cy) = flat_points_centroid_center(&flat_points);
        commands.push(SceneCommand::PushTransform {
            angle_deg: angle,
            cx,
            cy,
        });
    }

    // FILL (drawn first, stroke on top) — node-local overrides style cascade.
    let fill_prop = poly
        .fill
        .as_ref()
        .or_else(|| style_prop(&poly.style, style_map, "fill"));
    if let Some(fill_prop) = fill_prop
        && let Some(mut color) = resolve_property_color(fill_prop, resolved, diagnostics, &poly.id)
    {
        color.a = (color.a as f64 * node_opacity * ctx.opacity).round() as u8;
        commands.push(SceneCommand::FillPolygon {
            points: flat_points.clone(),
            color,
            even_odd,
        });
    }

    // STROKE (drawn on top of fill) — node-local overrides style cascade.
    let stroke_prop = poly
        .stroke
        .as_ref()
        .or_else(|| style_prop(&poly.style, style_map, "stroke"));
    if let Some(stroke_prop) = stroke_prop
        && let Some(mut color) =
            resolve_property_color(stroke_prop, resolved, diagnostics, &poly.id)
    {
        color.a = (color.a as f64 * node_opacity * ctx.opacity).round() as u8;
        let sw = poly
            .stroke_width
            .clone()
            .or_else(|| style_prop(&poly.style, style_map, "stroke-width").cloned());
        let stroke_width = resolve_property_dimension_px(&sw, resolved, 1.0);
        // stroke-alignment: "inside"/"outside" shift the stroke off the path
        // boundary; anything else (incl. "center", None, or an invalid value)
        // falls back to Center. Validation emits the warning for bad values.
        let align = match poly.stroke_alignment.as_deref() {
            Some("inside") => StrokeAlign::Inside,
            Some("outside") => StrokeAlign::Outside,
            _ => StrokeAlign::Center,
        };
        commands.push(SceneCommand::StrokePolyline {
            points: flat_points,
            color,
            stroke_width,
            closed: true,
            align,
            fill_even_odd: even_odd,
        });
    }

    if rot.is_some() {
        commands.push(SceneCommand::PopTransform);
    }
}

/// Compile a `polyline` leaf node.
///
/// Emits `FillPolygon` (if fill is present, renderer closes the path implicitly)
/// THEN `StrokePolyline { closed: false }` (if stroke is present).
///
/// Points are in absolute document coordinates — `ctx.dx`/`ctx.dy` are added
/// exactly as for `line` endpoints.
pub(in crate::compile) fn compile_polyline(
    poly: &PolylineNode,
    resolved: &BTreeMap<String, ResolvedToken>,
    style_map: &BTreeMap<&str, &Style>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) {
    if poly.visible == Some(false) {
        return;
    }

    // Build the flat point list.
    let Some(flat_points) = resolve_flat_points(
        &poly.points,
        "polyline",
        &poly.id,
        poly.source_span,
        ctx,
        diagnostics,
    ) else {
        return;
    };

    // Need at least 2 points (4 coordinates) — validate already errors, skip emit.
    if flat_points.len() < 4 {
        return;
    }

    let node_opacity = poly.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
    let even_odd = poly.fill_rule.as_deref() == Some("evenodd");

    // Rotation bracket: compute centroid-bbox center from the flat point vec.
    // PushTransform only when rotate is non-zero; unrotated polylines are unchanged.
    let rot = rotation_degrees(poly.rotate.as_ref());
    if let Some(angle) = rot {
        let (cx, cy) = flat_points_centroid_center(&flat_points);
        commands.push(SceneCommand::PushTransform {
            angle_deg: angle,
            cx,
            cy,
        });
    }

    // FILL (drawn first; FillPolygon renderer closes the path) — style cascade.
    let fill_prop = poly
        .fill
        .as_ref()
        .or_else(|| style_prop(&poly.style, style_map, "fill"));
    if let Some(fill_prop) = fill_prop
        && let Some(mut color) = resolve_property_color(fill_prop, resolved, diagnostics, &poly.id)
    {
        color.a = (color.a as f64 * node_opacity * ctx.opacity).round() as u8;
        commands.push(SceneCommand::FillPolygon {
            points: flat_points.clone(),
            color,
            even_odd,
        });
    }

    // STROKE — open path (closed: false) — style cascade.
    let stroke_prop = poly
        .stroke
        .as_ref()
        .or_else(|| style_prop(&poly.style, style_map, "stroke"));
    if let Some(stroke_prop) = stroke_prop
        && let Some(mut color) =
            resolve_property_color(stroke_prop, resolved, diagnostics, &poly.id)
    {
        color.a = (color.a as f64 * node_opacity * ctx.opacity).round() as u8;
        let sw = poly
            .stroke_width
            .clone()
            .or_else(|| style_prop(&poly.style, style_map, "stroke-width").cloned());
        let stroke_width = resolve_property_dimension_px(&sw, resolved, 1.0);
        commands.push(SceneCommand::StrokePolyline {
            points: flat_points,
            color,
            stroke_width,
            closed: false,
            // polyline is an open path: alignment never applies.
            align: StrokeAlign::Center,
            fill_even_odd: false,
        });
    }

    if rot.is_some() {
        commands.push(SceneCommand::PopTransform);
    }
}

/// Compute the center of the bounding box of a flat `[x0, y0, x1, y1, …]` point list.
///
/// Used to determine the rotation pivot for polygon/polyline/connector nodes. The
/// slice must be non-empty and even-length (guaranteed by the callers). If the
/// slice is somehow empty, returns `(0.0, 0.0)` as a safe degenerate fallback
/// (the no-panic contract requires this instead of indexing).
pub(super) fn flat_points_centroid_center(flat: &[f64]) -> (f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    // `chunks_exact(2)` yields each (x, y) pair and ignores any trailing odd
    // element — byte-identical to the prior `while i + 1 < len` step-by-2 loop,
    // but with no unchecked indexing.
    for pair in flat.chunks_exact(2) {
        let &[px, py] = pair else { continue };
        if px < min_x {
            min_x = px;
        }
        if px > max_x {
            max_x = px;
        }
        if py < min_y {
            min_y = py;
        }
        if py > max_y {
            max_y = py;
        }
    }
    if min_x.is_infinite() {
        return (0.0, 0.0);
    }
    ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0)
}
