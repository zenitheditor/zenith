//! The scene-walk driver ([`translate`]), effect-bracket classification, the
//! per-command emitter ([`emit_command`]), and image emission.

use pdf_writer::Content;
use zenith_core::{AssetProvider, FontProvider};
use zenith_scene::{
    FillRule, FitMode, ImageClip, Scene, SceneCommand, StrokeAlign,
    ir::{path_segments_bbox, path_segments_finite},
};

use crate::pdf::color;
use crate::pdf::content::draw::{
    apply_alpha, apply_fill_rule, fill_region, finite, rect_ok, set_dash, set_line_cap,
    set_line_join, set_miter_limit,
};
use crate::pdf::content::resources::{ALPHA_PREFIX, IMAGE_PREFIX, PageResources, name};
use crate::pdf::font::FontPlan;
use crate::pdf::geometry::{ellipse_path, poly_bbox, poly_path, rounded_rect_path, scene_path};
use crate::pdf::image::decode_for_pdf;

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

    // Inclusive effect buffer. The four non-vector effect brackets — blur,
    // drop-shadow, per-pixel color filter, and mask — have no vector PDF
    // primitive, so while one is open we BUFFER the WHOLE bracket INCLUSIVE (the
    // `Begin*`, its body, and the matching `End*`) instead of emitting, then
    // render it as a standalone sub-scene whose raster backend self-applies every
    // effect (see `embed_rasterized_region`). `depth` counts nested effect
    // brackets so the buffer closes only at the matching outermost `End*`; a
    // mask>blur nesting closes correctly because every inner `Begin*` bumps the
    // count and every `End*` lowers it. A bracket is intercepted only at top
    // level; once a buffer is open, any nested effect is just more buffered
    // content handled by that one sub-scene render. `None` means draws emit
    // directly.
    let mut effect_buf: Option<(u32, Vec<SceneCommand>)> = None;

    for cmd in &scene.commands {
        let is_open = is_effect_open(cmd);
        let is_close = is_effect_close(cmd);

        // While a bracket is open, buffer everything, tracking nesting depth, and
        // render the region when the outermost bracket closes.
        if let Some((depth, buffered)) = effect_buf.as_mut() {
            buffered.push(cmd.clone());
            if is_open {
                *depth += 1;
            } else if is_close {
                *depth = depth.saturating_sub(1);
                if *depth == 0
                    && let Some((_, region)) = effect_buf.take()
                {
                    crate::pdf::raster_embed::embed_rasterized_region(
                        &mut content,
                        &mut res,
                        &region,
                        page,
                        fonts,
                        assets,
                        font_plan,
                    );
                }
            }
            continue;
        }

        // No buffer open: a top-level effect-open starts one (the Begin is
        // buffered too so the sub-scene self-applies the effect). An empty
        // `BeginFilter` is a no-op — it stays vector and falls through to
        // `emit_command` (which no-ops it); blur/shadow/mask always open.
        if is_open && !is_empty_filter(cmd) {
            effect_buf = Some((1, vec![cmd.clone()]));
            continue;
        }

        emit_command(&mut content, &mut res, cmd, page, fonts, assets, font_plan);
    }

    (content, res)
}

/// A scene command's role in non-vector effect bracketing.
enum EffectBracket {
    /// Opens an offscreen capture (blur, shadow, filter, or mask).
    Open,
    /// Closes the innermost offscreen capture.
    Close,
    /// Not an effect bracket — a plain draw, clip, layer, or transform command.
    None,
}

/// Exhaustively classify a command's effect-bracket role. Every `SceneCommand`
/// variant is listed explicitly (no wildcard arm), so adding a new variant
/// forces a compile error here and can never be silently treated as a plain draw.
fn effect_bracket(cmd: &SceneCommand) -> EffectBracket {
    match cmd {
        // ── Effect openers ────────────────────────────────────────────────
        SceneCommand::BeginShadow { .. }
        | SceneCommand::BeginBlur { .. }
        | SceneCommand::BeginFilter { .. }
        | SceneCommand::BeginMask { .. } => EffectBracket::Open,

        // ── Effect closers ────────────────────────────────────────────────
        SceneCommand::EndShadow
        | SceneCommand::EndBlur
        | SceneCommand::EndFilter
        | SceneCommand::EndMask => EffectBracket::Close,

        // ── Plain draw / clip / layer / transform commands ─────────────────
        SceneCommand::FillRect { .. }
        | SceneCommand::StrokeRect { .. }
        | SceneCommand::FillRoundedRect { .. }
        | SceneCommand::StrokeRoundedRect { .. }
        | SceneCommand::FillEllipse { .. }
        | SceneCommand::StrokeEllipse { .. }
        | SceneCommand::StrokeLine { .. }
        | SceneCommand::FillPolygon { .. }
        | SceneCommand::StrokePolyline { .. }
        | SceneCommand::FillPath { .. }
        | SceneCommand::StrokePath { .. }
        | SceneCommand::DrawImage { .. }
        | SceneCommand::DrawSvgAsset { .. }
        | SceneCommand::DrawGlyphRun { .. }
        | SceneCommand::PushClip { .. }
        | SceneCommand::PushClipRoundedRect { .. }
        | SceneCommand::PopClip
        | SceneCommand::PushLayer { .. }
        | SceneCommand::PopLayer
        | SceneCommand::PushTransform { .. }
        | SceneCommand::PushScaleTranslate { .. }
        | SceneCommand::PushTransformMatrix { .. }
        | SceneCommand::PopTransform => EffectBracket::None,
    }
}

/// True for a command that opens a non-vector effect bracket (shadow, blur,
/// filter, or mask).
fn is_effect_open(cmd: &SceneCommand) -> bool {
    matches!(effect_bracket(cmd), EffectBracket::Open)
}

/// True for a command that closes a non-vector effect bracket — the matching
/// `End*` for each [`is_effect_open`] case.
fn is_effect_close(cmd: &SceneCommand) -> bool {
    matches!(effect_bracket(cmd), EffectBracket::Close)
}

/// True for a `BeginFilter` carrying no filters — a no-op bracket that must not
/// open a buffer (it stays vector). The compiler never emits empty filters; the
/// guard is preserved defensively. All other commands return false.
fn is_empty_filter(cmd: &SceneCommand) -> bool {
    matches!(cmd, SceneCommand::BeginFilter { filters } if filters.is_empty())
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
            src_rect: _,
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
            let (s, c) = (theta.sin() as f32, theta.cos() as f32);
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
        // Layer opacity is applied per-draw via the color alpha cascade already
        // resolved into each command's color in the scene IR, so a layer
        // bracket needs only a save/restore to scope any state it sets. (No
        // group transparency object in v0; matched explicitly, not dropped.)
        //
        // v0 limitation: the `blend_mode` field is ignored — the PDF backend has
        // no ExtGState soft-mask / blend-mode group, so blended content renders
        // source-over. Documented honest limitation (the PNG backend honors it).
        SceneCommand::PushLayer { .. } => {
            content.save_state();
        }
        SceneCommand::PopLayer => {
            content.restore_state();
        }

        // ── Non-vector effect brackets ────────────────────────────────────
        // `translate` intercepts every BALANCED effect bracket (blur, shadow,
        // filter, mask) before dispatch, buffering it inclusive and rendering it
        // as a self-applying sub-scene (see `embed_rasterized_region`). These
        // arms are therefore unreachable in normal flow; kept (no wildcard) for
        // exhaustiveness, and a no-op is the safe fallback so a malformed or
        // standalone End* / empty Begin* reaching here can never panic.
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

/// Borrow/scalar context for one [`SceneCommand::DrawImage`] emission, bundled
/// into a `Copy` struct so [`emit_image`] stays within the argument-count
/// budget without an `#[allow]`.
#[derive(Clone, Copy)]
struct ImageDraw<'a> {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    /// Stable asset id; resolved via `AssetProvider::by_id`.
    asset_id: &'a str,
    /// How the image scales to fill the box.
    fit: FitMode,
    /// Horizontal object-position anchor in `0.0..=100.0`.
    pos_x: f64,
    /// Vertical object-position anchor in `0.0..=100.0`.
    pos_y: f64,
    /// Effective opacity, `0.0..=1.0`.
    opacity: f64,
    /// Optional non-rectangular clip shape inscribed in the box.
    clip_shape: &'a Option<ImageClip>,
    /// SVG-only style overrides.
    svg_style: Option<zenith_scene::SvgStyle>,
}

fn emit_image(
    content: &mut Content,
    res: &mut PageResources,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    draw: ImageDraw<'_>,
) {
    let ImageDraw {
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
        svg_style,
    } = draw;
    if !rect_ok(x, y, w, h) {
        return;
    }
    let Some(asset) = assets.by_id(asset_id) else {
        return;
    };
    // Dispatch on asset kind. Raster images embed as a Flate XObject below; SVG
    // assets translate to native PDF vector operators (paths + shadings) via the
    // `svg` module — true vector output, not a rasterized embed. Font/Unknown
    // kinds are not drawable images.
    match asset.kind {
        zenith_core::AssetKind::Image => {}
        zenith_core::AssetKind::Svg => {
            crate::pdf::svg::emit_svg(
                content,
                res,
                fonts,
                &asset.bytes,
                crate::pdf::svg::SvgPlacement {
                    x,
                    y,
                    w,
                    h,
                    fit,
                    pos_x,
                    pos_y,
                    opacity,
                    clip_shape,
                    svg_style,
                },
            );
            return;
        }
        zenith_core::AssetKind::Font | zenith_core::AssetKind::Unknown(_) => return,
    }
    let Some(decoded) = decode_for_pdf(&asset.bytes) else {
        return;
    };
    let (sw, sh) = (f64::from(decoded.width), f64::from(decoded.height));
    if !(sw > 0.0 && sh > 0.0) {
        return;
    }

    // Fit transform (sx, sy, tx, ty) in scene space — identical math to the
    // raster backend's DrawImage arm.
    let (sx, sy, tx, ty) = match fit {
        FitMode::Stretch => (w / sw, h / sh, x, y),
        FitMode::Contain => {
            let s = (w / sw).min(h / sh);
            let (rw, rh) = (sw * s, sh * s);
            (
                s,
                s,
                x + (w - rw) * pos_x / 100.0,
                y + (h - rh) * pos_y / 100.0,
            )
        }
        FitMode::Cover => {
            let s = (w / sw).max(h / sh);
            let (rw, rh) = (sw * s, sh * s);
            (
                s,
                s,
                x - (rw - w) * pos_x / 100.0,
                y - (rh - h) * pos_y / 100.0,
            )
        }
        FitMode::None => (
            1.0,
            1.0,
            x - (sw - w) * pos_x / 100.0,
            y - (sh - h) * pos_y / 100.0,
        ),
    };
    if !finite(sx) || !finite(sy) || !finite(tx) || !finite(ty) || sx <= 0.0 || sy <= 0.0 {
        return;
    }

    let id = res.images.len();
    res.images.push(decoded);

    content.save_state();

    // Opacity via an ExtGState (image opacity is a separate factor from any
    // color alpha). 1.0 needs no state.
    let op = (opacity as f32).clamp(0.0, 1.0);
    if op < 1.0 {
        let a = (op * 255.0).round().clamp(0.0, 255.0) as u8;
        let aidx = res.intern_alpha(a);
        content.set_parameters(name(ALPHA_PREFIX, aidx).as_name());
    }

    // Box clip (rect or inscribed shape). The compiler also pushes a PushClip
    // box around images, but re-asserting the box here is harmless and makes
    // the non-rectangular shape clip self-contained.
    match clip_shape {
        None => {
            content.rect(x as f32, y as f32, w as f32, h as f32);
            content.clip_nonzero();
            content.end_path();
        }
        Some(ImageClip::Ellipse) => {
            ellipse_path(content, x, y, w, h, None, None);
            content.clip_nonzero();
            content.end_path();
        }
        Some(ImageClip::RoundedRect { radius }) => {
            rounded_rect_path(content, x, y, w, h, [*radius; 4]);
            content.clip_nonzero();
            content.end_path();
        }
    }

    // An image XObject is a 1×1 unit square in its own space; place it by
    // mapping that unit square onto the fitted box. PDF images are y-up, so we
    // flip within the placement matrix: image row 0 (top) must land at the box
    // top (smaller scene-y). The CTM below maps unit (u, v) → scene point
    // (tx + u*sw*sx, ty + (1-v)*sh*sy), i.e. scale_y is negative with a +height
    // translate, all composed with the page's outer flip.
    let iw = (sw * sx) as f32;
    let ih = (sh * sy) as f32;
    content.transform([iw, 0.0, 0.0, -ih, tx as f32, ty as f32 + ih]);
    content.x_object(name(IMAGE_PREFIX, id).as_name());

    content.restore_state();
}
