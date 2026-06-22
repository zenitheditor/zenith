//! `frame` container compilation: clip-only (it does not translate children),
//! with optional rotation / blend / blur brackets and `flow` / `grid` layout.

use zenith_core::{Diagnostic, FrameNode, Node, dim_to_px};

use crate::ir::SceneCommand;

use super::super::util::{
    blend_mode_ir, resolve_property_dimension_px, rotation_degrees, unsupported_unit_diag,
};
use super::super::{RenderCtx, compile_node, style_prop};
use super::ContainerCtx;
use super::flow::{node_declared_h, node_declared_w, node_skipped_in_flow, with_flow_box};

/// The already-resolved frame box in page coordinates (pixels), passed to the
/// `flow`/`grid` layout helpers.
#[derive(Clone, Copy)]
struct FrameBox {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

// NOTE: compile_frame → compile_node → compile_frame recursion has no depth
// guard, consistent with the compile_group limitation in v0.
pub(in crate::compile) fn compile_frame(
    frame: &FrameNode,
    cx: ContainerCtx,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
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

    let Some(frame_x) = dim_to_px(x_dim.value, &x_dim.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "frame",
            &frame.id,
            "x",
            frame.source_span,
        ));
        return;
    };
    let Some(frame_y) = dim_to_px(y_dim.value, &y_dim.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "frame",
            &frame.id,
            "y",
            frame.source_span,
        ));
        return;
    };
    let Some(frame_w) = dim_to_px(w_dim.value, &w_dim.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "frame",
            &frame.id,
            "w",
            frame.source_span,
        ));
        return;
    };
    let Some(frame_h) = dim_to_px(h_dim.value, &h_dim.unit) else {
        diagnostics.push(unsupported_unit_diag(
            "frame",
            &frame.id,
            "h",
            frame.source_span,
        ));
        return;
    };

    // Rotation bracket — outermost, wrapping PushClip + children + PopClip.
    // v0 limitation: the clip rectangle below stays axis-aligned even when the
    // frame is rotated; rotated children may extend past the axis-aligned clip.
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

    // Blend-mode layer (inside the rotation, around the clip + children). When a
    // non-normal blend is active the children render into an offscreen layer that
    // composites back with the frame's opacity cascade; the children therefore
    // inherit `ctx.opacity` UNMULTIPLIED (the layer carries the frame opacity so
    // it is not double-counted). With no blend the cascade is unchanged and the
    // command stream is byte-identical.
    let frame_opacity = frame.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
    let blend = blend_mode_ir(frame.blend_mode.as_deref());
    let child_opacity = match blend {
        Some(blend_mode) => {
            commands.push(SceneCommand::PushLayer {
                opacity: ctx.opacity * frame_opacity,
                blend_mode: Some(blend_mode),
            });
            ctx.opacity
        }
        None => ctx.opacity * frame_opacity,
    };

    // BLUR bracket (inside blend, wrapping clip+children). Opened here so the
    // entire frame ink (clip + composited children) is blurred as a unit.
    let blur_sigma = frame
        .blur
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit))
        .filter(|&s| s > 0.0);
    let has_blur = blur_sigma.is_some();
    if let Some(sigma) = blur_sigma {
        commands.push(SceneCommand::BeginBlur { radius: sigma });
    }

    // Clip rectangle is the frame's own bbox.
    commands.push(SceneCommand::PushClip {
        x: frame_x,
        y: frame_y,
        w: frame_w,
        h: frame_h,
    });

    // Frame clips only — it does NOT translate children (dx/dy unchanged).
    // Opacity cascades into all descendant alphas exactly as group does.
    let child_ctx = RenderCtx {
        opacity: child_opacity,
        dx: ctx.dx, // clip-only: no translation
        dy: ctx.dy, // clip-only: no translation
        // Page baseline grid cascades unchanged so all text shares one grid.
        baseline_grid: ctx.baseline_grid,
    };

    match frame.layout.as_deref() {
        Some("flow") => {
            compile_frame_flow(
                frame,
                FrameBox {
                    x: frame_x,
                    y: frame_y,
                    w: frame_w,
                    h: frame_h,
                },
                cx,
                commands,
                diagnostics,
                child_ctx,
            );
        }
        Some("grid") => {
            compile_frame_grid(
                frame,
                FrameBox {
                    x: frame_x,
                    y: frame_y,
                    w: frame_w,
                    h: frame_h,
                },
                cx,
                commands,
                diagnostics,
                child_ctx,
            );
        }
        _ => {
            // Absolute (clip-only) model: children render at their own page coords.
            for child in &frame.children {
                compile_node(
                    child,
                    cx.resolved,
                    cx.style_map,
                    cx.components,
                    cx.fonts,
                    cx.engine,
                    commands,
                    diagnostics,
                    cx.chains,
                    cx.flows,
                    cx.anchors,
                    cx.field_ctx,
                    child_ctx,
                );
            }
        }
    }

    commands.push(SceneCommand::PopClip);

    if has_blur {
        commands.push(SceneCommand::EndBlur);
    }

    if blend.is_some() {
        commands.push(SceneCommand::PopLayer);
    }

    if frame_rot.is_some() {
        commands.push(SceneCommand::PopTransform);
    }
    // Frame emits no fill of its own in v0.
}

/// Resolve `padding` and `gap` from a frame's style; both default to `0.0`.
fn frame_pad_gap(frame: &FrameNode, cx: ContainerCtx) -> (f64, f64) {
    let pad = resolve_property_dimension_px(
        &style_prop(&frame.style, cx.style_map, "padding").cloned(),
        cx.resolved,
        0.0,
    );
    let gap = resolve_property_dimension_px(
        &style_prop(&frame.style, cx.style_map, "gap").cloned(),
        cx.resolved,
        0.0,
    );
    (pad, gap)
}

/// Lay a flow-frame's children out as a vertical stack inside its padded
/// content box, compiling each at the injected absolute coordinates.
///
/// Triggered only when `frame.layout == Some("flow")`. `frame_x`/`frame_y`/
/// `frame_w` are the already-resolved frame box in page coordinates (the same
/// values used for the surrounding `PushClip`). Children stack in source order
/// with `gap` between them; `padding` insets the content box uniformly. Both
/// `padding` and `gap` are token-only dimension style props on the frame's
/// style, defaulting to `0.0` when absent.
fn compile_frame_flow(
    frame: &FrameNode,
    fbox: FrameBox,
    cx: ContainerCtx,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    child_ctx: RenderCtx,
) {
    let FrameBox {
        x: frame_x,
        y: frame_y,
        w: frame_w,
        ..
    } = fbox;
    let (pad, gap) = frame_pad_gap(frame, cx);

    // Content box: uniform padding on all four sides.
    let content_left = frame_x + pad;
    let content_top = frame_y + pad;
    let content_w = (frame_w - 2.0 * pad).max(0.0);

    // Lay out children that participate (skip invisible and guide nodes) so a
    // trailing gap is only suppressed relative to the LAST laid-out child.
    let laid_out: Vec<&Node> = frame
        .children
        .iter()
        .filter(|c| !node_skipped_in_flow(c))
        .collect();
    let last_idx = laid_out.len().saturating_sub(1);

    let mut cursor_y = content_top;
    for (i, child) in laid_out.iter().enumerate() {
        // Cross-axis = start; child width = own declared `w` or the content
        // width. (A text child's own `align` still centers WITHIN its width.)
        let child_w = node_declared_w(child).unwrap_or(content_w);

        // Vertical extent: own declared `h` when present, else the MEASURED
        // intrinsic height returned by compiling the child (text/code wrapped
        // height; 0.0 for leaves with no declared height).
        let declared_h = node_declared_h(child);

        // Inject the absolute box onto a clone; compile with the SAME ctx
        // (dx/dy unchanged — injected coords are absolute page coords).
        let positioned = with_flow_box(child, content_left, cursor_y, child_w, declared_h);
        let measured_h = compile_node(
            &positioned,
            cx.resolved,
            cx.style_map,
            cx.components,
            cx.fonts,
            cx.engine,
            commands,
            diagnostics,
            cx.chains,
            cx.flows,
            cx.anchors,
            cx.field_ctx,
            child_ctx,
        );

        // Advance by the declared height when present, otherwise the measured
        // intrinsic height read back from the compile.
        let advance = declared_h.unwrap_or(measured_h);
        cursor_y += advance;
        if i != last_idx {
            cursor_y += gap;
        }
    }
}

/// Lay a grid-frame's children out into a `columns × rows` grid inside its
/// padded content box, compiling each at the injected absolute coordinates.
///
/// Triggered only when `frame.layout == Some("grid")`. `frame_x`/`frame_y`/
/// `frame_w`/`frame_h` are the already-resolved frame box in page coordinates
/// (the same values used for the surrounding `PushClip`). Participating children
/// (the same set the flow layout lays out: visible, non-guide) auto-place
/// row-major into the grid. Both `padding` and `gap` are token-only dimension
/// style props on the frame's style, defaulting to `0.0` when absent.
///
/// Cell sizing (uniform gutters of `gap`):
/// - `cols = frame.columns.unwrap_or(1).max(1)`
/// - `effective_rows = frame.rows.max(1)` or, when absent,
///   `ceil(n / cols).max(1)` so the grid grows to fit its children.
/// - `col_w = ((content_w - (cols-1)*gap) / cols).max(0.0)`
/// - `row_h = ((content_h - (effective_rows-1)*gap) / effective_rows).max(0.0)`
///
/// Unlike flow, every cell's height is FIXED (`Some(row_h)`) so an image child
/// with `fit="cover"` fills its cell.
fn compile_frame_grid(
    frame: &FrameNode,
    fbox: FrameBox,
    cx: ContainerCtx,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    child_ctx: RenderCtx,
) {
    let FrameBox {
        x: frame_x,
        y: frame_y,
        w: frame_w,
        h: frame_h,
    } = fbox;
    let (pad, gap) = frame_pad_gap(frame, cx);

    // Content box: uniform padding on all four sides.
    let content_left = frame_x + pad;
    let content_top = frame_y + pad;
    let content_w = (frame_w - 2.0 * pad).max(0.0);
    let content_h = (frame_h - 2.0 * pad).max(0.0);

    // Participating children: skip invisible and guide nodes (reuse flow helper).
    let participating: Vec<&Node> = frame
        .children
        .iter()
        .filter(|c| !node_skipped_in_flow(c))
        .collect();
    let n = participating.len();

    // Column / row counts (both guaranteed >= 1 so divisions are safe).
    let cols = frame.columns.unwrap_or(1).max(1) as usize;
    let effective_rows = frame
        .rows
        .map(|r| r.max(1) as usize)
        .unwrap_or_else(|| n.div_ceil(cols).max(1));

    // Uniform cell sizing with `gap` gutters between cells.
    let col_w = ((content_w - (cols - 1) as f64 * gap) / cols as f64).max(0.0);
    let row_h = ((content_h - (effective_rows - 1) as f64 * gap) / effective_rows as f64).max(0.0);

    for (i, child) in participating.iter().enumerate() {
        let col = i % cols;
        let row = i / cols;
        let cell_x = content_left + col as f64 * (col_w + gap);
        let cell_y = content_top + row as f64 * (row_h + gap);

        // Inject the FULL fixed cell box (always Some(row_h)) so e.g. an image
        // with fit="cover" fills its cell. Compile at absolute page coords
        // (dx/dy unchanged). The measured height is ignored — cells are fixed.
        let positioned = with_flow_box(child, cell_x, cell_y, col_w, Some(row_h));
        let _ = compile_node(
            &positioned,
            cx.resolved,
            cx.style_map,
            cx.components,
            cx.fonts,
            cx.engine,
            commands,
            diagnostics,
            cx.chains,
            cx.flows,
            cx.anchors,
            cx.field_ctx,
            child_ctx,
        );
    }
}
