//! `pattern` node compilation: deterministic expansion of a single `motif`
//! template into many copies laid out across the pattern's bounds.
//!
//! Two layouts are supported. A `grid` tiles the motif on a fixed `spacing`
//! lattice (with optional positional `jitter`); a `scatter` places `count`
//! copies at seed-derived positions inside the bounds. Every instance is the
//! same motif node, compiled through [`compile_node`] with a translation offset
//! folded into its [`RenderCtx`] — exactly how a group translates its children —
//! so the motif keeps its own authored geometry and gains the instance offset.
//!
//! The pattern's own `fill` (solid or gradient), `radius` (uniform rounded
//! corners), and `stroke` + `stroke-width` paint a background panel behind the
//! clipped motif tiling; a pattern without any of those emits nothing new and is
//! byte-identical to before. A `shadow` (node attribute, else the style's)
//! brackets the clipped motif tiling as one unit; the background panel stays
//! unshadowed. The remaining visual properties (blur/mask/per-corner radii/…)
//! are inert. Placement is fully deterministic — instance
//! offsets are computed by `zenith_core::pattern_positions`, which is the single
//! source of truth shared with any other backend (e.g. the detach transaction op).

use std::collections::BTreeMap;

use zenith_core::{
    Diagnostic, PatternLayout, PatternNode, ResolvedToken, dim_to_px, pattern_positions,
};

use crate::ir::{Paint, SceneCommand};

use super::NodeCtx;
use super::RenderCtx;
use super::anchor::AnchorMap;
use super::boxes::StreamId;
use super::compile_node;
use super::paint::{
    NodeEffect, apply_gradient_opacity, emit_node_with_effects, resolve_property_color,
    resolve_property_gradient, resolve_property_shadow,
};
use super::style_prop;
use super::util::{
    AxisTarget, resolve_anchored_axis, resolve_geometry_px, resolve_property_dimension_px,
};

/// Compile a `pattern` node by expanding its motif across the resolved bounds.
///
/// Returns `0.0`: patterns are absolute-positioned and do not participate in
/// flow layout.
pub(in crate::compile) fn compile_pattern(
    pattern: &PatternNode,
    cx: NodeCtx,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) -> f64 {
    // Entire pattern excluded when visible=false.
    if pattern.visible == Some(false) {
        return 0.0;
    }

    // Resolve the bounds box in the pattern's LOCAL space (pre-ctx.dx/dy). The
    // instance contexts add `ctx.dx`/`ctx.dy` on top, so the box origin must be
    // local here. Validation already emitted diagnostics for bad geometry; when
    // anything fails to resolve to a usable box we render nothing and do NOT
    // re-emit (avoid duplicate diagnostics).
    let Some((bx, by, bw, bh)) = resolve_bounds(pattern, cx.anchors, cx.resolved) else {
        return 0.0;
    };

    // Validate the motif ONCE into scratch buffers at an origin instance ctx. A
    // broken motif must not spam one error per instance: if the scratch carries
    // any error, surface the scratch diagnostics once and render nothing.
    let mut scratch_cmds: Vec<SceneCommand> = Vec::new();
    let mut scratch_diags: Vec<Diagnostic> = Vec::new();
    let probe_ctx = RenderCtx {
        dx: ctx.dx + bx,
        dy: ctx.dy + by,
        ..ctx
    };
    // Pattern motif instances are self-contained and replicated inside a
    // `PushClip`; their connectors do NOT participate in page line-jumps, so the
    // recorded strokes go to a throwaway accumulator.
    // The probe records no boxes; each placed instance records below.
    compile_node(
        &pattern.motif,
        NodeCtx { boxes: None, ..cx },
        &mut scratch_cmds,
        &mut scratch_diags,
        &mut Vec::new(),
        probe_ctx,
    );
    if Diagnostic::has_errors(&scratch_diags) {
        diagnostics.extend(scratch_diags);
        return 0.0;
    }
    // Motif is renderable. Surface any non-error scratch diagnostics
    // (warnings / advisories) exactly once. The scratch commands are not used —
    // each instance is recompiled at its own offset below.
    diagnostics.extend(scratch_diags);

    // Paint the pattern's own background panel (fill + stroke) for the bounds
    // box BEFORE the clip, so the stroke outline is not clipped. A pattern with
    // neither a resolvable fill nor stroke emits nothing here, leaving the
    // command stream byte-identical to before.
    emit_background(pattern, cx, commands, diagnostics, ctx, (bx, by, bw, bh));

    // The clipped motif tiling is collected so a `shadow` (node attribute,
    // else the style's) brackets it as one unit. Without a shadow the tiling
    // lands verbatim.
    let mut tiles: Vec<SceneCommand> = Vec::new();
    let effect = pattern
        .shadow
        .as_ref()
        .or_else(|| style_prop(&pattern.style, cx.style_map, "shadow"))
        .and_then(|p| resolve_property_shadow(p, cx.resolved, &pattern.id))
        .map(NodeEffect::Shadow);
    // Commands the effect bracket puts before the tiling.
    let lead = usize::from(effect.is_some());

    // Clip every instance to the bounds box (in device space).
    tiles.push(SceneCommand::PushClip {
        x: ctx.dx + bx,
        y: ctx.dy + by,
        w: bw,
        h: bh,
    });

    let seed = pattern.seed.unwrap_or(0);
    let spacing = pattern
        .spacing
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit));

    let layout = PatternLayout {
        kind: pattern.kind.as_str(),
        bounds_w: bw,
        bounds_h: bh,
        spacing,
        count: pattern.count,
        seed,
        jitter: pattern.jitter.unwrap_or(0.0),
    };

    for (index, (ox, oy)) in pattern_positions(layout).into_iter().enumerate() {
        match cx.boxes {
            // Each instance records under `<pattern-id>/<index>/`, in a
            // recorder that knows the transform open where `tiles` lands.
            Some(recorder) => {
                let nested = recorder.nested(StreamId::of(commands), commands, lead);
                let instance_cx = NodeCtx {
                    boxes: Some(&nested),
                    ..cx
                };
                emit_instance(pattern, instance_cx, &mut tiles, ctx, bx + ox, by + oy);
                recorder.absorb(nested, &format!("{}/{index}/", pattern.id));
            }
            None => emit_instance(pattern, cx, &mut tiles, ctx, bx + ox, by + oy),
        }
    }

    tiles.push(SceneCommand::PopClip);

    emit_node_with_effects(commands, tiles, effect, None);

    0.0
}

/// Paint the pattern's background panel (fill, then stroke on top) for the
/// bounds box in device space, using the pattern's OWN visual props.
///
/// `box_local` is `(bx, by, bw, bh)` in LOCAL space; device coords add
/// `ctx.dx`/`ctx.dy`. The fill honors a solid color or a gradient token; the
/// stroke honors a color + width (defaulting to 1px like the rect compiler).
/// `radius` resolves to a UNIFORM rounded corner (absent/≤0 → sharp). The node
/// opacity (× `ctx.opacity`) scales solid-color alpha and gradient stops.
///
/// Emits nothing when neither fill nor stroke resolves → byte-identical to a
/// pattern that never carried these props. The shadow brackets the motif tiling
/// in [`compile_pattern`]; blur/mask/per-corner radii remain inert.
fn emit_background(
    pattern: &PatternNode,
    cx: NodeCtx,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
    box_local: (f64, f64, f64, f64),
) {
    let (bx, by, bw, bh) = box_local;
    let x = ctx.dx + bx;
    let y = ctx.dy + by;

    // Opacity cascade: node opacity × ctx opacity (no blend layer for the panel).
    let color_op = pattern.opacity.unwrap_or(1.0).clamp(0.0, 1.0) * ctx.opacity;

    // Uniform corner radius (absent/≤0 → sharp). Per-corner overrides are inert.
    let radius = resolve_property_dimension_px(pattern.radius.as_ref(), cx.resolved, 0.0);
    let is_rounded = radius > 0.0;
    // The rect compiler passes `None` for `radii` when there are no per-corner
    // overrides; match that so a uniform-radius panel is byte-identical in shape.
    let radii: Option<[f64; 4]> = None;

    // FILL (emitted first, under the stroke).
    if let Some(fill_prop) = pattern.fill.as_ref() {
        if let Some(mut gradient) = resolve_property_gradient(fill_prop, cx.resolved, &pattern.id) {
            apply_gradient_opacity(&mut gradient, color_op, 1.0);
            let paint = Paint::Gradient(gradient);
            if is_rounded {
                commands.push(SceneCommand::FillRoundedRect {
                    x,
                    y,
                    w: bw,
                    h: bh,
                    radius,
                    radii,
                    paint,
                });
            } else {
                commands.push(SceneCommand::FillRect {
                    x,
                    y,
                    w: bw,
                    h: bh,
                    paint,
                });
            }
        } else if let Some(mut color) =
            resolve_property_color(fill_prop, cx.resolved, diagnostics, &pattern.id)
        {
            color.a = (color.a as f64 * color_op).round() as u8;
            let paint = Paint::solid(color);
            if is_rounded {
                commands.push(SceneCommand::FillRoundedRect {
                    x,
                    y,
                    w: bw,
                    h: bh,
                    radius,
                    radii,
                    paint,
                });
            } else {
                commands.push(SceneCommand::FillRect {
                    x,
                    y,
                    w: bw,
                    h: bh,
                    paint,
                });
            }
        }
    }

    // STROKE (emitted on top of the fill, centered on the bounds edge).
    if let Some(stroke_prop) = pattern.stroke.as_ref()
        && let Some(mut color) =
            resolve_property_color(stroke_prop, cx.resolved, diagnostics, &pattern.id)
    {
        color.a = (color.a as f64 * color_op).round() as u8;
        let stroke_width =
            resolve_property_dimension_px(pattern.stroke_width.as_ref(), cx.resolved, 1.0);
        if is_rounded {
            commands.push(SceneCommand::StrokeRoundedRect {
                x,
                y,
                w: bw,
                h: bh,
                radius,
                radii,
                color,
                stroke_width,
                stroke_dash: None,
                stroke_gap: None,
                stroke_linecap: None,
            });
        } else {
            commands.push(SceneCommand::StrokeRect {
                x,
                y,
                w: bw,
                h: bh,
                color,
                stroke_width,
                stroke_dash: None,
                stroke_gap: None,
                stroke_linecap: None,
            });
        }
    }
}

/// Resolve the pattern's bounds box `(bx, by, bw, bh)` in LOCAL (pre-ctx) space.
///
/// `w`/`h` must resolve to a positive px value; `x`/`y` default to `0.0` when
/// absent (honoring the anchor map like a leaf node). Returns `None` (render
/// nothing) when the box is unusable. No diagnostics are emitted — validation
/// already covered these cases.
fn resolve_bounds(
    pattern: &PatternNode,
    anchors: &AnchorMap,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Option<(f64, f64, f64, f64)> {
    let bw = resolve_geometry_px(pattern.w.as_ref(), resolved)?;
    let bh = resolve_geometry_px(pattern.h.as_ref(), resolved)?;
    if bw <= 0.0 || bh <= 0.0 {
        return None;
    }

    // Anchor-derived (x, y) fallback, mirroring the leaf compilers. A throwaway
    // diagnostics buffer absorbs any push from the helper: we never surface a
    // geometry diagnostic from here (validation owns that), and x/y default to
    // 0 when neither an explicit value nor an anchor is present.
    let anchor_xy = anchors.get(&pattern.id).copied();
    let mut sink: Vec<Diagnostic> = Vec::new();
    let bx = resolve_anchored_axis(
        AxisTarget {
            kind: "pattern",
            node_id: &pattern.id,
            axis: "x",
        },
        pattern.x.as_ref(),
        resolved,
        anchor_xy.map(|(ax, _)| ax),
        pattern.source_span,
        &mut sink,
    )
    .unwrap_or(0.0);
    let by = resolve_anchored_axis(
        AxisTarget {
            kind: "pattern",
            node_id: &pattern.id,
            axis: "y",
        },
        pattern.y.as_ref(),
        resolved,
        anchor_xy.map(|(_, ay)| ay),
        pattern.source_span,
        &mut sink,
    )
    .unwrap_or(0.0);

    Some((bx, by, bw, bh))
}

/// Compile one motif instance translated by `(ox, oy)` in LOCAL space. The
/// instance context folds `ctx.dx + ox` / `ctx.dy + oy` into the translation so
/// the motif renders at (its own authored x/y + the instance offset).
///
/// Per-instance diagnostics are routed to a local throwaway buffer; the motif
/// was already validated in `compile_pattern` (and any diagnostics surfaced once)
/// so accumulating them here would only produce duplicates and unbounded growth
/// proportional to instance count.
fn emit_instance(
    pattern: &PatternNode,
    cx: NodeCtx,
    commands: &mut Vec<SceneCommand>,
    ctx: RenderCtx,
    ox: f64,
    oy: f64,
) {
    let inst_ctx = RenderCtx {
        dx: ctx.dx + ox,
        dy: ctx.dy + oy,
        ..ctx
    };
    let mut throwaway: Vec<Diagnostic> = Vec::new();
    // Motif connectors are clipped, replicated furniture — excluded from page
    // line-jumps via a throwaway stroke accumulator.
    compile_node(
        &pattern.motif,
        cx,
        commands,
        &mut throwaway,
        &mut Vec::new(),
        inst_ctx,
    );
}
