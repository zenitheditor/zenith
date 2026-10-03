//! The hand-off from a sized node to the shared WRAP path.

use zenith_core::Diagnostic;

use crate::compile::text::wrap::{WrapEnv, WrapGeom, emit_wrap_path};
use crate::ir::SceneCommand;

use super::spans::ShapedSet;
use super::style::{BoxLayout, NodeStyle};

/// Wrap and emit `set`; returns the laid-out line count.
///
/// A node with a box width wraps to it. A width-less node reaches this path
/// only for a mandatory `\n` break, so it uses its natural content width and
/// only the explicit breaks apply. The shared shaping, packing, and emit
/// helpers are the same ones the chain distributor uses, so a wrapped node and
/// a chain member produce identical command streams.
pub(super) fn emit_wrapped(
    style: &NodeStyle,
    layout: BoxLayout,
    set: &ShapedSet,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) -> usize {
    let resolved_spans = set.spans.iter().map(|s| s.to_resolved()).collect();
    emit_wrap_path(
        style.text,
        resolved_spans,
        style.families,
        WrapEnv {
            env: style.shape(),
            resolved: style.env.resolved,
            node_boxes: style.env.node_boxes,
            node_fill_prop: style.fill_prop,
            node_weight_prop: style.weight_prop,
            color_opacity: style.color_opacity,
            ctx: style.ctx,
        },
        WrapGeom {
            text_x: layout.text_x,
            text_y: layout.text_y,
            box_w: layout.box_w.unwrap_or(set.total_advance),
            box_h_opt: layout.box_h,
            font_size: style.font_size,
            letter_spacing_px: style.letter_spacing_px,
            kerning_pairs: style.kerning_pairs,
            align: layout.align,
            deco_thickness: layout.deco_thickness,
            direction: style.direction,
            glyph_stroke: style.glyph_stroke,
        },
        commands,
        diagnostics,
    )
}
