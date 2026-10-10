//! Scene-walk driver ([`translate`]) and per-command emitter ([`emit_command`]).

use pdf_writer::Content;
use zenith_core::{AssetProvider, FontProvider};
use zenith_geometry::math;
use zenith_scene::{
    FillRule, Scene, SceneCommand, StrokeAlign,
    ir::{path_segments_bbox, path_segments_finite},
};

use super::image::{ImageDraw, emit_image};
use crate::pdf::color;
use crate::pdf::content::draw::{
    apply_alpha, apply_fill_rule, fill_region, finite, rect_ok, set_dash, set_line_cap,
    set_line_join, set_miter_limit,
};
use crate::pdf::content::resources::PageResources;
use crate::pdf::font::FontPlan;
use crate::pdf::geometry::{ellipse_path, poly_bbox, poly_path, rounded_rect_path, scene_path};

/// Translate `scene` into a single content stream plus the [`PageResources`] it
/// references. `fonts` resolves glyph outlines; `assets` resolves image bytes.
pub(in crate::pdf) fn translate(
    scene: &Scene,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    font_plan: &FontPlan,
) -> (Content, PageResources) {
    let mut content = Content::new();
    let mut res = PageResources::default();

    // Initial CTM: flip the y axis so scene (top-left, y-down) maps to PDF
    // user space (bottom-left, y-up). 1 scene px == 1 PDF unit.
    content.transform([1.0, 0.0, 0.0, -1.0, 0.0, scene.height as f32]);

    let page = (scene.width, scene.height);

    // Plan complete scopes before emitting any graphics state.
    // Planner errors produce empty ranges, so the vector emitter retains every command.
    let ranges = super::super::scopes::plan(scene, fonts, assets).unwrap_or_default();
    let mut cursor = 0;
    for range in ranges {
        if let Some(commands) = scene.commands.get(cursor..range.start) {
            for cmd in commands {
                emit_command(&mut content, &mut res, cmd, page, fonts, assets, font_plan);
            }
        }
        if let Some(commands) = scene.commands.get(range.clone()) {
            crate::pdf::raster_embed::embed_rasterized_region(
                &mut content,
                &mut res,
                commands,
                page,
                fonts,
                assets,
                font_plan,
            );
        }
        cursor = range.end;
    }
    if let Some(commands) = scene.commands.get(cursor..) {
        for cmd in commands {
            emit_command(&mut content, &mut res, cmd, page, fonts, assets, font_plan);
        }
    }
    (content, res)
}

pub(in crate::pdf) fn translate_strict(
    scene: &Scene,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    font_plan: &FontPlan,
    regions: &[crate::pdf::report::PlannedRegion],
    raster_scale: f64,
) -> Result<(Content, PageResources), crate::RenderError> {
    let mut content = Content::new();
    let mut res = PageResources::default();
    content.transform([1.0, 0.0, 0.0, -1.0, 0.0, scene.height as f32]);
    let page = (scene.width, scene.height);
    let mut cursor = 0;
    for region in regions {
        for cmd in scene
            .commands
            .get(cursor..region.range.start)
            .unwrap_or_default()
        {
            emit_command(&mut content, &mut res, cmd, page, fonts, assets, font_plan);
        }
        let commands = scene.commands.get(region.range.clone()).ok_or_else(|| {
            crate::RenderError::new("invalid PDF capture range; correct scene commands")
        })?;
        crate::pdf::raster_embed::embed_strict(
            &mut content,
            &mut res,
            commands,
            page,
            fonts,
            assets,
            raster_scale,
        )
        .map_err(|error| {
            crate::RenderError::new(format!(
                "PDF commands {}..{} capture error: {error}; correct scene dimensions or resources",
                region.range.start, region.range.end
            ))
        })?;
        cursor = region.range.end;
    }
    for cmd in scene.commands.get(cursor..).unwrap_or_default() {
        emit_command(&mut content, &mut res, cmd, page, fonts, assets, font_plan);
    }
    Ok((content, res))
}

pub(in crate::pdf) fn emit_command(
    content: &mut Content,
    res: &mut PageResources,
    cmd: &SceneCommand,
    page: (f64, f64),
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    font_plan: &FontPlan,
) {
    match cmd {
        // ── Filled shapes ─────────────────────────────────────────────────
        SceneCommand::FillRect { x, y, w, h, paint } => {
            if !rect_ok(*x, *y, *w, *h) {
                return;
            }
            fill_region(
                content,
                res,
                paint,
                (*x, *y, *w, *h),
                FillRule::NonZero,
                |c| {
                    c.rect(*x as f32, *y as f32, *w as f32, *h as f32);
                    true
                },
            );
        }

        SceneCommand::StrokeRect {
            x,
            y,
            w,
            h,
            color,
            stroke_width,
            stroke_dash,
            stroke_gap,
            stroke_linecap,
        } => {
            if !rect_ok(*x, *y, *w, *h) || !finite(*stroke_width) {
                return;
            }
            content.save_state();
            apply_alpha(content, res, color);
            color::set_stroke(content, color);
            content.set_line_width(*stroke_width as f32);
            set_dash(content, *stroke_dash, *stroke_gap);
            if stroke_linecap.is_some() {
                set_line_cap(content, *stroke_linecap);
            }
            content.rect(*x as f32, *y as f32, *w as f32, *h as f32);
            content.stroke();
            content.restore_state();
        }

        SceneCommand::FillRoundedRect {
            x,
            y,
            w,
            h,
            radius,
            radii,
            paint,
        } => {
            if !rect_ok(*x, *y, *w, *h) || !finite(*radius) {
                return;
            }
            let corner_radii = radii.unwrap_or([*radius; 4]);
            fill_region(
                content,
                res,
                paint,
                (*x, *y, *w, *h),
                FillRule::NonZero,
                |c| {
                    rounded_rect_path(c, *x, *y, *w, *h, corner_radii);
                    true
                },
            );
        }

        SceneCommand::StrokeRoundedRect {
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
        } => {
            if !rect_ok(*x, *y, *w, *h) || !finite(*radius) || !finite(*stroke_width) {
                return;
            }
            let corner_radii = radii.unwrap_or([*radius; 4]);
            content.save_state();
            apply_alpha(content, res, color);
            color::set_stroke(content, color);
            content.set_line_width(*stroke_width as f32);
            set_dash(content, *stroke_dash, *stroke_gap);
            if stroke_linecap.is_some() {
                set_line_cap(content, *stroke_linecap);
            }
            rounded_rect_path(content, *x, *y, *w, *h, corner_radii);
            content.stroke();
            content.restore_state();
        }

        SceneCommand::FillEllipse {
            x,
            y,
            w,
            h,
            rx,
            ry,
            paint,
        } => {
            if !rect_ok(*x, *y, *w, *h) {
                return;
            }
            fill_region(
                content,
                res,
                paint,
                (*x, *y, *w, *h),
                FillRule::NonZero,
                |c| {
                    ellipse_path(c, *x, *y, *w, *h, *rx, *ry);
                    true
                },
            );
        }

        SceneCommand::StrokeEllipse {
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
        } => {
            if !rect_ok(*x, *y, *w, *h) || !finite(*stroke_width) {
                return;
            }
            content.save_state();
            apply_alpha(content, res, color);
            color::set_stroke(content, color);
            content.set_line_width(*stroke_width as f32);
            set_dash(content, *stroke_dash, *stroke_gap);
            if stroke_linecap.is_some() {
                set_line_cap(content, *stroke_linecap);
            }
            ellipse_path(content, *x, *y, *w, *h, *rx, *ry);
            content.stroke();
            content.restore_state();
        }

        SceneCommand::StrokeLine {
            x1,
            y1,
            x2,
            y2,
            color,
            stroke_width,
            stroke_dash,
            stroke_gap,
            stroke_linecap,
        } => {
            if !finite(*x1)
                || !finite(*y1)
                || !finite(*x2)
                || !finite(*y2)
                || !finite(*stroke_width)
            {
                return;
            }
            content.save_state();
            apply_alpha(content, res, color);
            color::set_stroke(content, color);
            content.set_line_width(*stroke_width as f32);
            set_dash(content, *stroke_dash, *stroke_gap);
            if stroke_linecap.is_some() {
                set_line_cap(content, *stroke_linecap);
            }
            content.move_to(*x1 as f32, *y1 as f32);
            content.line_to(*x2 as f32, *y2 as f32);
            content.stroke();
            content.restore_state();
        }

        SceneCommand::FillPolygon {
            points,
            paint,
            fill_rule,
        } => {
            if points.len() < 6 || points.iter().any(|v| !v.is_finite()) {
                return;
            }
            let bbox = poly_bbox(points);
            fill_region(content, res, paint, bbox, *fill_rule, |c| {
                poly_path(c, points, true)
            });
        }

        SceneCommand::StrokePolyline {
            points,
            color,
            stroke_width,
            closed,
            align,
            clip_fill_rule,
        } => {
            if points.len() < 4 || points.iter().any(|v| !v.is_finite()) || !finite(*stroke_width) {
                return;
            }

            // Aligned stroke (Inside/Outside on a CLOSED polygon): draw at 2× width
            // and clip to the fill region (Inside) or its complement (Outside) so a
            // full-width stroke sits flush against the boundary. Center / open paths
            // are unchanged.
            let aligned = *closed && !matches!(align, StrokeAlign::Center);

            content.save_state();
            apply_alpha(content, res, color);
            color::set_stroke(content, color);

            if aligned {
                // 1. Install the alignment clip from the polygon fill path.
                match align {
                    StrokeAlign::Inside => {
                        // Clip = polygon interior (per fill rule).
                        if !poly_path(content, points, true) {
                            content.end_path();
                            content.restore_state();
                            return;
                        }
                        apply_fill_rule(
                            content,
                            *clip_fill_rule,
                            |content| {
                                content.clip_nonzero();
                            },
                            |content| {
                                content.clip_even_odd();
                            },
                        );
                        content.end_path();
                    }
                    StrokeAlign::Outside => {
                        // Clip = (generous outer rect) minus polygon interior, via the
                        // even-odd rule on the combined subpaths → the exterior region.
                        let (pw, ph) = page;
                        let m = pw.max(ph).max(1.0); // generous margin past the page
                        content.move_to(-m as f32, -m as f32);
                        content.line_to((pw + m) as f32, -m as f32);
                        content.line_to((pw + m) as f32, (ph + m) as f32);
                        content.line_to(-m as f32, (ph + m) as f32);
                        content.close_path();
                        if !poly_path(content, points, true) {
                            content.end_path();
                            content.restore_state();
                            return;
                        }
                        content.clip_even_odd();
                        content.end_path();
                    }
                    // `aligned` is only true when align != Center, so this arm is dead;
                    // kept (no wildcard) for exhaustiveness. A no-op is the safe fallback
                    // — it simply leaves the clip unchanged.
                    StrokeAlign::Center => {}
                }
                // 2. Stroke the path at 2× width inside the clip.
                content.set_line_width((*stroke_width * 2.0) as f32);
                if poly_path(content, points, true) {
                    content.stroke();
                } else {
                    content.end_path();
                }
            } else {
                content.set_line_width(*stroke_width as f32);
                if poly_path(content, points, *closed) {
                    content.stroke();
                } else {
                    content.end_path();
                }
            }
            content.restore_state();
        }

        SceneCommand::FillPath {
            segments,
            paint,
            fill_rule,
        } => {
            if segments.len() < 3 || !path_segments_finite(segments) {
                return;
            }
            let Some(bbox) = path_segments_bbox(segments) else {
                return;
            };
            fill_region(content, res, paint, bbox, *fill_rule, |c| {
                scene_path(c, segments)
            });
        }

        SceneCommand::StrokePath {
            segments,
            color,
            stroke_width,
            closed,
            align,
            clip_fill_rule,
            stroke_linejoin,
            stroke_linecap,
            stroke_miter_limit,
        } => {
            if segments.len() < 2 || !path_segments_finite(segments) || !finite(*stroke_width) {
                return;
            }

            let aligned = *closed && !matches!(align, StrokeAlign::Center);

            content.save_state();
            apply_alpha(content, res, color);
            color::set_stroke(content, color);

            if aligned {
                match align {
                    StrokeAlign::Inside => {
                        if !scene_path(content, segments) {
                            content.end_path();
                            content.restore_state();
                            return;
                        }
                        apply_fill_rule(
                            content,
                            *clip_fill_rule,
                            |content| {
                                content.clip_nonzero();
                            },
                            |content| {
                                content.clip_even_odd();
                            },
                        );
                        content.end_path();
                    }
                    StrokeAlign::Outside => {
                        let (pw, ph) = page;
                        let m = pw.max(ph).max(1.0);
                        content.move_to(-m as f32, -m as f32);
                        content.line_to((pw + m) as f32, -m as f32);
                        content.line_to((pw + m) as f32, (ph + m) as f32);
                        content.line_to(-m as f32, (ph + m) as f32);
                        content.close_path();
                        if !scene_path(content, segments) {
                            content.end_path();
                            content.restore_state();
                            return;
                        }
                        content.clip_even_odd();
                        content.end_path();
                    }
                    StrokeAlign::Center => {}
                }
                let stroke_width = *stroke_width * 2.0;
                if !stroke_width.is_finite() || stroke_width > f64::from(f32::MAX) {
                    content.restore_state();
                    return;
                }
                content.set_line_width(stroke_width as f32);
                set_line_join(content, *stroke_linejoin);
                set_line_cap(content, *stroke_linecap);
                if !set_miter_limit(content, *stroke_miter_limit) {
                    content.restore_state();
                    return;
                }
                if scene_path(content, segments) {
                    content.stroke();
                } else {
                    content.end_path();
                }
            } else {
                if *stroke_width > f64::from(f32::MAX) {
                    content.restore_state();
                    return;
                }
                content.set_line_width(*stroke_width as f32);
                set_line_join(content, *stroke_linejoin);
                set_line_cap(content, *stroke_linecap);
                if !set_miter_limit(content, *stroke_miter_limit) {
                    content.restore_state();
                    return;
                }
                if scene_path(content, segments) {
                    content.stroke();
                } else {
                    content.end_path();
                }
            }
            content.restore_state();
        }

        SceneCommand::DrawGlyphRun {
            x,
            y,
            font_id,
            font_size,
            color,
            stroke_color,
            stroke_width,
            link,
            selectable,
            source_node_id: _,
            glyphs,
        } => {
            crate::pdf::glyph::emit_glyph_run(
                content,
                res,
                fonts,
                font_plan,
                crate::pdf::glyph::GlyphRun {
                    x: *x,
                    y: *y,
                    font_id,
                    font_size: *font_size,
                    color,
                    stroke_color: stroke_color.as_ref(),
                    stroke_width: *stroke_width,
                    link: link.as_deref(),
                    selectable: *selectable,
                    glyphs,
                },
            );
        }

        SceneCommand::DrawImage {
            x,
            y,
            w,
            h,
            asset_id,
            fit,
            pos_x,
            pos_y,
            opacity,
            clip_shape,
            src_rect,
            svg_style,
        } => {
            emit_image(
                content,
                res,
                fonts,
                assets,
                ImageDraw {
                    x: *x,
                    y: *y,
                    w: *w,
                    h: *h,
                    asset_id,
                    fit: *fit,
                    pos_x: *pos_x,
                    pos_y: *pos_y,
                    opacity: *opacity,
                    clip_shape,
                    src_rect: src_rect.as_ref(),
                    svg_style: *svg_style,
                },
            );
        }

        // SVG assets are pre-resolved to a raster in the raster backend; the
        // scene IR for the print scenarios never emits this variant. It is
        // matched explicitly (no silent wildcard) and deferred for PDF v0: a
        // faithful vector embedding would require an SVG→PDF path translator,
        // out of scope here. Documented limitation.
        SceneCommand::DrawSvgAsset { .. } => {}

        // ── Clip stack ────────────────────────────────────────────────────
        // PushClip → save the graphics state, install the rect clip, and clear
        // the path; the matching PopClip restores. This nests one q/Q level per
        // clip exactly like the raster backend's clip stack.
        SceneCommand::PushClip { x, y, w, h } => {
            content.save_state();
            content.rect(*x as f32, *y as f32, *w as f32, *h as f32);
            content.clip_nonzero();
            content.end_path();
        }
        // Rounded clip: the same q/Q nesting with the rounded path as `W n`.
        SceneCommand::PushClipRoundedRect { x, y, w, h, radius } => {
            content.save_state();
            if *w > 0.0 && *h > 0.0 {
                rounded_rect_path(content, *x, *y, *w, *h, [*radius; 4]);
            } else {
                // A degenerate box still needs a path before `W n`.
                content.rect(*x as f32, *y as f32, *w as f32, *h as f32);
            }
            content.clip_nonzero();
            content.end_path();
        }
        SceneCommand::PopClip => {
            content.restore_state();
        }

        // ── Transform stack ───────────────────────────────────────────────
        // Rotation about a pivot: save, translate to pivot, rotate, translate
        // back; the matching PopTransform restores.
        SceneCommand::PushTransform { angle_deg, cx, cy } => {
            content.save_state();
            let theta = (*angle_deg).to_radians();
            let (s, c) = (math::sin(theta) as f32, math::cos(theta) as f32);
            let (cx, cy) = (*cx as f32, *cy as f32);
            // Translate(cx,cy) · Rotate(θ) · Translate(-cx,-cy), as one matrix.
            content.transform([c, s, -s, c, cx - c * cx + s * cy, cy - s * cx - c * cy]);
        }
        SceneCommand::PushScaleTranslate { sx, sy, tx, ty } => {
            content.save_state();
            content.transform([*sx as f32, 0.0, 0.0, *sy as f32, *tx as f32, *ty as f32]);
        }
        SceneCommand::PushTransformMatrix { a, b, c, d, e, f } => {
            content.save_state();
            content.transform([
                *a as f32, *b as f32, *c as f32, *d as f32, *e as f32, *f as f32,
            ]);
        }
        SceneCommand::PopTransform => {
            content.restore_state();
        }

        // ── Compositing layers ────────────────────────────────────────────
        // Planned raster ranges handle group opacity and backdrop blends.
        // Identity layers retain their existing vector save/restore operators.
        SceneCommand::PushLayer { .. } => {
            content.save_state();
        }
        SceneCommand::PopLayer => {
            content.restore_state();
        }

        // ── Non-vector effect brackets ────────────────────────────────────
        // Planned ranges capture balanced effects. Malformed scopes and raster
        // errors retain their draw commands through these marker no-ops.
        SceneCommand::BeginShadow { .. } => {}
        SceneCommand::EndShadow => {}
        SceneCommand::BeginBlur { .. } => {}
        SceneCommand::EndBlur => {}
        SceneCommand::BeginFilter { .. } => {}
        SceneCommand::EndFilter => {}
        SceneCommand::BeginMask { .. } => {}
        SceneCommand::EndMask => {}
    }
}
