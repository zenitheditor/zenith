//! `compile_text_core`: the sized `text` orchestrator. It resolves the node
//! style, takes the chain / markdown / tab-leader branches, picks the fast or
//! wrap emit, then applies the rotation, blend, effect, mask, and clip brackets.

use zenith_core::{Diagnostic, PropertyValue, TextNode};
use zenith_layout::TextDirection;

use crate::compile::RenderCtx;
use crate::compile::paint::{emit_node_with_effects, resolve_property_mask};
use crate::compile::style_prop;
use crate::compile::text::chain_member::render_chain_member;
use crate::compile::text::ctx::{ChainMemberPlace, ShapeEnv, TextCompileEnv};
use crate::compile::text::ink::ink_bounds;
use crate::compile::text::markdown_block::compile_markdown_blocks;
use crate::compile::text::measure::{font_size_px, resolve_text_families};
use crate::compile::text::overflow_mode::{ClipBox, TextOverflow, clip_commands_since};
use crate::compile::text::resolve_kerning_pairs;
use crate::compile::text::shape::{resolve_font_feature_set, resolve_letter_spacing};
use crate::compile::util::{blend_mode_ir, resolve_geometry_px, rotation_degrees};
use crate::ir::SceneCommand;

use super::super::overflow::{OverflowCheck, SizedOutcome, measure_overflow};
use super::effects::resolve_effect;
use super::line::emit_single_line;
use super::origin::resolve_origin;
use super::spans::{effective_spans, shape_spans};
use super::style::{BoxLayout, NodeStyle, resolve_glyph_stroke};
use super::tab_leader::render_tab_leader;
use super::valign::v_align_offset;
use super::wrapped::emit_wrapped;

/// Compile a `text` leaf node at its resolved font size (the layout engine:
/// wrap/fast/drop-cap/runaround/chain paths + the overflow clip bracket).
///
/// Pushes NO overflow diagnostic: the measured overflow rides back in
/// [`SizedOutcome::overflow`] so the autofit search can probe sizes without
/// recursing into its own diagnostics. [`super::super::fit::compile_text_sized`]
/// adds the diagnostics.
///
/// `SizedOutcome::height` is the laid-out content height in pixels
/// (`line_count * line_height`), which the flow-layout path in
/// [`crate::compile::container`] uses to advance its vertical cursor past a text
/// child that declares no explicit `h`. Early returns (invisible, missing/bad
/// geometry, empty spans) yield `0.0`.
pub(in crate::compile::text::text_node) fn compile_text_core(
    text: &TextNode,
    env: TextCompileEnv,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) -> SizedOutcome {
    if text.visible == Some(false) {
        return SizedOutcome::plain(0.0);
    }

    let anchor_xy = env.anchors.get(&text.id).copied();
    let Some((x_raw, y_raw)) = resolve_origin(text, anchor_xy, env.resolved, diagnostics) else {
        return SizedOutcome::plain(0.0);
    };
    // Group translation. `box_y` is the box top; `text_y` is the first line's
    // top, moved down by a `v-align` offset below.
    let text_x = x_raw + ctx.dx;
    let box_y = y_raw + ctx.dy;
    let mut text_y = box_y;

    // Resolved before the chain branch so the member gets it too.
    let glyph_stroke = resolve_glyph_stroke(text, env.resolved, diagnostics);

    // A chain member renders the lines the page-level chain pre-pass assigned to
    // its box, not its own spans. A continuation member (empty spans) renders
    // here too, so this precedes the empty-spans return.
    if text.chain.is_some()
        && let Some(assignment) = env.chains.get(&text.id)
    {
        return SizedOutcome::plain(render_chain_member(
            text,
            assignment,
            ChainMemberPlace {
                shape: ShapeEnv {
                    engine: env.engine,
                    fonts: env.fonts,
                },
                font_size: font_size_px(text, env.resolved, env.style_map),
                text_x,
                text_y,
                baseline_grid: ctx.baseline_grid,
                glyph_stroke,
            },
            env.resolved,
            commands,
            diagnostics,
        ));
    }

    // A non-chained node in the parsed-markdown side-channel renders as stacked
    // blocks. It follows the chain exit so chained markdown keeps its single-run
    // behavior. Every other node falls through unchanged.
    if text.chain.is_none()
        && let Some(blocks) = env.md_blocks.get(&text.id)
    {
        return SizedOutcome::plain(compile_markdown_blocks(
            text,
            blocks,
            env,
            commands,
            diagnostics,
            ctx,
        ));
    }

    // Nothing to draw.
    if text.spans.iter().all(|s| s.text.is_empty()) {
        return SizedOutcome::plain(0.0);
    }

    let spans = effective_spans(text, env.footnote_markers, diagnostics);
    let families = resolve_text_families(text, env.resolved, env.style_map, env.fonts, diagnostics);
    let font_size: f32 = font_size_px(text, env.resolved, env.style_map);

    if let Some(offset) = v_align_offset(text, &families, env, diagnostics) {
        text_y += offset;
    }

    // Opacity cascade. A non-normal blend carries it on the PushLayer and emits
    // glyph colors at full alpha. Otherwise glyph colors carry it.
    let node_opacity = text.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
    let blend = blend_mode_ir(text.blend_mode.as_deref());
    let layer_opacity = node_opacity * ctx.opacity;
    let color_opacity = if blend.is_some() { 1.0 } else { layer_opacity };

    // Node-level props with the style cascade: the per-span fallbacks.
    let fill_prop: Option<&PropertyValue> = text
        .fill
        .as_ref()
        .or_else(|| style_prop(&text.style, env.style_map, "fill"));
    let weight_prop: Option<&PropertyValue> = text
        .font_weight
        .as_ref()
        .or_else(|| style_prop(&text.style, env.style_map, "font-weight"));
    let features = resolve_font_feature_set(
        text.font_features.as_deref(),
        text.font_alternates.as_deref(),
        diagnostics,
        &text.id,
        text.source_span,
    );
    let letter_spacing_prop = text
        .letter_spacing
        .as_ref()
        .or_else(|| style_prop(&text.style, env.style_map, "letter-spacing"));
    let kerning_pairs = resolve_kerning_pairs(&text.kerning_pairs, env.resolved);
    // `direction="rtl"` shapes RTL and flips line layout. Anything else is LTR.
    let direction = match text.direction.as_deref() {
        Some("rtl") => TextDirection::Rtl,
        _ => TextDirection::Ltr,
    };

    let style = NodeStyle {
        text,
        env,
        ctx,
        families: &families,
        font_size,
        fill_prop,
        weight_prop,
        features: &features,
        letter_spacing_prop,
        letter_spacing_px: resolve_letter_spacing(letter_spacing_prop, env.resolved),
        kerning_pairs: &kerning_pairs,
        direction,
        glyph_stroke,
        node_opacity,
        blend,
        layer_opacity,
        color_opacity,
    };
    let box_w_opt = resolve_geometry_px(text.w.as_ref(), env.resolved);
    let box_h_opt = resolve_geometry_px(text.h.as_ref(), env.resolved);
    let layout = BoxLayout {
        text_x,
        text_y,
        box_w: box_w_opt,
        box_h: box_h_opt,
        align: text.align.as_deref().unwrap_or("start"),
        deco_thickness: (font_size as f64 / 14.0).max(1.0),
    };

    // Tab-leader mode (table-of-contents rows): taken only for a non-empty
    // `tab-leader`, so every other node is untouched.
    if let Some(leader) = text.tab_leader.as_deref().filter(|s| !s.is_empty()) {
        return SizedOutcome::plain(render_tab_leader(
            &style,
            leader,
            layout,
            commands,
            diagnostics,
        ));
    }

    let shaped = shape_spans(&style, &spans, diagnostics);
    let first_line_height = shaped.first_line_height();
    let total_advance = shaped.total_advance;
    let has_spans = !shaped.spans.is_empty();

    let needs_wrap = needs_wrap(text, &spans, box_w_opt, total_advance);

    // Rotation about the BOX center, only when both w and h are present.
    let text_rot = rotation_degrees(text.rotate.as_ref())
        .zip(box_w_opt)
        .zip(box_h_opt)
        .map(|((a, bw), bh)| (a, text_x + bw / 2.0, box_y + bh / 2.0));
    if let Some((angle_deg, cx, cy)) = text_rot {
        commands.push(SceneCommand::PushTransform { angle_deg, cx, cy });
    }
    // Blend layer: inside the rotation, outside the shadow.
    if let Some(blend_mode) = blend {
        commands.push(SceneCommand::PushLayer {
            opacity: layer_opacity,
            blend_mode: Some(blend_mode),
        });
    }

    // The effect and mask bracket the node's glyph draws. The draws go into
    // `commands` first, then are split off at `draw_start` and re-emitted.
    let effect = resolve_effect(text, env.resolved, has_spans);
    let mask = text.mask.as_ref().and_then(|p| {
        let mask_w = box_w_opt.unwrap_or(total_advance);
        let mask_h = box_h_opt.unwrap_or(first_line_height);
        resolve_property_mask(p, env.resolved, (text_x, box_y, mask_w, mask_h))
    });
    let draw_start = commands.len();

    let fit_line_count = if needs_wrap {
        emit_wrapped(&style, layout, &shaped, commands, diagnostics)
    } else {
        emit_single_line(&style, layout, shaped, commands);
        1
    };

    // Ink of the node's draws (measured only against a complete box).
    let ink = if box_w_opt.is_some() && box_h_opt.is_some() {
        commands
            .get(draw_start..)
            .and_then(|draws| ink_bounds(draws, style.shape()))
    } else {
        None
    };
    let overflow = measure_overflow(OverflowCheck {
        box_w_opt,
        box_h_opt,
        box_y,
        ink,
        fit_line_count,
        needs_wrap,
        total_advance,
        font_size,
    });

    // In `clip` mode (the default) overflowing content is clipped at the box
    // edge. Content that fits emits no bracket.
    let clip = match TextOverflow::from_attr(text.overflow.as_deref()) {
        TextOverflow::Clip => overflow.map(|f| ClipBox {
            x: text_x,
            y: box_y,
            w: f.box_w,
            h: f.box_h,
        }),
        TextOverflow::Visible | TextOverflow::Fit | TextOverflow::Autofit => None,
    };

    // Re-emit the draws through the effect/mask helper, inside the clip bracket.
    let draws = commands.split_off(draw_start);
    emit_node_with_effects(commands, draws, effect, mask);
    clip_commands_since(commands, draw_start, clip);

    if blend.is_some() {
        commands.push(SceneCommand::PopLayer);
    }
    if text_rot.is_some() {
        commands.push(SceneCommand::PopTransform);
    }

    // Line count times the shared line height. It reuses the quantities the
    // overflow measurement uses, so flow advance and fit detection agree.
    SizedOutcome {
        height: fit_line_count as f64 * first_line_height,
        overflow,
    }
}

/// Whether the node takes the wrap path instead of the fast single line.
///
/// The fast path needs no box width or a line that fits. A `bullet`,
/// `padding-left`, or `text-indent` lives only on the wrap path (marker and
/// hanging indent), so any of them forces it. A literal `\n` in any span is a
/// mandatory break the fast path would feed to the shaper as tofu, so it also
/// forces the wrap path.
fn needs_wrap(
    text: &TextNode,
    spans: &[zenith_core::TextSpan],
    box_w: Option<f64>,
    total_advance: f64,
) -> bool {
    let has_hanging = text.bullet.as_deref().is_some_and(|s| !s.is_empty())
        || text.padding_left.is_some()
        || text.text_indent.is_some();
    let has_mandatory_break = spans.iter().any(|s| s.text.contains('\n'));
    match box_w {
        Some(box_w) => total_advance > box_w || has_hanging || has_mandatory_break,
        None => has_mandatory_break,
    }
}
