//! `frame` box paint: the background fill and border stroke drawn under the
//! children, plus the clip command that matches the frame's corner radius.

use zenith_core::{Diagnostic, FrameNode};

use crate::ir::SceneCommand;

use super::super::leaf::{BoxGeom, BoxPaintEnv, BoxStroke, push_box_fill, push_box_stroke};
use super::super::util::resolve_property_dimension_px;
use super::super::{NodeCtx, style_prop};

/// Resolve the frame's corner radius in px: attribute, then style `radius`,
/// then 0.
pub(super) fn frame_radius(frame: &FrameNode, cx: NodeCtx) -> f64 {
    let prop = frame
        .radius
        .as_ref()
        .or_else(|| style_prop(&frame.style, cx.style_map, "radius"));
    resolve_property_dimension_px(prop, cx.resolved, 0.0)
}

/// Push the frame fill, then the frame stroke (center-aligned, like a rect's
/// default). Each attribute falls back to the frame style. `color_op` is the
/// opacity the frame's children receive.
pub(super) fn push_frame_paint(
    frame: &FrameNode,
    geom: BoxGeom,
    cx: NodeCtx,
    color_op: f64,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let env = BoxPaintEnv {
        resolved: cx.resolved,
        node_id: &frame.id,
        color_op,
    };
    let fill = frame
        .fill
        .as_ref()
        .or_else(|| style_prop(&frame.style, cx.style_map, "fill"));
    if let Some(fill) = fill {
        push_box_fill(commands, geom, fill, env, diagnostics);
    }
    let stroke = frame
        .stroke
        .as_ref()
        .or_else(|| style_prop(&frame.style, cx.style_map, "stroke"));
    if let Some(stroke) = stroke {
        let width = frame
            .stroke_width
            .as_ref()
            .or_else(|| style_prop(&frame.style, cx.style_map, "stroke-width"));
        let stroke = BoxStroke {
            color: stroke,
            width: resolve_property_dimension_px(width, cx.resolved, 1.0),
            dash: None,
            gap: None,
            linecap: None,
            alignment: None,
        };
        push_box_stroke(commands, geom, stroke, env, diagnostics);
    }
}

/// The clip command for the frame box: a rounded clip when `radius > 0`,
/// otherwise the plain rect clip.
pub(super) fn frame_clip_command(geom: BoxGeom) -> SceneCommand {
    let BoxGeom {
        x, y, w, h, radius, ..
    } = geom;
    if radius > 0.0 {
        SceneCommand::PushClipRoundedRect { x, y, w, h, radius }
    } else {
        SceneCommand::PushClip { x, y, w, h }
    }
}
