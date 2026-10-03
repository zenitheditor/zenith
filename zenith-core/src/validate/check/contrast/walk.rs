//! The paint-order walk: collects backdrop candidates in document order and
//! hands each `text` node, label, and table to its judge.

use std::collections::BTreeMap;

use crate::ast::node::{ImageNode, Node, PathNode, Point, PolygonNode, PolylineNode, ShapeNode};
use crate::ast::value::PropertyValue;
use crate::diagnostics::Diagnostic;
use crate::tokens::ResolvedToken;

use super::geometry::{
    CoverageShape, RectPx, group_offset, local_box, path_fill_region, polygon_region,
};
use super::label::check_label;
use super::paint::resolve_fill_paint;
use super::props::{
    candidate_has_effect, clip_bounds, container_is_unmodeled, frame_coverage_shape, leaf_rotation,
    node_rotate_deg, rect_coverage_shape,
};
use super::table::check_table_text_contrast;
use super::text::check_text_node;
use super::types::{BackdropCandidate, BackdropPaint, ContrastEnv, MIN_PAINT_ALPHA, PaintCtx};

pub(super) fn walk_paint(
    children: &[Node],
    ctx: PaintCtx<'_>,
    candidates: &mut Vec<BackdropCandidate>,
    env: ContrastEnv<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for node in children {
        if !node.is_visible() {
            continue;
        }
        match node {
            Node::Rect(r) => push_backdrop(
                node,
                &r.fill,
                &r.style,
                rect_coverage_shape(r, ctx.page_size, env.resolved_tokens),
                ctx,
                candidates,
                env,
            ),
            Node::Ellipse(e) => push_backdrop(
                node,
                &e.fill,
                &e.style,
                CoverageShape::Ellipse,
                ctx,
                candidates,
                env,
            ),
            Node::Shape(s) => {
                push_shape_backdrop(node, s, ctx, candidates, env);
                check_label(node, ctx, candidates, env, diagnostics);
            }
            Node::Connector(_) => check_label(node, ctx, candidates, env, diagnostics),
            Node::Image(img) => push_image_backdrop(node, img, ctx, candidates, env),
            Node::Polygon(poly) => push_polygon_backdrop(node, poly, ctx, candidates, env),
            Node::Polyline(poly) => push_polyline_backdrop(node, poly, ctx, candidates, env),
            Node::Frame(f) => {
                let clips = f.clips();
                let frame_box = absolute_box(node, ctx, env.resolved_tokens);
                let frame_clip = match frame_box {
                    // A frame clipped out entirely hides its children: an
                    // empty clip keeps them out of sampling.
                    Some(b) if clips => clip_bounds(ctx.clip, b).or(Some(RectPx {
                        x: b.x,
                        y: b.y,
                        w: 0.0,
                        h: 0.0,
                    })),
                    Some(_) | None => ctx.clip,
                };
                // The frame fill paints under its children, so it is a backdrop
                // before any child candidate.
                push_backdrop(
                    node,
                    &f.fill,
                    &f.style,
                    frame_coverage_shape(f, ctx.page_size, env.resolved_tokens),
                    ctx,
                    candidates,
                    env,
                );
                let child_ctx = PaintCtx {
                    clip: frame_clip,
                    opacity: cascaded_opacity(ctx.opacity, f.opacity),
                    unmodeled: ctx.unmodeled || container_is_unmodeled(node),
                    header_style: None,
                    ..ctx
                };
                walk_paint(&f.children, child_ctx, candidates, env, diagnostics);
            }
            Node::Group(g) => {
                let scope = env.scopes.and_then(|scopes| scopes.get(&g.id));
                // A scoped group's `x` / `y` resolve in the enclosing scope.
                let (ox, oy, sx, sy) = match scope.and_then(|s| s.fit) {
                    Some(fit) => (fit.tx, fit.ty, fit.sx, fit.sy),
                    None => {
                        let (gx, gy) = group_offset(
                            g.x.as_ref(),
                            g.y.as_ref(),
                            ctx.page_size,
                            env.resolved_tokens,
                        );
                        (gx, gy, 1.0, 1.0)
                    }
                };
                let (dx, dy) = ctx.place().point(ox, oy);
                let child_ctx = PaintCtx {
                    dx,
                    dy,
                    sx: ctx.sx * sx,
                    sy: ctx.sy * sy,
                    opacity: cascaded_opacity(ctx.opacity, g.opacity),
                    unmodeled: ctx.unmodeled || container_is_unmodeled(node),
                    header_style: None,
                    ..ctx
                };
                let child_env = match scope.and_then(|s| s.tokens) {
                    Some(tokens) => ContrastEnv {
                        resolved_tokens: tokens.resolved,
                        style_map: tokens.styles,
                        ..env
                    },
                    None => env,
                };
                walk_paint(&g.children, child_ctx, candidates, child_env, diagnostics);
            }
            Node::Text(t) => check_text_node(t, ctx, candidates, env, diagnostics),
            Node::Table(t) => {
                check_table_text_contrast(t, ctx.page_bg_rgb, ctx.page_size, env, diagnostics);
            }
            Node::Path(p) => push_path_backdrop(node, p, ctx, candidates, env),
            Node::Line(_)
            | Node::Code(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Footnote(_)
            | Node::Toc(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => {}
        }
    }
}

fn push_shape_backdrop(
    node: &Node,
    shape: &ShapeNode,
    ctx: PaintCtx<'_>,
    candidates: &mut Vec<BackdropCandidate>,
    env: ContrastEnv<'_>,
) {
    let coverage = match shape.kind.as_deref() {
        Some("decision") => CoverageShape::Diamond,
        Some("terminator") => CoverageShape::Capsule,
        Some("ellipse") => CoverageShape::Ellipse,
        Some("process") | None => CoverageShape::Rect,
        _ => CoverageShape::Rect,
    };
    push_backdrop(
        node,
        &shape.fill,
        &shape.style,
        coverage,
        ctx,
        candidates,
        env,
    );
}

fn push_polygon_backdrop(
    node: &Node,
    polygon: &PolygonNode,
    ctx: PaintCtx<'_>,
    candidates: &mut Vec<BackdropCandidate>,
    env: ContrastEnv<'_>,
) {
    push_point_backdrop(
        node,
        &polygon.fill,
        &polygon.style,
        &polygon.points,
        ctx,
        candidates,
        env,
    );
}

fn push_polyline_backdrop(
    node: &Node,
    polyline: &PolylineNode,
    ctx: PaintCtx<'_>,
    candidates: &mut Vec<BackdropCandidate>,
    env: ContrastEnv<'_>,
) {
    push_point_backdrop(
        node,
        &polyline.fill,
        &polyline.style,
        &polyline.points,
        ctx,
        candidates,
        env,
    );
}

fn push_point_backdrop(
    node: &Node,
    fill: &Option<PropertyValue>,
    style: &Option<String>,
    points: &[Point],
    ctx: PaintCtx<'_>,
    candidates: &mut Vec<BackdropCandidate>,
    env: ContrastEnv<'_>,
) {
    let opacity = ctx.opacity * node.opacity().unwrap_or(1.0);
    let Some(paint) = resolve_fill_paint(
        fill,
        style.as_deref(),
        env.style_map,
        env.resolved_tokens,
        opacity,
    ) else {
        return;
    };
    let Some((bounds, shape)) = polygon_region(points, ctx.place(), ctx.page_size) else {
        return;
    };
    // A rotated polygon/polyline pivots on its centroid box (not its bounding-box
    // center), which the validator does not replicate — so any rotation makes the
    // backdrop indeterminate rather than silently mis-testing containment.
    let paint = if ctx.unmodeled || node_rotate_deg(node).is_some() {
        BackdropPaint::Indeterminate
    } else {
        paint
    };
    if let Some(bounds) = clip_bounds(ctx.clip, bounds) {
        candidates.push(BackdropCandidate {
            paint,
            bounds,
            shape,
            rotation: None,
        });
    }
}

fn push_path_backdrop(
    node: &Node,
    path: &PathNode,
    ctx: PaintCtx<'_>,
    candidates: &mut Vec<BackdropCandidate>,
    env: ContrastEnv<'_>,
) {
    // Solid path fills resolve exact compound fill coverage (doc 24 Unit 2).
    // Open / stroke-only / undecidable geometry is not a plate; unmodeled
    // ancestor transforms still force Indeterminate paint with exact shape.
    let opacity = ctx.opacity * node.opacity().unwrap_or(1.0);
    let Some(paint) = resolve_fill_paint(
        &path.fill,
        path.style.as_deref(),
        env.style_map,
        env.resolved_tokens,
        opacity,
    ) else {
        return;
    };
    let Some((bounds, shape)) = path_fill_region(path, ctx.place(), ctx.page_size) else {
        return;
    };
    let rotation = leaf_rotation(node, bounds);
    let paint = if ctx.unmodeled || skewed(ctx, rotation.is_some()) {
        BackdropPaint::Indeterminate
    } else {
        paint
    };
    if let Some(bounds) = clip_bounds(ctx.clip, bounds) {
        candidates.push(BackdropCandidate {
            paint,
            bounds,
            shape,
            rotation,
        });
    }
}

fn push_backdrop(
    node: &Node,
    fill: &Option<PropertyValue>,
    style: &Option<String>,
    shape: CoverageShape,
    ctx: PaintCtx<'_>,
    candidates: &mut Vec<BackdropCandidate>,
    env: ContrastEnv<'_>,
) {
    let opacity = ctx.opacity * node.opacity().unwrap_or(1.0);
    let Some(paint) = resolve_fill_paint(
        fill,
        style.as_deref(),
        env.style_map,
        env.resolved_tokens,
        opacity,
    ) else {
        return;
    };
    let Some(bounds) = absolute_box(node, ctx, env.resolved_tokens) else {
        return;
    };
    // A paint-altering effect on the leaf itself (mask/filter/blur/non-normal
    // blend), or an unmodeled ancestor transform, makes the composited colour
    // un-sampleable — downgrade to indeterminate rather than trust the raw fill.
    // A rotation on the leaf is modeled EXACTLY: the renderer rotates it about
    // its own box center, so containment is tested by inverse-rotating samples.
    let rotation = leaf_rotation(node, bounds);
    let paint = if ctx.unmodeled || candidate_has_effect(node) || skewed(ctx, rotation.is_some()) {
        BackdropPaint::Indeterminate
    } else {
        paint
    };
    if let Some(clipped) = clip_bounds(ctx.clip, bounds) {
        candidates.push(BackdropCandidate {
            paint,
            bounds: clipped,
            shape: shape.scaled(ctx.sx.min(ctx.sy)),
            rotation,
        });
    }
}

fn push_image_backdrop(
    node: &Node,
    _image: &ImageNode,
    ctx: PaintCtx<'_>,
    candidates: &mut Vec<BackdropCandidate>,
    env: ContrastEnv<'_>,
) {
    if ctx.opacity * node.opacity().unwrap_or(1.0) < MIN_PAINT_ALPHA {
        return;
    }
    let Some(bounds) = absolute_box(node, ctx, env.resolved_tokens) else {
        return;
    };
    let rotation = leaf_rotation(node, bounds);
    if let Some(clipped) = clip_bounds(ctx.clip, bounds) {
        candidates.push(BackdropCandidate {
            paint: BackdropPaint::Indeterminate,
            bounds: clipped,
            shape: CoverageShape::Rect,
            rotation,
        });
    }
}

fn absolute_box(
    node: &Node,
    ctx: PaintCtx<'_>,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
) -> Option<RectPx> {
    local_box(node, ctx.page_size, resolved_tokens).map(|b| ctx.place().rect(b))
}

/// `true` when a rotated leaf draws under a non-uniform scale: the drawn
/// outline is sheared, so the rigid rotation model does not hold.
fn skewed(ctx: PaintCtx<'_>, rotates: bool) -> bool {
    rotates && !ctx.uniform()
}

fn cascaded_opacity(parent: f64, opacity: Option<f64>) -> f64 {
    parent * opacity.unwrap_or(1.0).clamp(0.0, 1.0)
}
