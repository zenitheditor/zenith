//! `rect` and `ellipse` leaf-node compilation.

use std::collections::BTreeMap;

use std::borrow::Cow;

use zenith_core::{
    DataContext, Diagnostic, EllipseNode, PropertyValue, RectNode, ResolvedToken, Style, dim_to_px,
};

use crate::ir::{Paint, SceneCommand};

use super::super::RenderCtx;
use super::super::anchor::AnchorMap;
use super::super::data_resolve::resolve_data_prop;
use super::super::paint::{
    NodeEffect, apply_gradient_opacity, emit_node_with_effects, resolve_property_color,
    resolve_property_filter, resolve_property_gradient, resolve_property_mask,
    resolve_property_shadow,
};
use super::super::style_prop;
use super::super::util::{
    blend_mode_ir, missing_geometry_diag, resolve_anchored_axis, resolve_property_dimension_px,
    rotation_degrees, unsupported_unit_diag,
};
use super::common::resolve_dash_params;

/// Read-only environment references shared by both `compile_rect` and
/// `compile_ellipse`. Bundled into a `Copy` struct so callers pass one
/// argument instead of four and the value cascades cheaply.
#[derive(Clone, Copy)]
pub(in crate::compile) struct RectEllipseEnv<'a> {
    pub resolved: &'a BTreeMap<String, ResolvedToken>,
    pub style_map: &'a BTreeMap<&'a str, &'a Style>,
    pub anchors: &'a AnchorMap,
    pub data: Option<&'a DataContext>,
}

/// Compile a `rect` leaf node.
pub(in crate::compile) fn compile_rect(
    rect: &RectNode,
    env: RectEllipseEnv<'_>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) {
    let RectEllipseEnv {
        resolved,
        style_map,
        anchors,
        data,
    } = env;
    // Skip invisible rects.
    if rect.visible == Some(false) {
        return;
    }

    // Resolve geometry — w and h are always required. x and y may be
    // derived from a page-relative anchor when absent.
    let (Some(w_dim), Some(h_dim)) = (&rect.w, &rect.h) else {
        diagnostics.push(missing_geometry_diag("rect", &rect.id, rect.source_span));
        return;
    };
    let Some(w) = dim_to_px(w_dim.value, &w_dim.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "rect",
            &rect.id,
            "w",
            rect.source_span,
        ));
        return;
    };
    let Some(h) = dim_to_px(h_dim.value, &h_dim.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "rect",
            &rect.id,
            "h",
            rect.source_span,
        ));
        return;
    };

    // Anchor-derived (x, y): look up the pre-pass map when x or y is absent.
    let anchor_xy = anchors.get(&rect.id).copied();

    let Some(x_raw) = resolve_anchored_axis(
        "rect",
        &rect.id,
        "x",
        rect.x.as_ref(),
        anchor_xy.map(|(ax, _)| ax),
        rect.source_span,
        diagnostics,
    ) else {
        return;
    };
    let Some(y_raw) = resolve_anchored_axis(
        "rect",
        &rect.id,
        "y",
        rect.y.as_ref(),
        anchor_xy.map(|(_, ay)| ay),
        rect.source_span,
        diagnostics,
    ) else {
        return;
    };

    // Apply group translation offset.
    let x = x_raw + ctx.dx;
    let y = y_raw + ctx.dy;

    // Apply node opacity then cascade ctx.opacity on top.
    let node_opacity = rect.opacity.unwrap_or(1.0).clamp(0.0, 1.0);

    // Blend-mode layer. When the node specifies a non-normal blend, the whole
    // opacity cascade (`node_opacity * ctx.opacity`) rides on the PushLayer and
    // is applied once, at PopLayer; the colors are then emitted at full alpha
    // (`color_op == 1.0`) so opacity is not double-counted. With no blend (the
    // common case) the layer is absent and `color_op` keeps the prior
    // `node_opacity * ctx.opacity`, leaving the command stream byte-identical.
    let blend = blend_mode_ir(rect.blend_mode.as_deref());
    let (layer_op, color_op) = match blend {
        Some(_) => (node_opacity * ctx.opacity, 1.0),
        None => (1.0, node_opacity * ctx.opacity),
    };

    // Resolve corner radius (optional; 0.0 when absent). Node-local
    // overrides style.
    let radius_prop = rect
        .radius
        .clone()
        .or_else(|| style_prop(&rect.style, style_map, "radius").cloned());
    let radius = resolve_property_dimension_px(radius_prop.as_ref(), resolved, 0.0);

    // Per-corner radius overrides. When any corner prop is present, build the
    // `radii` array (each corner falls back to uniform `radius`). When NO
    // corner prop is present, `radii` stays `None` → byte-identical to before.
    let has_corner_props = rect.radius_tl.is_some()
        || rect.radius_tr.is_some()
        || rect.radius_br.is_some()
        || rect.radius_bl.is_some();
    let radii: Option<[f64; 4]> = if has_corner_props {
        let tl = rect.radius_tl.as_ref().map_or(radius, |p| {
            resolve_property_dimension_px(Some(p), resolved, radius)
        });
        let tr = rect.radius_tr.as_ref().map_or(radius, |p| {
            resolve_property_dimension_px(Some(p), resolved, radius)
        });
        let br = rect.radius_br.as_ref().map_or(radius, |p| {
            resolve_property_dimension_px(Some(p), resolved, radius)
        });
        let bl = rect.radius_bl.as_ref().map_or(radius, |p| {
            resolve_property_dimension_px(Some(p), resolved, radius)
        });
        Some([tl, tr, br, bl])
    } else {
        None
    };

    // A rect is "rounded" when the uniform radius or any per-corner override > 0.
    let is_rounded = radius > 0.0 || radii.as_ref().is_some_and(|a| a.iter().any(|&v| v > 0.0));

    // Rotation bracket (outermost). PushTransform is only emitted when
    // rotate is non-zero; unrotated rects are byte-identical to before.
    let rot = rotation_degrees(rect.rotate.as_ref());
    if let Some(angle) = rot {
        let cx = x + w / 2.0;
        let cy = y + h / 2.0;
        commands.push(SceneCommand::PushTransform {
            angle_deg: angle,
            cx,
            cy,
        });
    }

    // BLEND-MODE layer bracket (inside the rotation, outside the shadow). Only
    // emitted for a non-normal blend; the matching PopLayer rides the arm tail.
    if let Some(blend_mode) = blend {
        commands.push(SceneCommand::PushLayer {
            opacity: layer_op,
            blend_mode: Some(blend_mode),
        });
    }

    // BLUR / SHADOW / FILTER effect (innermost, behind fill+stroke). Blur wins
    // over shadow over filter when several are set: at most one effect is chosen.
    // The single winning effect plus the optional mask bracket the node's DRAWS
    // (FILL + STROKE), emitted via `emit_node_with_effects` below; the
    // stroke-outer / per-side borders are NOT part of the masked/effected draws
    // (they land after the effect bracket, exactly as before).
    let blur_sigma = rect
        .blur
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit))
        .filter(|&s| s > 0.0);
    let effect: Option<NodeEffect> = if let Some(sigma) = blur_sigma {
        Some(NodeEffect::Blur(sigma))
    } else if let Some(shadows) = rect
        .shadow
        .as_ref()
        .and_then(|p| resolve_property_shadow(p, resolved, &rect.id))
    {
        Some(NodeEffect::Shadow(shadows))
    } else {
        rect.filter
            .as_ref()
            .and_then(|p| resolve_property_filter(p, resolved, &rect.id))
            .map(NodeEffect::Filter)
    };

    // Resolve the optional node mask against the rect's page-absolute box.
    let mask = rect
        .mask
        .as_ref()
        .and_then(|p| resolve_property_mask(p, resolved, (x, y, w, h)));

    // Collect the node's DRAW commands (fill + stroke) into a local buffer so
    // the shared helper can bracket them with the effect and/or mask. With no
    // effect and no mask the helper extends `commands` verbatim → byte-identical.
    let mut draws: Vec<SceneCommand> = Vec::new();

    // FILL (emitted first, under the stroke) — node-local prop overrides
    // style cascade. A `(data)` ref is resolved first via `resolve_data_prop`
    // so the rest of the paint pipeline sees a plain `Literal` or `TokenRef`.
    let fill_raw = rect
        .fill
        .as_ref()
        .or_else(|| style_prop(&rect.style, style_map, "fill"));
    // Resolve any DataRef → Literal before paint resolution.
    // Hold the Cow to keep any owned value alive for the duration of the fill block.
    let fill_cow: Option<Cow<'_, PropertyValue>> =
        fill_raw.map(|pv| resolve_data_prop(pv, data, "fill", &rect.id, diagnostics));
    let fill_prop: Option<&PropertyValue> = fill_cow.as_deref();
    if let Some(fill_prop) = fill_prop {
        if let Some(mut gradient) = resolve_property_gradient(fill_prop, resolved, &rect.id) {
            apply_gradient_opacity(&mut gradient, color_op, 1.0);
            let paint = Paint::Gradient(gradient);
            if is_rounded {
                draws.push(SceneCommand::FillRoundedRect {
                    x,
                    y,
                    w,
                    h,
                    radius,
                    radii,
                    paint,
                });
            } else {
                draws.push(SceneCommand::FillRect { x, y, w, h, paint });
            }
        } else if let Some(mut color) =
            resolve_property_color(fill_prop, resolved, diagnostics, &rect.id)
        {
            color.a = (color.a as f64 * color_op).round() as u8;
            let paint = Paint::solid(color);
            if is_rounded {
                draws.push(SceneCommand::FillRoundedRect {
                    x,
                    y,
                    w,
                    h,
                    radius,
                    radii,
                    paint,
                });
            } else {
                draws.push(SceneCommand::FillRect { x, y, w, h, paint });
            }
        }
    }

    // STROKE (emitted on top of the fill) — node-local prop overrides
    // style cascade.
    let stroke_prop = rect
        .stroke
        .as_ref()
        .or_else(|| style_prop(&rect.style, style_map, "stroke"));
    if let Some(stroke_prop) = stroke_prop
        && let Some(mut color) =
            resolve_property_color(stroke_prop, resolved, diagnostics, &rect.id)
    {
        color.a = (color.a as f64 * color_op).round() as u8;
        let sw = rect
            .stroke_width
            .clone()
            .or_else(|| style_prop(&rect.style, style_map, "stroke-width").cloned());
        let stroke_width = resolve_property_dimension_px(sw.as_ref(), resolved, 1.0);

        // Resolve dashed stroke parameters.
        let (stroke_dash, stroke_gap, stroke_linecap) = resolve_dash_params(
            rect.stroke_dash.as_ref(),
            rect.stroke_gap.as_ref(),
            rect.stroke_linecap.as_deref(),
            resolved,
        );

        // Stroke alignment offsets the stroke path relative to the box
        // edge by half the stroke width. `center` (default) straddles the
        // edge; `inside`/`outside` shift the whole stroked rectangle in or
        // out. The fill geometry above is unaffected.
        let half = stroke_width / 2.0;

        // Helper: adjust a single corner radius for stroke alignment. A
        // corner with radius 0 stays sharp (no adjustment).
        let adjust_inside = |v: f64| if v > 0.0 { (v - half).max(0.0) } else { 0.0 };
        let adjust_outside = |v: f64| if v > 0.0 { v + half } else { 0.0 };

        let (sx, sy, sw_geom, sh_geom, sradius, sradii) = match rect.stroke_alignment.as_deref() {
            Some("inside") => (
                x + half,
                y + half,
                w - stroke_width,
                h - stroke_width,
                adjust_inside(radius),
                radii.map(|[tl, tr, br, bl]| {
                    [
                        adjust_inside(tl),
                        adjust_inside(tr),
                        adjust_inside(br),
                        adjust_inside(bl),
                    ]
                }),
            ),
            Some("outside") => (
                x - half,
                y - half,
                w + stroke_width,
                h + stroke_width,
                adjust_outside(radius),
                radii.map(|[tl, tr, br, bl]| {
                    [
                        adjust_outside(tl),
                        adjust_outside(tr),
                        adjust_outside(br),
                        adjust_outside(bl),
                    ]
                }),
            ),
            // "center" (default) and any unrecognized value.
            _ => (x, y, w, h, radius, radii),
        };

        // Whether the stroked shape is still rounded after alignment.
        let stroke_is_rounded =
            sradius > 0.0 || sradii.as_ref().is_some_and(|a| a.iter().any(|&v| v > 0.0));

        // An inside-aligned stroke can shrink the box to nothing; skip
        // rather than emit a degenerate rectangle.
        if sw_geom > 0.0 && sh_geom > 0.0 {
            if stroke_is_rounded {
                draws.push(SceneCommand::StrokeRoundedRect {
                    x: sx,
                    y: sy,
                    w: sw_geom,
                    h: sh_geom,
                    radius: sradius,
                    radii: sradii,
                    color,
                    stroke_width,
                    stroke_dash,
                    stroke_gap,
                    stroke_linecap,
                });
            } else {
                draws.push(SceneCommand::StrokeRect {
                    x: sx,
                    y: sy,
                    w: sw_geom,
                    h: sh_geom,
                    color,
                    stroke_width,
                    stroke_dash,
                    stroke_gap,
                    stroke_linecap,
                });
            }
        }
    }

    // Emit the collected draws into `commands`, bracketed by the winning effect
    // and/or the mask. No effect + no mask → draws appended verbatim, so an
    // unmasked, uneffected rect is byte-identical to the prior command stream.
    emit_node_with_effects(commands, draws, effect, mask);

    // STROKE-OUTER: a second stroke painted OUTSIDE the rect geometry.
    // Emitted after shadow/blur bracket (so it lands on top of the shadow)
    // and outside the blend layer bracket (so it inherits the same opacity
    // cascade). Default-OFF: no commands emitted when stroke_outer is absent.
    if let Some(op) = rect.stroke_outer.as_ref()
        && let Some(mut oc) = resolve_property_color(op, resolved, diagnostics, &rect.id)
    {
        oc.a = (oc.a as f64 * color_op).round() as u8;
        let ow = resolve_property_dimension_px(rect.stroke_outer_width.as_ref(), resolved, 1.0);
        let half = ow / 2.0;
        let ox = x - half;
        let oy = y - half;
        let ogw = w + ow;
        let ogh = h + ow;
        if ogw > 0.0 && ogh > 0.0 {
            if is_rounded {
                // Expand corner radii outward by half the outer stroke width.
                // A corner with radius 0 stays sharp (no outset).
                let outer_expand = |v: f64| if v > 0.0 { v + half } else { 0.0 };
                let outer_radius = outer_expand(radius);
                let outer_radii = radii.map(|[tl, tr, br, bl]| {
                    [
                        outer_expand(tl),
                        outer_expand(tr),
                        outer_expand(br),
                        outer_expand(bl),
                    ]
                });
                commands.push(SceneCommand::StrokeRoundedRect {
                    x: ox,
                    y: oy,
                    w: ogw,
                    h: ogh,
                    radius: outer_radius,
                    radii: outer_radii,
                    color: oc,
                    stroke_width: ow,
                    stroke_dash: None,
                    stroke_gap: None,
                    stroke_linecap: None,
                });
            } else {
                commands.push(SceneCommand::StrokeRect {
                    x: ox,
                    y: oy,
                    w: ogw,
                    h: ogh,
                    color: oc,
                    stroke_width: ow,
                    stroke_dash: None,
                    stroke_gap: None,
                    stroke_linecap: None,
                });
            }
        }
    }

    // PER-SIDE BORDERS: straight StrokeLine along each present side.
    // Emitted after the outer stroke. Default-OFF: no commands when all four
    // border_* props are absent.
    let has_border = rect.border_top.is_some()
        || rect.border_bottom.is_some()
        || rect.border_left.is_some()
        || rect.border_right.is_some();
    if has_border {
        // border-width falls back to stroke-width then 1px.
        let bw_prop = rect
            .border_width
            .clone()
            .or_else(|| rect.stroke_width.clone());
        let bw = resolve_property_dimension_px(bw_prop.as_ref(), resolved, 1.0);
        for (prop, x1, y1, x2, y2) in [
            (rect.border_top.as_ref(), x, y, x + w, y),
            (rect.border_bottom.as_ref(), x, y + h, x + w, y + h),
            (rect.border_left.as_ref(), x, y, x, y + h),
            (rect.border_right.as_ref(), x + w, y, x + w, y + h),
        ] {
            if let Some(sp) = prop
                && let Some(mut sc) = resolve_property_color(sp, resolved, diagnostics, &rect.id)
            {
                sc.a = (sc.a as f64 * color_op).round() as u8;
                commands.push(SceneCommand::StrokeLine {
                    x1,
                    y1,
                    x2,
                    y2,
                    color: sc,
                    stroke_width: bw,
                    stroke_dash: None,
                    stroke_gap: None,
                    stroke_linecap: None,
                });
            }
        }
    }

    if blend.is_some() {
        commands.push(SceneCommand::PopLayer);
    }

    if rot.is_some() {
        commands.push(SceneCommand::PopTransform);
    }
}

/// Compile an `ellipse` leaf node.
pub(in crate::compile) fn compile_ellipse(
    ellipse: &EllipseNode,
    env: RectEllipseEnv<'_>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) {
    let RectEllipseEnv {
        resolved,
        style_map,
        anchors,
        data,
    } = env;
    // Skip invisible ellipses.
    if ellipse.visible == Some(false) {
        return;
    }

    // Resolve geometry — w and h are always required. x and y may be
    // derived from a page-relative anchor when absent.
    let (Some(w_dim), Some(h_dim)) = (&ellipse.w, &ellipse.h) else {
        diagnostics.push(missing_geometry_diag(
            "ellipse",
            &ellipse.id,
            ellipse.source_span,
        ));
        return;
    };
    let Some(w) = dim_to_px(w_dim.value, &w_dim.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "ellipse",
            &ellipse.id,
            "w",
            ellipse.source_span,
        ));
        return;
    };
    let Some(h) = dim_to_px(h_dim.value, &h_dim.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "ellipse",
            &ellipse.id,
            "h",
            ellipse.source_span,
        ));
        return;
    };

    // Anchor-derived (x, y): look up the pre-pass map when x or y is absent.
    let anchor_xy = anchors.get(&ellipse.id).copied();

    let Some(x_raw) = resolve_anchored_axis(
        "ellipse",
        &ellipse.id,
        "x",
        ellipse.x.as_ref(),
        anchor_xy.map(|(ax, _)| ax),
        ellipse.source_span,
        diagnostics,
    ) else {
        return;
    };
    let Some(y_raw) = resolve_anchored_axis(
        "ellipse",
        &ellipse.id,
        "y",
        ellipse.y.as_ref(),
        anchor_xy.map(|(_, ay)| ay),
        ellipse.source_span,
        diagnostics,
    ) else {
        return;
    };

    // Apply group translation offset.
    let x = x_raw + ctx.dx;
    let y = y_raw + ctx.dy;

    // Apply node opacity then cascade ctx.opacity on top.
    let node_opacity = ellipse.opacity.unwrap_or(1.0).clamp(0.0, 1.0);

    // Blend-mode layer (see compile_rect for the opacity-split rationale). With
    // no blend, `color_op == node_opacity * ctx.opacity` → byte-identical.
    let blend = blend_mode_ir(ellipse.blend_mode.as_deref());
    let (layer_op, color_op) = match blend {
        Some(_) => (node_opacity * ctx.opacity, 1.0),
        None => (1.0, node_opacity * ctx.opacity),
    };

    // Resolve independent semi-axis overrides. When absent → `None` → byte-identical
    // to inscribed-ellipse behavior (renderer uses w/h directly).
    let rx: Option<f64> = ellipse.rx.as_ref().and_then(|p| {
        let v = resolve_property_dimension_px(Some(p), resolved, 0.0);
        if v > 0.0 { Some(v) } else { None }
    });
    let ry: Option<f64> = ellipse.ry.as_ref().and_then(|p| {
        let v = resolve_property_dimension_px(Some(p), resolved, 0.0);
        if v > 0.0 { Some(v) } else { None }
    });

    // Rotation bracket (outermost). PushTransform only when rotate ≠ 0.
    let rot = rotation_degrees(ellipse.rotate.as_ref());
    if let Some(angle) = rot {
        let cx = x + w / 2.0;
        let cy = y + h / 2.0;
        commands.push(SceneCommand::PushTransform {
            angle_deg: angle,
            cx,
            cy,
        });
    }

    // BLEND-MODE layer bracket (inside rotation, outside shadow).
    if let Some(blend_mode) = blend {
        commands.push(SceneCommand::PushLayer {
            opacity: layer_op,
            blend_mode: Some(blend_mode),
        });
    }

    // BLUR / SHADOW / FILTER effect (behind fill+stroke). Blur > shadow > filter;
    // at most one is chosen. The winning effect plus the optional mask bracket
    // the node's DRAWS (FILL + STROKE) via `emit_node_with_effects` below.
    let blur_sigma = ellipse
        .blur
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit))
        .filter(|&s| s > 0.0);
    let effect: Option<NodeEffect> = if let Some(sigma) = blur_sigma {
        Some(NodeEffect::Blur(sigma))
    } else if let Some(shadows) = ellipse
        .shadow
        .as_ref()
        .and_then(|p| resolve_property_shadow(p, resolved, &ellipse.id))
    {
        Some(NodeEffect::Shadow(shadows))
    } else {
        ellipse
            .filter
            .as_ref()
            .and_then(|p| resolve_property_filter(p, resolved, &ellipse.id))
            .map(NodeEffect::Filter)
    };

    // Resolve the optional node mask against the ellipse's page-absolute box.
    let mask = ellipse
        .mask
        .as_ref()
        .and_then(|p| resolve_property_mask(p, resolved, (x, y, w, h)));

    // Collect the node's DRAW commands (fill + stroke) into a local buffer.
    let mut draws: Vec<SceneCommand> = Vec::new();

    // FILL (emitted first, under the stroke) — node-local prop overrides
    // style cascade. A `(data)` ref is resolved first via `resolve_data_prop`.
    let fill_raw = ellipse
        .fill
        .as_ref()
        .or_else(|| style_prop(&ellipse.style, style_map, "fill"));
    let fill_cow: Option<Cow<'_, PropertyValue>> =
        fill_raw.map(|pv| resolve_data_prop(pv, data, "fill", &ellipse.id, diagnostics));
    let fill_prop: Option<&PropertyValue> = fill_cow.as_deref();
    if let Some(fill_prop) = fill_prop {
        if let Some(mut gradient) = resolve_property_gradient(fill_prop, resolved, &ellipse.id) {
            apply_gradient_opacity(&mut gradient, color_op, 1.0);
            draws.push(SceneCommand::FillEllipse {
                x,
                y,
                w,
                h,
                rx,
                ry,
                paint: Paint::Gradient(gradient),
            });
        } else if let Some(mut color) =
            resolve_property_color(fill_prop, resolved, diagnostics, &ellipse.id)
        {
            color.a = (color.a as f64 * color_op).round() as u8;
            draws.push(SceneCommand::FillEllipse {
                x,
                y,
                w,
                h,
                rx,
                ry,
                paint: Paint::solid(color),
            });
        }
    }

    // STROKE (emitted on top of the fill) — node-local prop overrides
    // style cascade. Stroke is centered on the ellipse path in v0.
    let stroke_prop = ellipse
        .stroke
        .as_ref()
        .or_else(|| style_prop(&ellipse.style, style_map, "stroke"));
    if let Some(stroke_prop) = stroke_prop
        && let Some(mut color) =
            resolve_property_color(stroke_prop, resolved, diagnostics, &ellipse.id)
    {
        color.a = (color.a as f64 * color_op).round() as u8;
        let sw = ellipse
            .stroke_width
            .clone()
            .or_else(|| style_prop(&ellipse.style, style_map, "stroke-width").cloned());
        let stroke_width = resolve_property_dimension_px(sw.as_ref(), resolved, 1.0);

        // Resolve dashed stroke parameters.
        let (stroke_dash, stroke_gap, stroke_linecap) = resolve_dash_params(
            ellipse.stroke_dash.as_ref(),
            ellipse.stroke_gap.as_ref(),
            ellipse.stroke_linecap.as_deref(),
            resolved,
        );

        draws.push(SceneCommand::StrokeEllipse {
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
        });
    }

    // If neither fill nor stroke is present, the ellipse draws nothing —
    // no diagnostic needed (an invisible ellipse is valid in v0).

    // Emit the collected draws, bracketed by the winning effect and/or mask.
    // No effect + no mask → draws appended verbatim (byte-identical).
    emit_node_with_effects(commands, draws, effect, mask);

    if blend.is_some() {
        commands.push(SceneCommand::PopLayer);
    }

    if rot.is_some() {
        commands.push(SceneCommand::PopTransform);
    }
}
