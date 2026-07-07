//! `line`, `polygon`, and `polyline` leaf-node compilation, plus the shared
//! flat-point resolution / centroid helpers reused by the connector compiler.

use std::collections::BTreeMap;

use zenith_core::{
    Diagnostic, Dimension, LineNode, PathAnchor, PathNode, Point, PolygonNode, PolylineNode,
    ResolvedToken, Span, Style, dim_to_px,
};

use crate::ir::{Paint, PathSegment, SceneCommand, StrokeAlign};

use super::super::RenderCtx;
use super::super::paint::{
    apply_gradient_opacity, resolve_property_color, resolve_property_gradient,
};
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
    let stroke_width: f64 = resolve_property_dimension_px(sw.as_ref(), resolved, 1.0);

    // Resolve dashed stroke parameters.
    let (stroke_dash, stroke_gap, stroke_linecap) = resolve_dash_params(
        line.stroke_dash.as_ref(),
        line.stroke_gap.as_ref(),
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

fn resolve_path_anchor_point(
    anchor: &PathAnchor,
    node_id: &str,
    idx: usize,
    source_span: Option<Span>,
    ctx: RenderCtx,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(f64, f64)> {
    let (Some(xd), Some(yd)) = (&anchor.x, &anchor.y) else {
        diagnostics.push(Diagnostic::advisory(
            "scene.missing_geometry",
            format!(
                "path '{}' anchor[{}] is missing x or y coordinate; skipped",
                node_id, idx
            ),
            source_span,
            Some(node_id.to_owned()),
        ));
        return None;
    };
    let Some(px) = dim_to_px(xd.value, &xd.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "path",
            node_id,
            "anchor x",
            source_span,
        ));
        return None;
    };
    let Some(py) = dim_to_px(yd.value, &yd.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "path",
            node_id,
            "anchor y",
            source_span,
        ));
        return None;
    };
    Some((px + ctx.dx, py + ctx.dy))
}

struct PathBuildCtx<'a, 'b> {
    node_id: &'a str,
    source_span: Option<Span>,
    render: RenderCtx,
    diagnostics: &'b mut Vec<Diagnostic>,
}

struct PathHandleInput<'a> {
    x: &'a Option<Dimension>,
    y: &'a Option<Dimension>,
    fallback: (f64, f64),
    label: &'static str,
}

struct PathEdgeInput<'a> {
    prev_anchor: &'a PathAnchor,
    prev_point: (f64, f64),
    next_anchor: &'a PathAnchor,
    next_point: (f64, f64),
}

struct ResolvedPathSegments {
    segments: Vec<PathSegment>,
    anchor_points: Vec<(f64, f64)>,
    closed: bool,
}

fn resolve_path_handle_point(
    input: PathHandleInput<'_>,
    ctx: &mut PathBuildCtx<'_, '_>,
) -> Option<(f64, f64)> {
    let PathHandleInput {
        x,
        y,
        fallback,
        label,
    } = input;
    let (Some(xd), Some(yd)) = (x, y) else {
        return Some(fallback);
    };
    let Some(px) = dim_to_px(xd.value, &xd.unit) else {
        ctx.diagnostics.push(unsupported_unit_diag(
            "path",
            ctx.node_id,
            label,
            ctx.source_span,
        ));
        return None;
    };
    let Some(py) = dim_to_px(yd.value, &yd.unit) else {
        ctx.diagnostics.push(unsupported_unit_diag(
            "path",
            ctx.node_id,
            label,
            ctx.source_span,
        ));
        return None;
    };
    Some((px + ctx.render.dx, py + ctx.render.dy))
}

fn path_anchor_bbox_center(points: &[(f64, f64)]) -> (f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for &(x, y) in points {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    if min_x.is_infinite() {
        return (0.0, 0.0);
    }
    ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0)
}

fn push_path_segment(
    segments: &mut Vec<PathSegment>,
    edge: PathEdgeInput<'_>,
    ctx: &mut PathBuildCtx<'_, '_>,
) -> Option<()> {
    let PathEdgeInput {
        prev_anchor,
        prev_point,
        next_anchor,
        next_point,
    } = edge;
    let has_prev_out = prev_anchor.out_x.is_some() || prev_anchor.out_y.is_some();
    let has_next_in = next_anchor.in_x.is_some() || next_anchor.in_y.is_some();
    if !has_prev_out && !has_next_in {
        segments.push(PathSegment::LineTo {
            x: next_point.0,
            y: next_point.1,
        });
        return Some(());
    }

    let c1 = resolve_path_handle_point(
        PathHandleInput {
            x: &prev_anchor.out_x,
            y: &prev_anchor.out_y,
            fallback: prev_point,
            label: "out handle",
        },
        ctx,
    )?;
    let c2 = resolve_path_handle_point(
        PathHandleInput {
            x: &next_anchor.in_x,
            y: &next_anchor.in_y,
            fallback: next_point,
            label: "in handle",
        },
        ctx,
    )?;
    segments.push(PathSegment::CubicTo {
        x1: c1.0,
        y1: c1.1,
        x2: c2.0,
        y2: c2.1,
        x: next_point.0,
        y: next_point.1,
    });
    Some(())
}

fn resolve_path_segments(
    path: &PathNode,
    ctx: RenderCtx,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ResolvedPathSegments> {
    let closed = path.closed == Some(true);
    let mut build_ctx = PathBuildCtx {
        node_id: &path.id,
        source_span: path.source_span,
        render: ctx,
        diagnostics,
    };
    let mut points = Vec::with_capacity(path.anchors.len());
    for (idx, anchor) in path.anchors.iter().enumerate() {
        points.push(resolve_path_anchor_point(
            anchor,
            &path.id,
            idx,
            path.source_span,
            ctx,
            build_ctx.diagnostics,
        )?);
    }

    let first = points.first().copied()?;
    let mut segments = Vec::with_capacity(path.anchors.len() + 2);
    segments.push(PathSegment::MoveTo {
        x: first.0,
        y: first.1,
    });

    for pair in path.anchors.windows(2).zip(points.windows(2)) {
        let (anchor_pair, point_pair) = pair;
        let (Some(prev_anchor), Some(next_anchor)) = (anchor_pair.first(), anchor_pair.get(1))
        else {
            return None;
        };
        let (Some(&prev_point), Some(&next_point)) = (point_pair.first(), point_pair.get(1)) else {
            return None;
        };
        push_path_segment(
            &mut segments,
            PathEdgeInput {
                prev_anchor,
                prev_point,
                next_anchor,
                next_point,
            },
            &mut build_ctx,
        )?;
    }

    if closed
        && let (Some(prev_anchor), Some(first_anchor), Some(&prev_point)) =
            (path.anchors.last(), path.anchors.first(), points.last())
    {
        push_path_segment(
            &mut segments,
            PathEdgeInput {
                prev_anchor,
                prev_point,
                next_anchor: first_anchor,
                next_point: first,
            },
            &mut build_ctx,
        )?;
        segments.push(PathSegment::Close);
    }

    Some(ResolvedPathSegments {
        segments,
        anchor_points: points,
        closed,
    })
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
    // A gradient fill resolves over the polygon's bounding box at raster time;
    // a color fill bakes in the node + ancestor opacity.
    let fill_prop = poly
        .fill
        .as_ref()
        .or_else(|| style_prop(&poly.style, style_map, "fill"));
    if let Some(fill_prop) = fill_prop {
        let fill_op = node_opacity * ctx.opacity;
        if let Some(mut gradient) = resolve_property_gradient(fill_prop, resolved, &poly.id) {
            apply_gradient_opacity(&mut gradient, fill_op, 1.0);
            commands.push(SceneCommand::FillPolygon {
                points: flat_points.clone(),
                paint: Paint::Gradient(gradient),
                even_odd,
            });
        } else if let Some(mut color) =
            resolve_property_color(fill_prop, resolved, diagnostics, &poly.id)
        {
            color.a = (color.a as f64 * fill_op).round() as u8;
            commands.push(SceneCommand::FillPolygon {
                points: flat_points.clone(),
                paint: Paint::solid(color),
                even_odd,
            });
        }
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
        let stroke_width = resolve_property_dimension_px(sw.as_ref(), resolved, 1.0);
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
    if let Some(fill_prop) = fill_prop {
        let fill_op = node_opacity * ctx.opacity;
        if let Some(mut gradient) = resolve_property_gradient(fill_prop, resolved, &poly.id) {
            apply_gradient_opacity(&mut gradient, fill_op, 1.0);
            commands.push(SceneCommand::FillPolygon {
                points: flat_points.clone(),
                paint: Paint::Gradient(gradient),
                even_odd,
            });
        } else if let Some(mut color) =
            resolve_property_color(fill_prop, resolved, diagnostics, &poly.id)
        {
            color.a = (color.a as f64 * fill_op).round() as u8;
            commands.push(SceneCommand::FillPolygon {
                points: flat_points.clone(),
                paint: Paint::solid(color),
                even_odd,
            });
        }
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
        let stroke_width = resolve_property_dimension_px(sw.as_ref(), resolved, 1.0);
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

/// Compile a structured cubic Bezier `path` leaf node.
pub(in crate::compile) fn compile_path(
    path: &PathNode,
    resolved: &BTreeMap<String, ResolvedToken>,
    style_map: &BTreeMap<&str, &Style>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) {
    if path.visible == Some(false) {
        return;
    }

    let Some(resolved_path) = resolve_path_segments(path, ctx, diagnostics) else {
        return;
    };
    let ResolvedPathSegments {
        segments,
        anchor_points,
        closed,
    } = resolved_path;
    if anchor_points.len() < 2 {
        return;
    }

    let node_opacity = path.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
    let even_odd = path.fill_rule.as_deref() == Some("evenodd");

    let rot = rotation_degrees(path.rotate.as_ref());
    if let Some(angle) = rot {
        let (cx, cy) = path_anchor_bbox_center(&anchor_points);
        commands.push(SceneCommand::PushTransform {
            angle_deg: angle,
            cx,
            cy,
        });
    }

    let fill_prop = path
        .fill
        .as_ref()
        .or_else(|| style_prop(&path.style, style_map, "fill"));
    if anchor_points.len() >= 3
        && let Some(fill_prop) = fill_prop
    {
        let fill_op = node_opacity * ctx.opacity;
        if let Some(mut gradient) = resolve_property_gradient(fill_prop, resolved, &path.id) {
            apply_gradient_opacity(&mut gradient, fill_op, 1.0);
            commands.push(SceneCommand::FillPath {
                segments: segments.clone(),
                paint: Paint::Gradient(gradient),
                even_odd,
            });
        } else if let Some(mut color) =
            resolve_property_color(fill_prop, resolved, diagnostics, &path.id)
        {
            color.a = (color.a as f64 * fill_op).round() as u8;
            commands.push(SceneCommand::FillPath {
                segments: segments.clone(),
                paint: Paint::solid(color),
                even_odd,
            });
        }
    }

    let stroke_prop = path
        .stroke
        .as_ref()
        .or_else(|| style_prop(&path.style, style_map, "stroke"));
    if let Some(stroke_prop) = stroke_prop
        && let Some(mut color) =
            resolve_property_color(stroke_prop, resolved, diagnostics, &path.id)
    {
        color.a = (color.a as f64 * node_opacity * ctx.opacity).round() as u8;
        let sw = path
            .stroke_width
            .clone()
            .or_else(|| style_prop(&path.style, style_map, "stroke-width").cloned());
        let stroke_width = resolve_property_dimension_px(sw.as_ref(), resolved, 1.0);
        let align = match path.stroke_alignment.as_deref() {
            Some("inside") => StrokeAlign::Inside,
            Some("outside") => StrokeAlign::Outside,
            _ => StrokeAlign::Center,
        };
        commands.push(SceneCommand::StrokePath {
            segments,
            color,
            stroke_width,
            closed,
            align,
            fill_even_odd: even_odd,
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
