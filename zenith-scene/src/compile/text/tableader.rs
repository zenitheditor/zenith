//! Tab-leader (table-of-contents) rendering: rows split on the first `\t` into a
//! left segment (flushed to the box left edge) and a right segment (right-aligned
//! to the box right edge), with the gap between filled by a repeated leader glyph.

use zenith_core::{Diagnostic, FontProvider, FontStyle, TextNode};
use zenith_layout::{
    RustybuzzEngine, ShapeRequest, TextDirection, TextLayoutEngine, ZenithGlyphRun,
};

use crate::ir::{Color, SceneCommand};

use super::super::paint::resolve_property_color;
use super::super::util::{resolve_geometry_px, rotation_degrees};
use super::ctx::TabLeaderArgs;
use super::shape::{resolve_font_weight, run_to_scene_glyphs};

/// Horizontal breathing space, as a fraction of the leader glyph's advance,
/// left between the LEFT segment's right edge (and before the RIGHT segment's
/// left edge) and the run of leader glyphs. A deterministic, compact default.
const TAB_LEADER_GAP_FACTOR: f64 = 1.0;

/// A row's left/right text shaped into glyph runs, plus the summed advances.
struct TabLeaderRow {
    /// LEFT-segment runs (placed at the box left edge), left-to-right.
    left_runs: Vec<ZenithGlyphRun>,
    left_advance: f64,
    /// RIGHT-segment runs (right-aligned to the box right edge), left-to-right.
    /// Empty when the row carried no `\t` (left-aligned row, no leader).
    right_runs: Vec<ZenithGlyphRun>,
    right_advance: f64,
    /// Whether this row carried a tab (so a leader run + right segment apply).
    has_tab: bool,
}

/// Shape one TOC row's LEFT and RIGHT segments with the node font/size/weight.
///
/// The row is split on its FIRST `\t`; everything before is the LEFT segment,
/// everything after is the RIGHT segment. A row without a tab yields only a
/// LEFT segment (`has_tab == false`). Empty segments shape to zero runs/advance
/// safely. Deterministic: the same engine/fonts always produce the same runs.
fn shape_tab_leader_row(
    row: &str,
    families: &[String],
    font_size: f32,
    weight: u16,
    engine: &RustybuzzEngine,
    fonts: &dyn FontProvider,
) -> TabLeaderRow {
    let (left_text, right_text, has_tab) = match row.split_once('\t') {
        Some((l, r)) => (l, r, true),
        None => (row, "", false),
    };

    let shape_seg = |seg: &str| -> (Vec<ZenithGlyphRun>, f64) {
        if seg.is_empty() {
            return (Vec::new(), 0.0);
        }
        let req = ShapeRequest {
            text: seg,
            families,
            weight,
            style: FontStyle::Normal,
            font_size,
            // Tab-leader (TOC) rows are LTR in v0; RTL TOC is a follow-up.
            direction: TextDirection::Ltr,
        };
        match engine.shape_with_fallback(&req, fonts) {
            Ok(result) => {
                let adv: f64 = result.runs.iter().map(|r| r.advance_width as f64).sum();
                (result.runs, adv)
            }
            Err(_) => (Vec::new(), 0.0),
        }
    };

    let (left_runs, left_advance) = shape_seg(left_text);
    let (right_runs, right_advance) = shape_seg(right_text);

    TabLeaderRow {
        left_runs,
        left_advance,
        right_runs,
        right_advance,
        has_tab,
    }
}

/// Emit a sequence of glyph runs starting at pen `start_x` on baseline `y`, in
/// the node's resolved `color`. Runs are positioned left-to-right by their own
/// advances. Shared by the LEFT, RIGHT, and leader emission so the field mapping
/// lives in one place.
fn emit_tab_leader_runs(
    runs: &[ZenithGlyphRun],
    start_x: f64,
    y: f64,
    color: Color,
    glyph_stroke: (Option<Color>, Option<f64>),
    commands: &mut Vec<SceneCommand>,
) {
    let mut x = start_x;
    for run in runs {
        commands.push(SceneCommand::DrawGlyphRun {
            x,
            y,
            font_id: run.font_id.clone(),
            font_size: run.font_size,
            color,
            stroke_color: glyph_stroke.0,
            stroke_width: glyph_stroke.1,
            link: None,
            selectable: true,
            glyphs: run_to_scene_glyphs(run),
        });
        x += run.advance_width as f64;
    }
}

/// Render a text node in TAB-LEADER (table-of-contents) mode.
///
/// The combined span text is split into rows on `\n`. Each row is stacked by
/// `line_height` (baseline = `text_y + ascent + i*line_height`) like the
/// multi-line text/code paths. Within a row the FIRST `\t` splits a LEFT and a
/// RIGHT segment: LEFT is placed at the box left edge (`text_x`); RIGHT is
/// right-aligned so its right edge sits at the box right edge
/// (`text_x + box_w - right_advance`). The gap between LEFT's right edge (plus a
/// small breathing gap) and RIGHT's left edge (minus the same gap) is filled
/// with the leader glyph repeated: ONE leader glyph is shaped, the integer count
/// is `floor(gap / leader_advance)`, and the leaders are placed left-to-right
/// starting just after LEFT, spaced by `leader_advance` (documented choice). A
/// non-positive gap (or a zero leader advance) emits no leaders and an advisory
/// `text.overflow` warning, leaving LEFT and RIGHT to abut/overlap. A row with no
/// tab renders left-aligned with no leader.
///
/// Determinism: rows are processed in source order; the leader count is an
/// integer from a deterministic float division; runs are emitted in a fixed
/// order — so two runs are byte-identical. Returns the laid-out content height
/// (`row_count * line_height`).
pub(in crate::compile) fn compile_tab_leader(
    text: &TextNode,
    leader: &str,
    families: &[String],
    args: TabLeaderArgs,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) -> f64 {
    let TabLeaderArgs {
        font_size,
        node_fill_prop,
        node_weight_prop,
        node_opacity,
        resolved,
        env,
        text_x,
        text_y,
        ctx,
        glyph_stroke,
    } = args;
    let engine = env.engine;
    let fonts = env.fonts;

    // Combined source text across all spans (tab-leader mode treats the node as
    // one verbatim block, like a code node, so `\t`/`\n` keep their meaning).
    let combined: String = text.spans.iter().map(|s| s.text.as_str()).collect();
    if combined.is_empty() {
        return 0.0;
    }

    // Resolved node color (the whole TOC block shares one fill) and weight.
    let mut color = node_fill_prop
        .and_then(|fp| resolve_property_color(fp, resolved, diagnostics, &text.id))
        .unwrap_or(Color::srgb(0, 0, 0, 255));
    color.a = (color.a as f64 * node_opacity * ctx.opacity).round() as u8;
    let weight = resolve_font_weight(node_weight_prop, resolved, 400);

    // Box width is required to right-align the page number. Without it, a TOC
    // row cannot be laid out (no box edge to flush to) — surface an advisory and
    // render nothing rather than guessing a width.
    let Some(box_w) = resolve_geometry_px(text.w.as_ref(), resolved) else {
        diagnostics.push(Diagnostic::advisory(
            "scene.missing_geometry",
            format!(
                "text node '{}' uses tab-leader but has no box width; skipped",
                text.id
            ),
            text.source_span,
            Some(text.id.clone()),
        ));
        return 0.0;
    };

    // Shape ONE leader glyph once; its advance drives the fill count + spacing.
    let leader_req = ShapeRequest {
        text: leader,
        families,
        weight,
        style: FontStyle::Normal,
        font_size,
        // Tab-leader (TOC) mode is LTR in v0.
        direction: TextDirection::Ltr,
    };
    let leader_run = match engine.shape_with_fallback(&leader_req, fonts) {
        Ok(result) => result.runs.into_iter().next(),
        Err(_) => None,
    };
    let leader_advance = leader_run
        .as_ref()
        .map(|r| r.advance_width as f64)
        .unwrap_or(0.0);

    // Shape every row first so the shared ascent/line-height (from the first
    // non-empty run) define a uniform line grid for the whole block.
    let rows: Vec<TabLeaderRow> = combined
        .split('\n')
        .map(|row| shape_tab_leader_row(row, families, font_size, weight, engine, fonts))
        .collect();

    // Shared metrics from the first available run (left, right, or the leader).
    let (ascent, line_height) = rows
        .iter()
        .flat_map(|r| r.left_runs.iter().chain(r.right_runs.iter()))
        .next()
        .or(leader_run.as_ref())
        .map(|r| (r.ascent as f64, r.line_height as f64))
        .unwrap_or((0.0, 0.0));

    // Rotation bracket: only when both w and h are present (safe pivot center),
    // mirroring the main text path so rotated TOC pages behave consistently.
    let box_h_opt: Option<f64> = resolve_geometry_px(text.h.as_ref(), resolved);
    let rot = rotation_degrees(text.rotate.as_ref());
    let text_rot = rot
        .zip(Some(box_w))
        .zip(box_h_opt)
        .map(|((a, bw), bh)| (a, text_x + bw / 2.0, text_y + bh / 2.0));
    if let Some((angle, cx, cy)) = text_rot {
        commands.push(SceneCommand::PushTransform {
            angle_deg: angle,
            cx,
            cy,
        });
    }

    let gap_pad = leader_advance * TAB_LEADER_GAP_FACTOR;
    let box_right = text_x + box_w;

    for (i, row) in rows.iter().enumerate() {
        let baseline_y = text_y + ascent + (i as f64) * line_height;

        // LEFT segment at the box left edge.
        emit_tab_leader_runs(
            &row.left_runs,
            text_x,
            baseline_y,
            color,
            glyph_stroke,
            commands,
        );

        if !row.has_tab {
            // No tab → left-aligned row, no right segment, no leader.
            continue;
        }

        // RIGHT segment right-aligned: its right edge = box right edge.
        let right_x = box_right - row.right_advance;
        emit_tab_leader_runs(
            &row.right_runs,
            right_x,
            baseline_y,
            color,
            glyph_stroke,
            commands,
        );

        // Fill the gap between LEFT and RIGHT with leader glyphs. The usable gap
        // leaves a small breathing pad on each side.
        let leader_start = text_x + row.left_advance + gap_pad;
        let leader_end = right_x - gap_pad;
        let gap = leader_end - leader_start;

        if leader_advance <= 0.0 || gap <= 0.0 {
            // Row too long (or no leader glyph): emit no leaders. Surface a
            // non-fatal warning so the author knows this row overflowed.
            diagnostics.push(Diagnostic::warning(
                "text.overflow",
                format!(
                    "text '{}': tab-leader row {} is too long to fit a leader \
                     in its box; no leader emitted",
                    text.id,
                    i + 1
                ),
                text.source_span,
                Some(text.id.clone()),
            ));
            continue;
        }

        let count = (gap / leader_advance).floor() as usize;
        if let Some(run) = leader_run.as_ref() {
            let mut x = leader_start;
            for _ in 0..count {
                commands.push(SceneCommand::DrawGlyphRun {
                    x,
                    y: baseline_y,
                    font_id: run.font_id.clone(),
                    font_size: run.font_size,
                    color,
                    stroke_color: glyph_stroke.0,
                    stroke_width: glyph_stroke.1,
                    link: None,
                    selectable: true,
                    glyphs: run_to_scene_glyphs(run),
                });
                x += leader_advance;
            }
        }
    }

    if text_rot.is_some() {
        commands.push(SceneCommand::PopTransform);
    }

    rows.len() as f64 * line_height
}
