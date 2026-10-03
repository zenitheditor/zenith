//! `frame` container compilation: box paint (fill, stroke, radius) under the
//! children, an optional clip, and rotation / blend / effect brackets. A frame
//! does not translate its children. Row / column / grid frames arrive here
//! already lowered to absolute geometry by [`crate::layout`].

use zenith_core::{Diagnostic, FrameNode, dim_to_px};

use crate::ir::SceneCommand;

use super::super::leaf::BoxGeom;
use super::super::paint::{
    NodeEffect, resolve_property_filter, resolve_property_mask, resolve_property_shadow,
};
use super::super::util::{
    blend_mode_ir, resolve_geometry_px, rotation_degrees, unsupported_unit_diag,
};
use super::super::{NodeCtx, RenderCtx, compile_node, style_prop};
use super::frame_paint::{frame_clip_command, frame_radius, push_frame_paint};
use super::wrap::emit_wrapped_container;

/// What the frame body draws besides its children: the painted and clipped
/// box (offset by the inherited `dx` / `dy`) and whether the clip is on.
#[derive(Clone, Copy)]
struct FrameShell {
    geom: BoxGeom,
    clip: bool,
}

// NOTE: compile_frame → compile_node → compile_frame recursion has no depth
// guard, consistent with the compile_group limitation in v0.
pub(in crate::compile) fn compile_frame(
    frame: &FrameNode,
    cx: NodeCtx,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    connector_strokes: &mut Vec<usize>,
    ctx: RenderCtx,
) {
    // Entire subtree excluded when visible=false (no PushClip emitted).
    if frame.visible == Some(false) {
        return;
    }

    // All four geometry dimensions are required for a frame clip rectangle.
    // Resolve them BEFORE pushing any PushClip to keep push/pop balanced.
    let (Some(x_dim), Some(y_dim), Some(w_dim), Some(h_dim)) =
        (&frame.x, &frame.y, &frame.w, &frame.h)
    else {
        diagnostics.push(Diagnostic::advisory(
            "scene.missing_geometry",
            format!(
                "frame '{}' is missing one or more geometry properties (x, y, w, h); \
                 skipped",
                frame.id
            ),
            frame.source_span,
            Some(frame.id.clone()),
        ));
        return;
    };

    let Some(frame_x) = resolve_geometry_px(Some(x_dim), cx.resolved) else {
        diagnostics.push(unsupported_unit_diag(
            "frame",
            &frame.id,
            "x",
            frame.source_span,
        ));
        return;
    };
    let Some(frame_y) = resolve_geometry_px(Some(y_dim), cx.resolved) else {
        diagnostics.push(unsupported_unit_diag(
            "frame",
            &frame.id,
            "y",
            frame.source_span,
        ));
        return;
    };
    let Some(frame_w) = resolve_geometry_px(Some(w_dim), cx.resolved) else {
        diagnostics.push(unsupported_unit_diag(
            "frame",
            &frame.id,
            "w",
            frame.source_span,
        ));
        return;
    };
    let Some(frame_h) = resolve_geometry_px(Some(h_dim), cx.resolved) else {
        diagnostics.push(unsupported_unit_diag(
            "frame",
            &frame.id,
            "h",
            frame.source_span,
        ));
        return;
    };

    // Rotation bracket — outermost, wrapping paint + clip + children. The clip
    // sits under the rotation, so it rotates with the frame.
    let frame_rot = rotation_degrees(frame.rotate.as_ref());
    if let Some(angle) = frame_rot {
        let cx_pivot = ctx.dx + frame_x + frame_w / 2.0;
        let cy_pivot = ctx.dy + frame_y + frame_h / 2.0;
        commands.push(SceneCommand::PushTransform {
            angle_deg: angle,
            cx: cx_pivot,
            cy: cy_pivot,
        });
    }

    // Blend-mode layer (inside the rotation, around paint + clip + children).
    // With a non-normal blend the frame draws into an offscreen layer that
    // composites back with the full opacity cascade, so the content inside the
    // layer draws at opacity 1 (the cascade applies once, at PopLayer). With no
    // blend the cascade multiplies into the content as before.
    let frame_opacity = frame.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
    let blend = blend_mode_ir(frame.blend_mode.as_deref());
    let child_opacity = match blend {
        Some(blend_mode) => {
            commands.push(SceneCommand::PushLayer {
                opacity: ctx.opacity * frame_opacity,
                blend_mode: Some(blend_mode),
            });
            1.0
        }
        None => ctx.opacity * frame_opacity,
    };

    // Attached visual effect (inside blend, wrapping paint + clip + children).
    // The entire frame ink (box paint plus clipped children) is affected as one
    // unit: a shadow follows the painted box and the children together.
    // Precedence matches leaf nodes: blur > shadow > filter.
    let blur_sigma = frame
        .blur
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit))
        .filter(|&s| s > 0.0);
    let effect: Option<NodeEffect> = if let Some(sigma) = blur_sigma {
        Some(NodeEffect::Blur(sigma))
    } else if let Some(shadows) = frame
        .shadow
        .as_ref()
        .or_else(|| style_prop(&frame.style, cx.style_map, "shadow"))
        .and_then(|p| resolve_property_shadow(p, cx.resolved, &frame.id))
    {
        Some(NodeEffect::Shadow(shadows))
    } else {
        frame
            .filter
            .as_ref()
            .and_then(|p| resolve_property_filter(p, cx.resolved, &frame.id))
            .map(NodeEffect::Filter)
    };
    let shell = FrameShell {
        geom: BoxGeom {
            x: ctx.dx + frame_x,
            y: ctx.dy + frame_y,
            w: frame_w,
            h: frame_h,
            radius: frame_radius(frame, cx),
            radii: None,
        },
        clip: frame.clips(),
    };
    let mask = frame.mask.as_ref().and_then(|p| {
        let g = shell.geom;
        resolve_property_mask(p, cx.resolved, (g.x, g.y, g.w, g.h))
    });

    let child_ctx = RenderCtx {
        opacity: child_opacity,
        dx: ctx.dx, // clip-only: no translation
        dy: ctx.dy, // clip-only: no translation
        // Page baseline grid cascades unchanged so all text shares one grid.
        baseline_grid: ctx.baseline_grid,
        page_origin: ctx.page_origin,
    };

    if effect.is_none() && mask.is_none() {
        compile_frame_body(
            frame,
            shell,
            cx,
            commands,
            diagnostics,
            connector_strokes,
            child_ctx,
        );
    } else {
        let mut draws = Vec::new();
        let mut local_connector_strokes = Vec::new();
        compile_frame_body(
            frame,
            shell,
            cx,
            &mut draws,
            diagnostics,
            &mut local_connector_strokes,
            child_ctx,
        );
        emit_wrapped_container(
            commands,
            draws,
            effect,
            mask,
            connector_strokes,
            local_connector_strokes,
        );
    }

    if blend.is_some() {
        commands.push(SceneCommand::PopLayer);
    }

    if frame_rot.is_some() {
        commands.push(SceneCommand::PopTransform);
    }
}

/// Emit the frame box paint (unclipped, under the children), then the clip
/// bracket around the children. The paint uses the children's opacity.
fn compile_frame_body(
    frame: &FrameNode,
    shell: FrameShell,
    cx: NodeCtx,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    connector_strokes: &mut Vec<usize>,
    child_ctx: RenderCtx,
) {
    push_frame_paint(
        frame,
        shell.geom,
        cx,
        child_ctx.opacity,
        commands,
        diagnostics,
    );
    if shell.clip {
        commands.push(frame_clip_command(shell.geom));
    }

    for child in &frame.children {
        compile_node(
            child,
            cx,
            commands,
            diagnostics,
            connector_strokes,
            child_ctx,
        );
    }

    if shell.clip {
        commands.push(SceneCommand::PopClip);
    }
}
