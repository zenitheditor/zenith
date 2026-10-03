//! The tab-leader (table-of-contents row) branch of a sized node.

use zenith_core::Diagnostic;

use crate::compile::text::ctx::TabLeaderArgs;
use crate::compile::text::tableader::compile_tab_leader;
use crate::ir::SceneCommand;

use super::style::{BoxLayout, NodeStyle};

/// Render the leader rows; returns the laid-out height.
///
/// Under a non-normal blend the rows draw into a compositing layer that carries
/// the opacity cascade. The inner emit then runs at full alpha (node opacity
/// `1.0`, ctx opacity `1.0`, dx/dy/grid kept). With no blend the rows draw
/// directly.
pub(super) fn render_tab_leader(
    style: &NodeStyle,
    leader: &str,
    layout: BoxLayout,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) -> f64 {
    let mut ctx = style.ctx;
    let mut node_opacity = style.node_opacity;
    if let Some(blend_mode) = style.blend {
        commands.push(SceneCommand::PushLayer {
            opacity: style.layer_opacity,
            blend_mode: Some(blend_mode),
        });
        ctx.opacity = 1.0;
        node_opacity = 1.0;
    }
    let height = compile_tab_leader(
        style.text,
        leader,
        style.families,
        TabLeaderArgs {
            font_size: style.font_size,
            features: style.features,
            kerning_pairs: style.kerning_pairs,
            letter_spacing_px: style.letter_spacing_px,
            node_fill_prop: style.fill_prop,
            node_weight_prop: style.weight_prop,
            node_opacity,
            resolved: style.env.resolved,
            env: style.shape(),
            text_x: layout.text_x,
            text_y: layout.text_y,
            ctx,
            glyph_stroke: style.glyph_stroke,
        },
        commands,
        diagnostics,
    );
    if style.blend.is_some() {
        commands.push(SceneCommand::PopLayer);
    }
    height
}
