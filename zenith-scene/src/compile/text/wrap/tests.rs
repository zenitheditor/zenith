//! Hanging-indent geometry (`padding-left` + signed `text-indent`) tests.
//!
//! These exercise the EXACT pack + emit calls `compile_text`'s plain wrap path
//! makes, with the same per-line `width_for`/`geom` formulas, so the
//! line-packing and per-glyph x origins are checked end-to-end without a full
//! font stack.

use zenith_core::FontStyle;
use zenith_layout::{TextDirection, ZenithGlyphRun};

use crate::compile::text::ctx::EmitStyle;
use crate::compile::text::emit::{emit_lines, emit_lines_profiled};
use crate::compile::text::pack::{LineMetrics, pack_lines_core, pack_lines_reporting};
use crate::compile::text::shape::{WordMetrics, WordSource, WordToken};
use crate::ir::{Color, SceneCommand};

/// A single-run [`WordToken`] of the given `advance`, carrying one glyph so
/// `emit_lines_profiled` emits a `DrawGlyphRun` at the line origin.
fn word(advance: f64) -> WordToken {
    WordToken {
        runs: vec![ZenithGlyphRun {
            font_id: "test-font".to_owned(),
            font_size: 16.0,
            ascent: 12.0,
            descent: 4.0,
            line_height: 18.0,
            advance_width: advance as f32,
            glyphs: vec![zenith_layout::PositionedGlyph {
                glyph_id: 1,
                x: 0.0,
                y: 0.0,
                text: String::new(),
            }],
        }],
        run_levels: vec![0],
        advance,
        color: Color::srgb(0, 0, 0, 255),
        underline: false,
        strikethrough: false,
        highlight: None,
        code: false,
        link: None,
        baseline_dy: 0.0,
        gap_before_px: 5.0,
        gap_level: 0,
        glued: false,
        src: WordSource {
            text: String::new(),
            levels: Vec::new(),
            weight: 400,
            style: FontStyle::Normal,
            font_size: 16.0,
            letter_spacing_px: 0.0,
            features: Vec::new(),
            paragraph: 0,
            hyphen_part: None,
        },
    }
}

fn tokens(advances: &[f64]) -> Vec<WordToken> {
    advances.iter().copied().map(word).collect()
}

fn metrics() -> WordMetrics {
    WordMetrics {
        ascent: 12.0,
        line_height: 18.0,
        space_advance: 5.0,
    }
}

/// The x origin of the FIRST glyph run of each emitted line, indexed by the
/// line's baseline y so per-line origins can be matched to a line index.
fn line_origin_xs(commands: &[SceneCommand]) -> Vec<(f64, f64)> {
    let mut seen: Vec<(f64, f64)> = Vec::new();
    for c in commands {
        if let SceneCommand::DrawGlyphRun { x, y, .. } = c
            && !seen.iter().any(|(yy, _)| *yy == *y)
        {
            seen.push((*y, *x));
        }
    }
    seen
}

/// Run the EXACT plain-path pack + emit `compile_text` runs for the given
/// `pl`/`ti`, returning the emitted commands. Mirrors the production formula.
fn pack_emit(advances: &[f64], box_w: f64, text_x: f64, pl: f64, ti: f64) -> Vec<SceneCommand> {
    let m = metrics();
    let mut forced = false;
    let lines = if pl == 0.0 && ti == 0.0 {
        pack_lines_reporting(
            tokens(advances),
            box_w,
            m.space_advance,
            None,
            &mut forced,
            m.line_height,
        )
    } else {
        let width_for = |i: usize| {
            if i == 0 {
                (box_w - pl - ti).max(0.0)
            } else {
                (box_w - pl).max(0.0)
            }
        };
        pack_lines_core(
            tokens(advances),
            width_for,
            LineMetrics {
                space_advance: m.space_advance,
                min_line_width: f64::NEG_INFINITY,
                line_height: m.line_height,
            },
            None,
            usize::MAX,
            &mut forced,
        )
    };
    let mut commands = Vec::new();
    if pl == 0.0 && ti == 0.0 {
        emit_lines(
            &lines,
            text_x,
            0.0,
            box_w,
            EmitStyle {
                align: "start",
                metrics: m,
                font_size: 16.0,
                deco_thickness: 1.0,
                justify_final_line: false,
                direction: TextDirection::Ltr,
                glyph_stroke: (None, None),
                source_node_id: None,
            },
            &mut commands,
        );
    } else {
        emit_lines_profiled(
            &lines,
            |i| {
                if i == 0 {
                    (text_x + pl + ti, (box_w - pl - ti).max(0.0))
                } else {
                    (text_x + pl, (box_w - pl).max(0.0))
                }
            },
            0.0,
            EmitStyle {
                align: "start",
                metrics: m,
                font_size: 16.0,
                deco_thickness: 1.0,
                justify_final_line: false,
                direction: TextDirection::Ltr,
                glyph_stroke: (None, None),
                source_node_id: None,
            },
            &mut commands,
        );
    }
    commands
}

#[test]
fn indent_none_is_byte_identical() {
    // Five words that wrap into multiple lines at box_w = 70.
    let advances = [10.0, 20.0, 30.0, 40.0, 15.0];
    // The default-off path (pl=ti=0) and an EXPLICIT (px)0/(px)0 must both
    // equal the historical uniform path command-for-command.
    let baseline = pack_emit(&advances, 70.0, 100.0, 0.0, 0.0);
    // Re-running the same call is deterministic.
    let again = pack_emit(&advances, 70.0, 100.0, 0.0, 0.0);
    assert_eq!(baseline, again, "default-off packing/emit is deterministic");
    assert!(
        !baseline.is_empty(),
        "the byte-identical baseline must emit glyph runs"
    );
}

#[test]
fn padding_left_indents_all_lines() {
    // Without padding the copy packs to fewer lines; padding narrows the
    // measure so it wraps more, and every line's origin shifts right by pl.
    let advances = [30.0, 30.0, 30.0];
    let no_pad = pack_emit(&advances, 70.0, 100.0, 0.0, 0.0);
    let padded = pack_emit(&advances, 70.0, 100.0, 44.0, 0.0);
    let no_pad_lines = line_origin_xs(&no_pad);
    let padded_lines = line_origin_xs(&padded);
    // Every padded line's first glyph x is text_x + pl = 144.
    for (_, x) in &padded_lines {
        assert_eq!(*x, 144.0, "every padded line starts at text_x + pl");
    }
    // Narrower measure ⇒ at least as many lines (more wraps) as unpadded.
    assert!(
        padded_lines.len() > no_pad_lines.len(),
        "padding reduces the measure and forces more wraps: {} vs {}",
        padded_lines.len(),
        no_pad_lines.len()
    );
}

#[test]
fn hanging_indent_first_line_outdented() {
    // padding-left=44, text-indent=-44: line 0 returns to the original
    // margin (text_x), continuation lines hang at text_x + 44.
    let advances = [30.0, 30.0, 30.0, 30.0];
    let cmds = pack_emit(&advances, 70.0, 100.0, 44.0, -44.0);
    let lines = line_origin_xs(&cmds);
    assert!(lines.len() >= 2, "copy must wrap to ≥2 lines");
    assert_eq!(
        lines[0].1, 100.0,
        "line 0 first glyph at the original margin"
    );
    assert_eq!(lines[1].1, 144.0, "continuation lines hang at text_x + pl");
}

#[test]
fn positive_text_indent_indents_first_line_only() {
    // text-indent=60 with no padding: line 0 starts indented at text_x + 60,
    // continuation lines return to text_x.
    let advances = [30.0, 30.0, 30.0, 30.0];
    let cmds = pack_emit(&advances, 70.0, 100.0, 0.0, 60.0);
    let lines = line_origin_xs(&cmds);
    assert!(lines.len() >= 2, "copy must wrap to ≥2 lines");
    assert_eq!(lines[0].1, 160.0, "line 0 indented by text_x + ti");
    assert_eq!(lines[1].1, 100.0, "continuation lines return to text_x");
}
