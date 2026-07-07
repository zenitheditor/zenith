//! Solid-fill and solid-stroke primitive draws (rect, rounded-rect, ellipse,
//! line, polygon, polyline, path). Each function pulls its fields from the matching
//! [`SceneCommand`] variant and draws into `target` under the shared
//! [`DrawCtx`]; behavior is byte-identical to the prior inline match arms.

use tiny_skia::{FillRule, Mask, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};
use zenith_scene::{
    Paint as ScenePaint, SceneCommand, StrokeAlign, ir::path_segments_bbox,
    ir::path_segments_finite,
};

use super::super::commands::{DrawCtx, build_stroke_dash, map_line_cap, map_line_join};
use super::super::gradient::gradient_shader;
use super::super::paths::{
    build_align_mask, build_path_align_mask, build_poly_path, build_rounded_rect_path,
    build_scene_path, clip_mask, intersect_rects,
};

/// Build a tiny-skia fill paint from a scene [`ScenePaint`] over the bounding
/// box `(x, y, w, h)` (used to resolve the gradient line). Anti-aliased — the
/// caller uses this for path fills (rounded-rect, ellipse, polygon, and the
/// rotated/gradient rect branch). Returns `None` when a gradient shader cannot
/// be built (e.g. fewer than two stops), so the caller skips the draw.
fn ts_fill_paint(paint: &ScenePaint, x: f64, y: f64, w: f64, h: f64) -> Option<Paint<'static>> {
    match paint {
        ScenePaint::Solid { color } => {
            let mut p = Paint::default();
            p.set_color_rgba8(color.r, color.g, color.b, color.a);
            p.anti_alias = true;
            Some(p)
        }
        ScenePaint::Gradient(gradient) => {
            let shader = gradient_shader(x, y, w, h, gradient)?;
            Some(Paint {
                shader,
                anti_alias: true,
                ..Default::default()
            })
        }
    }
}

/// The axis-aligned bounding box `(x, y, w, h)` of a flat `[x0,y0,x1,y1,…]`
/// point list, used to resolve a gradient line for polygon fills.
fn flat_points_bbox(points: &[f64]) -> Option<(f64, f64, f64, f64)> {
    let mut xs = points.iter().step_by(2).copied();
    let mut ys = points.iter().skip(1).step_by(2).copied();
    let (mut min_x, mut max_x) = {
        let first = xs.next()?;
        (first, first)
    };
    let (mut min_y, mut max_y) = {
        let first = ys.next()?;
        (first, first)
    };
    for x in xs {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
    }
    for y in ys {
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    Some((min_x, min_y, max_x - min_x, max_y - min_y))
}

pub(in crate::tiny_skia) fn fill_rect(target: &mut Pixmap, ctx: DrawCtx, cmd: &SceneCommand) {
    let SceneCommand::FillRect { x, y, w, h, paint } = cmd else {
        return;
    };
    match paint {
        // ── Solid fill — byte-identical to the pre-Paint behavior ──
        ScenePaint::Solid { color } => {
            if ctx.current_ts.is_identity() {
                // ── Unrotated (identity) path — AA-off axis-aligned fill ──
                let fill_rect = (*x, *y, x + w, y + h);
                let effective_clip = ctx.effective_clip;

                // Intersect the fill rect with the current effective clip.
                let (ix, iy, ix2, iy2) = match intersect_rects(fill_rect, effective_clip) {
                    Some(r) => r,
                    None => return, // nothing to draw
                };

                let iw = ix2 - ix;
                let ih = iy2 - iy;

                // tiny-skia requires positive, finite values for Rect::from_xywh.
                if iw <= 0.0
                    || ih <= 0.0
                    || !ix.is_finite()
                    || !iy.is_finite()
                    || !iw.is_finite()
                    || !ih.is_finite()
                {
                    return;
                }

                let rect = match Rect::from_xywh(ix as f32, iy as f32, iw as f32, ih as f32) {
                    Some(r) => r,
                    None => return,
                };

                let mut paint = Paint::default();
                paint.set_color_rgba8(color.r, color.g, color.b, color.a);
                paint.anti_alias = false; // deterministic: no edge AA variance

                // Drawing outside the pixmap simply touches no pixels; not an error.
                target.fill_rect(rect, &paint, Transform::identity(), None);
            } else {
                // ── Rotated path: fill the rect as a path under the current
                // transform, AA-on, masked by the (axis-aligned) clip. ──
                let effective_clip = ctx.effective_clip;
                let mask = match clip_mask(effective_clip, ctx.width, ctx.height) {
                    None => return,
                    Some(m) => m,
                };
                let Some(rect) = Rect::from_xywh(*x as f32, *y as f32, *w as f32, *h as f32) else {
                    return;
                };
                let path = PathBuilder::from_rect(rect);
                let mut paint = Paint::default();
                paint.set_color_rgba8(color.r, color.g, color.b, color.a);
                paint.anti_alias = true;
                target.fill_path(
                    &path,
                    &paint,
                    FillRule::Winding,
                    ctx.current_ts,
                    mask.as_ref(),
                );
            }
        }
        // ── Gradient fill — path-fill with a shader (AA on) ──
        ScenePaint::Gradient(_) => {
            if !x.is_finite()
                || !y.is_finite()
                || !w.is_finite()
                || !h.is_finite()
                || *w <= 0.0
                || *h <= 0.0
            {
                return;
            }
            let effective_clip = ctx.effective_clip;
            if intersect_rects((*x, *y, x + w, y + h), effective_clip).is_none() {
                return;
            }
            let Some(rect) = Rect::from_xywh(*x as f32, *y as f32, *w as f32, *h as f32) else {
                return;
            };
            let path = PathBuilder::from_rect(rect);
            let mask = match clip_mask(effective_clip, ctx.width, ctx.height) {
                None => return,
                Some(m) => m,
            };
            let Some(paint_ts) = ts_fill_paint(paint, *x, *y, *w, *h) else {
                return;
            };
            target.fill_path(
                &path,
                &paint_ts,
                FillRule::Winding,
                ctx.current_ts,
                mask.as_ref(),
            );
        }
    }
}

pub(in crate::tiny_skia) fn fill_ellipse(target: &mut Pixmap, ctx: DrawCtx, cmd: &SceneCommand) {
    let SceneCommand::FillEllipse {
        x,
        y,
        w,
        h,
        rx,
        ry,
        paint,
    } = cmd
    else {
        return;
    };
    // Guard against non-finite or degenerate dimensions.
    if !x.is_finite()
        || !y.is_finite()
        || !w.is_finite()
        || !h.is_finite()
        || *w <= 0.0
        || *h <= 0.0
    {
        return;
    }

    // Compute oval bounding box: rx/ry override the semi-axes.
    // When absent, the oval is inscribed in the node bbox.
    let ow = rx.map_or(*w, |r| r * 2.0);
    let oh = ry.map_or(*h, |r| r * 2.0);
    let ox = x + (w - ow) / 2.0;
    let oy = y + (h - oh) / 2.0;

    let effective_clip = ctx.effective_clip;

    // Early-out: skip if the ellipse bbox is entirely outside the clip.
    if intersect_rects((ox, oy, ox + ow, oy + oh), effective_clip).is_none() {
        return;
    }

    // Build the oval at its TRUE bounding box — NOT the intersected box.
    // Intersecting the bbox before building the oval would reshape (squish)
    // the ellipse under partial clip; instead we draw the full ellipse and
    // let the clip mask truncate it.
    let Some(rect) = Rect::from_xywh(ox as f32, oy as f32, ow as f32, oh as f32) else {
        return;
    };
    let Some(path) = PathBuilder::from_oval(rect) else {
        return; // degenerate rect: skip
    };

    // Build clip mask from the effective clip (truncates, not reshapes).
    // AA-on: curved fill, deterministic same-machine.
    let mask = match clip_mask(effective_clip, ctx.width, ctx.height) {
        None => return,
        Some(m) => m,
    };

    // The gradient line resolves over the node bbox (x, y, w, h), matching the
    // rect/rounded-rect convention; the path itself is the oval bbox.
    let Some(paint_ts) = ts_fill_paint(paint, *x, *y, *w, *h) else {
        return;
    };

    target.fill_path(
        &path,
        &paint_ts,
        FillRule::Winding,
        ctx.current_ts,
        mask.as_ref(),
    );
}

pub(in crate::tiny_skia) fn stroke_ellipse(target: &mut Pixmap, ctx: DrawCtx, cmd: &SceneCommand) {
    let SceneCommand::StrokeEllipse {
        x,
        y,
        w,
        h,
        rx,
        ry,
        color,
        stroke_width,
        stroke_dash,
        stroke_gap,
        stroke_linecap,
    } = cmd
    else {
        return;
    };
    if !x.is_finite()
        || !y.is_finite()
        || !w.is_finite()
        || !h.is_finite()
        || !stroke_width.is_finite()
        || *stroke_width > f64::from(f32::MAX)
        || *w <= 0.0
        || *h <= 0.0
    {
        return;
    }

    // Compute oval bounding box from rx/ry semi-axes (or node bbox).
    let ow = rx.map_or(*w, |r| r * 2.0);
    let oh = ry.map_or(*h, |r| r * 2.0);
    let ox = x + (w - ow) / 2.0;
    let oy = y + (h - oh) / 2.0;

    let effective_clip = ctx.effective_clip;

    // Ink-bbox early-out: the stroke extends half its width beyond
    // the ellipse edge on all sides.
    let half_sw = stroke_width / 2.0;
    if intersect_rects(
        (
            ox - half_sw,
            oy - half_sw,
            ox + ow + half_sw,
            oy + oh + half_sw,
        ),
        effective_clip,
    )
    .is_none()
    {
        return;
    }

    // Build the oval path at its TRUE bounding box — NOT the
    // intersected box. The clip mask truncates without reshaping.
    let Some(rect) = Rect::from_xywh(ox as f32, oy as f32, ow as f32, oh as f32) else {
        return;
    };
    let Some(path) = PathBuilder::from_oval(rect) else {
        return; // degenerate rect: skip
    };

    let mask = match clip_mask(effective_clip, ctx.width, ctx.height) {
        None => return,
        Some(m) => m,
    };

    let line_cap = map_line_cap(*stroke_linecap);
    let dash = build_stroke_dash(*stroke_dash, *stroke_gap);
    let stroke = Stroke {
        width: *stroke_width as f32,
        line_cap,
        dash,
        ..Default::default()
    };

    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, color.a);
    // AA-on: curved stroke edge, deterministic same-machine.
    paint.anti_alias = true;

    target.stroke_path(&path, &paint, &stroke, ctx.current_ts, mask.as_ref());
}

pub(in crate::tiny_skia) fn stroke_line(target: &mut Pixmap, ctx: DrawCtx, cmd: &SceneCommand) {
    let SceneCommand::StrokeLine {
        x1,
        y1,
        x2,
        y2,
        color,
        stroke_width,
        stroke_dash,
        stroke_gap,
        stroke_linecap,
    } = cmd
    else {
        return;
    };
    // Guard against non-finite or out-of-f32-range values before
    // building the path — tiny-skia requires finite f32 values,
    // and a finite-but-huge f64 would overflow to f32::INFINITY.
    if !x1.is_finite()
        || !y1.is_finite()
        || !x2.is_finite()
        || !y2.is_finite()
        || !stroke_width.is_finite()
        || *stroke_width > f64::from(f32::MAX)
    {
        return;
    }

    let effective_clip = ctx.effective_clip;

    // A line is 1-D so we cannot reshape it to the clip; instead we
    // compute the ink bounding box (endpoints expanded by half the
    // stroke width) as a cheap early-out, then clip the stroke to the
    // effective clip via a mask so sub-page (frame) clips truncate the
    // line at the frame edge.
    let half_sw = stroke_width / 2.0;
    let ink_x = x1.min(*x2) - half_sw;
    let ink_y = y1.min(*y2) - half_sw;
    let ink_x2 = x1.max(*x2) + half_sw;
    let ink_y2 = y1.max(*y2) + half_sw;
    if intersect_rects((ink_x, ink_y, ink_x2, ink_y2), effective_clip).is_none() {
        return;
    }

    let mask = match clip_mask(effective_clip, ctx.width, ctx.height) {
        None => return,
        Some(m) => m,
    };

    // Build path: a single open segment from (x1,y1) to (x2,y2).
    let mut pb = PathBuilder::new();
    pb.move_to(*x1 as f32, *y1 as f32);
    pb.line_to(*x2 as f32, *y2 as f32);
    let path = match pb.finish() {
        Some(p) => p,
        None => return, // degenerate (zero-length) line: skip
    };

    let line_cap = map_line_cap(*stroke_linecap);
    let dash = build_stroke_dash(*stroke_dash, *stroke_gap);
    let stroke = Stroke {
        width: *stroke_width as f32,
        line_cap,
        dash,
        ..Default::default()
    };

    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, color.a);
    // AA on: diagonal lines need sub-pixel coverage; deterministic
    // same-machine like ellipse/glyph fills.
    paint.anti_alias = true;

    target.stroke_path(&path, &paint, &stroke, ctx.current_ts, mask.as_ref());
}

pub(in crate::tiny_skia) fn fill_polygon(target: &mut Pixmap, ctx: DrawCtx, cmd: &SceneCommand) {
    let SceneCommand::FillPolygon {
        points,
        paint,
        even_odd,
    } = cmd
    else {
        return;
    };
    // Guard: need at least 3 points (6 coordinates).
    if points.len() < 6 {
        return;
    }
    // Guard: any non-finite coordinate.
    if points.iter().any(|v| !v.is_finite()) {
        return;
    }

    let path = match build_poly_path(points, true) {
        Some(p) => p,
        None => return,
    };

    let effective_clip = ctx.effective_clip;
    let mask = match clip_mask(effective_clip, ctx.width, ctx.height) {
        None => return,
        Some(m) => m,
    };

    let fill_rule = if *even_odd {
        FillRule::EvenOdd
    } else {
        FillRule::Winding
    };

    // A gradient fill resolves its line over the polygon's bounding box.
    let Some((bx, by, bw, bh)) = flat_points_bbox(points) else {
        return;
    };
    let Some(paint_ts) = ts_fill_paint(paint, bx, by, bw, bh) else {
        return;
    };

    target.fill_path(&path, &paint_ts, fill_rule, ctx.current_ts, mask.as_ref());
}

pub(in crate::tiny_skia) fn stroke_polyline(target: &mut Pixmap, ctx: DrawCtx, cmd: &SceneCommand) {
    let SceneCommand::StrokePolyline {
        points,
        color,
        stroke_width,
        closed,
        align,
        fill_even_odd,
    } = cmd
    else {
        return;
    };
    // Guard: need at least 2 points (4 coordinates).
    if points.len() < 4 {
        return;
    }
    // Guard: any non-finite coordinate or invalid stroke_width.
    if points.iter().any(|v| !v.is_finite())
        || !stroke_width.is_finite()
        || *stroke_width > f64::from(f32::MAX)
    {
        return;
    }

    let path = match build_poly_path(points, *closed) {
        Some(p) => p,
        None => return,
    };

    let effective_clip = ctx.effective_clip;
    let mask = match clip_mask(effective_clip, ctx.width, ctx.height) {
        None => return,
        Some(m) => m,
    };

    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, color.a);
    paint.anti_alias = true;

    // Aligned stroke (Inside/Outside on a CLOSED polygon): draw at
    // 2× width centered on the path and clip with a fill-region mask
    // so exactly the inside (or outside) half survives. Building this
    // mask can fail on degenerate sizes / paths — fall back to the
    // centered branch rather than skip or panic.
    let aligned_mask: Option<Mask> = match align {
        StrokeAlign::Center => None,
        StrokeAlign::Inside | StrokeAlign::Outside if *closed => build_align_mask(
            points,
            *align,
            *fill_even_odd,
            effective_clip,
            ctx.width,
            ctx.height,
            ctx.current_ts,
        ),
        // Inside/Outside on an open path is meaningless: center.
        StrokeAlign::Inside | StrokeAlign::Outside => None,
    };

    let stroke_width_px = if aligned_mask.is_some() {
        (*stroke_width * 2.0) as f32
    } else {
        *stroke_width as f32
    };
    // Stroke defaults: Butt cap, Miter join, miter_limit 4 — normative v0.
    let stroke = Stroke {
        width: stroke_width_px,
        ..Default::default()
    };

    let draw_mask: Option<&Mask> = match &aligned_mask {
        Some(m) => Some(m),
        None => mask.as_ref(),
    };

    target.stroke_path(&path, &paint, &stroke, ctx.current_ts, draw_mask);
}

pub(in crate::tiny_skia) fn fill_path(target: &mut Pixmap, ctx: DrawCtx, cmd: &SceneCommand) {
    let SceneCommand::FillPath {
        segments,
        paint,
        even_odd,
    } = cmd
    else {
        return;
    };
    if segments.len() < 3 || !path_segments_finite(segments) {
        return;
    }
    let path = match build_scene_path(segments) {
        Some(p) => p,
        None => return,
    };
    let effective_clip = ctx.effective_clip;
    let mask = match clip_mask(effective_clip, ctx.width, ctx.height) {
        None => return,
        Some(m) => m,
    };
    let fill_rule = if *even_odd {
        FillRule::EvenOdd
    } else {
        FillRule::Winding
    };
    let Some((bx, by, bw, bh)) = path_segments_bbox(segments) else {
        return;
    };
    let Some(paint_ts) = ts_fill_paint(paint, bx, by, bw, bh) else {
        return;
    };
    target.fill_path(&path, &paint_ts, fill_rule, ctx.current_ts, mask.as_ref());
}

pub(in crate::tiny_skia) fn stroke_path(target: &mut Pixmap, ctx: DrawCtx, cmd: &SceneCommand) {
    let SceneCommand::StrokePath {
        segments,
        color,
        stroke_width,
        closed,
        align,
        fill_even_odd,
        stroke_linejoin,
        stroke_miter_limit,
    } = cmd
    else {
        return;
    };
    if segments.len() < 2
        || !path_segments_finite(segments)
        || !stroke_width.is_finite()
        || *stroke_width > f64::from(f32::MAX)
    {
        return;
    }
    let path = match build_scene_path(segments) {
        Some(p) => p,
        None => return,
    };
    let effective_clip = ctx.effective_clip;
    let mask = match clip_mask(effective_clip, ctx.width, ctx.height) {
        None => return,
        Some(m) => m,
    };
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, color.a);
    paint.anti_alias = true;
    let aligned_mask: Option<Mask> = match align {
        StrokeAlign::Center => None,
        StrokeAlign::Inside | StrokeAlign::Outside if *closed => build_path_align_mask(
            &path,
            *align,
            *fill_even_odd,
            effective_clip,
            ctx.width,
            ctx.height,
            ctx.current_ts,
        ),
        StrokeAlign::Inside | StrokeAlign::Outside => None,
    };
    let stroke_width_px = if aligned_mask.is_some() {
        *stroke_width * 2.0
    } else {
        *stroke_width
    };
    if !stroke_width_px.is_finite() || stroke_width_px > f64::from(f32::MAX) {
        return;
    }
    let miter_limit = match stroke_miter_limit {
        Some(limit) if limit.is_finite() && *limit > 0.0 && *limit <= f64::from(f32::MAX) => {
            *limit as f32
        }
        Some(_) => return,
        None => Stroke::default().miter_limit,
    };
    let stroke = Stroke {
        width: stroke_width_px as f32,
        line_join: map_line_join(*stroke_linejoin),
        miter_limit,
        ..Default::default()
    };
    let draw_mask: Option<&Mask> = match &aligned_mask {
        Some(m) => Some(m),
        None => mask.as_ref(),
    };
    target.stroke_path(&path, &paint, &stroke, ctx.current_ts, draw_mask);
}

pub(in crate::tiny_skia) fn stroke_rect(target: &mut Pixmap, ctx: DrawCtx, cmd: &SceneCommand) {
    let SceneCommand::StrokeRect {
        x,
        y,
        w,
        h,
        color,
        stroke_width,
        stroke_dash,
        stroke_gap,
        stroke_linecap,
    } = cmd
    else {
        return;
    };
    if !x.is_finite()
        || !y.is_finite()
        || !w.is_finite()
        || !h.is_finite()
        || !stroke_width.is_finite()
        || *stroke_width > f64::from(f32::MAX)
        || *w <= 0.0
        || *h <= 0.0
    {
        return;
    }

    let effective_clip = ctx.effective_clip;

    // Ink-bbox early-out: the stroke extends half its width beyond
    // the rect edge on all sides.
    let half_sw = stroke_width / 2.0;
    if intersect_rects(
        (x - half_sw, y - half_sw, x + w + half_sw, y + h + half_sw),
        effective_clip,
    )
    .is_none()
    {
        return;
    }

    let Some(rect) = Rect::from_xywh(*x as f32, *y as f32, *w as f32, *h as f32) else {
        return;
    };
    let path = PathBuilder::from_rect(rect);

    let mask = match clip_mask(effective_clip, ctx.width, ctx.height) {
        None => return,
        Some(m) => m,
    };

    let line_cap = map_line_cap(*stroke_linecap);
    let dash = build_stroke_dash(*stroke_dash, *stroke_gap);
    let stroke = Stroke {
        width: *stroke_width as f32,
        line_cap,
        dash,
        ..Default::default()
    };

    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, color.a);
    paint.anti_alias = true;

    target.stroke_path(&path, &paint, &stroke, ctx.current_ts, mask.as_ref());
}

pub(in crate::tiny_skia) fn fill_rounded_rect(
    target: &mut Pixmap,
    ctx: DrawCtx,
    cmd: &SceneCommand,
) {
    let SceneCommand::FillRoundedRect {
        x,
        y,
        w,
        h,
        radius,
        radii,
        paint,
    } = cmd
    else {
        return;
    };
    if !x.is_finite()
        || !y.is_finite()
        || !w.is_finite()
        || !h.is_finite()
        || !radius.is_finite()
        || *w <= 0.0
        || *h <= 0.0
    {
        return;
    }

    let effective_clip = ctx.effective_clip;
    if intersect_rects((*x, *y, x + w, y + h), effective_clip).is_none() {
        return;
    }

    // Per-corner radii override uniform radius when present.
    let corner_radii = radii.map_or([*radius as f32; 4], |a| a.map(|v| v as f32));
    let Some(path) =
        build_rounded_rect_path(*x as f32, *y as f32, *w as f32, *h as f32, corner_radii)
    else {
        return;
    };

    let mask = match clip_mask(effective_clip, ctx.width, ctx.height) {
        None => return,
        Some(m) => m,
    };

    let Some(paint_ts) = ts_fill_paint(paint, *x, *y, *w, *h) else {
        return;
    };

    target.fill_path(
        &path,
        &paint_ts,
        FillRule::Winding,
        ctx.current_ts,
        mask.as_ref(),
    );
}

pub(in crate::tiny_skia) fn stroke_rounded_rect(
    target: &mut Pixmap,
    ctx: DrawCtx,
    cmd: &SceneCommand,
) {
    let SceneCommand::StrokeRoundedRect {
        x,
        y,
        w,
        h,
        radius,
        radii,
        color,
        stroke_width,
        stroke_dash,
        stroke_gap,
        stroke_linecap,
    } = cmd
    else {
        return;
    };
    if !x.is_finite()
        || !y.is_finite()
        || !w.is_finite()
        || !h.is_finite()
        || !radius.is_finite()
        || !stroke_width.is_finite()
        || *stroke_width > f64::from(f32::MAX)
        || *w <= 0.0
        || *h <= 0.0
    {
        return;
    }

    let effective_clip = ctx.effective_clip;

    let half_sw = stroke_width / 2.0;
    if intersect_rects(
        (x - half_sw, y - half_sw, x + w + half_sw, y + h + half_sw),
        effective_clip,
    )
    .is_none()
    {
        return;
    }

    // Per-corner radii override uniform radius when present.
    let corner_radii = radii.map_or([*radius as f32; 4], |a| a.map(|v| v as f32));
    let Some(path) =
        build_rounded_rect_path(*x as f32, *y as f32, *w as f32, *h as f32, corner_radii)
    else {
        return;
    };

    let mask = match clip_mask(effective_clip, ctx.width, ctx.height) {
        None => return,
        Some(m) => m,
    };

    let line_cap = map_line_cap(*stroke_linecap);
    let dash = build_stroke_dash(*stroke_dash, *stroke_gap);
    let stroke = Stroke {
        width: *stroke_width as f32,
        line_cap,
        dash,
        ..Default::default()
    };

    let mut paint = Paint::default();
    paint.set_color_rgba8(color.r, color.g, color.b, color.a);
    paint.anti_alias = true;

    target.stroke_path(&path, &paint, &stroke, ctx.current_ts, mask.as_ref());
}
