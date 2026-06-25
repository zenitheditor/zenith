//! Axis frame emission: Y axis line, X axis line, Y gridlines, and Y tick
//! labels for axis-bearing chart kinds (bar, line).
//!
//! `emit_axes_frame` pushes `SceneCommand`s for the axis box and tick labels.
//! It is pure (no side effects beyond pushing commands/diagnostics) and
//! deterministic.

use zenith_core::{Diagnostic, FontStyle};
use zenith_layout::{ShapeRequest, TextDirection, TextLayoutEngine};

use crate::ir::{Color, SceneCommand};

use super::super::NodeCtx;
use super::super::text::run_to_scene_glyphs;
use super::frame::PlotArea;
use super::scale::Tick;

// ── Color bundle ──────────────────────────────────────────────────────────────

/// Color bundle for the axis frame — avoids triggering the `too_many_arguments`
/// lint on `emit_axes_frame` without a suppression attribute.
#[derive(Clone, Copy)]
pub(super) struct AxisColors {
    /// Color for the Y and X axis lines.
    pub(super) axis: Color,
    /// Color for the horizontal Y-gridlines.
    pub(super) grid: Color,
    /// Color for Y tick-label text.
    pub(super) label: Color,
}

// ── Numeric label formatter ────────────────────────────────────────────────────

/// Format a tick value as a compact string.
///
/// - Integers are printed without a decimal point: `42`.
/// - Non-integers are printed with minimal trailing-zero trimming: `42.5`.
/// - No locale, no thousands separators.
fn format_tick_label(value: f64) -> String {
    // Round to 10 decimal places to suppress floating-point noise.
    let rounded = (value * 1e10).round() / 1e10;
    if rounded.fract() == 0.0 && rounded.abs() < 1e15 {
        // Safe to cast to i64 for integer formatting.
        format!("{}", rounded as i64)
    } else {
        // Trim trailing zeros from the decimal representation.
        let s = format!("{:.10}", rounded);
        let s = s.trim_end_matches('0');
        let s = s.trim_end_matches('.');
        s.to_owned()
    }
}

// ── emit_axes_frame ───────────────────────────────────────────────────────────

/// Emit the axis frame for a bar or line chart.
///
/// Pushes the following `SceneCommand`s (all reads are deterministic):
/// 1. Y axis line (left edge of the plot area, top-to-bottom).
/// 2. X axis line (bottom edge of the plot area, left-to-right).
/// 3. For each Y tick: a horizontal gridline in `colors.grid` and a
///    right-aligned numeric label in `colors.label` positioned just left of
///    the Y axis.
///
/// Shaping errors are collected as advisory diagnostics; the frame is still
/// emitted (with the label omitted for the failing tick).
pub(super) fn emit_axes_frame(
    plot: &PlotArea,
    y_ticks: &[Tick],
    colors: AxisColors,
    chart_id: &str,
    cx: NodeCtx,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Skip emission entirely for a zero-size plot area.
    if plot.w <= 0.0 || plot.h <= 0.0 {
        return;
    }

    // ── Y axis line ──────────────────────────────────────────────────────────
    commands.push(SceneCommand::StrokeLine {
        x1: plot.x,
        y1: plot.y,
        x2: plot.x,
        y2: plot.y + plot.h,
        color: colors.axis,
        stroke_width: 1.0,
        stroke_dash: None,
        stroke_gap: None,
        stroke_linecap: None,
    });

    // ── X axis line ──────────────────────────────────────────────────────────
    commands.push(SceneCommand::StrokeLine {
        x1: plot.x,
        y1: plot.y + plot.h,
        x2: plot.x + plot.w,
        y2: plot.y + plot.h,
        color: colors.axis,
        stroke_width: 1.0,
        stroke_dash: None,
        stroke_gap: None,
        stroke_linecap: None,
    });

    // ── Y gridlines and tick labels ──────────────────────────────────────────
    // Hoisted outside the tick loop: avoids one heap allocation per tick.
    let label_families = [String::from("Noto Sans")];

    for tick in y_ticks {
        // Skip ticks that land outside the plot area (with a small epsilon).
        let eps = 0.5;
        if tick.pixel < plot.y - eps || tick.pixel > plot.y + plot.h + eps {
            continue;
        }

        // Gridline: full width of plot area, in grid color.
        commands.push(SceneCommand::StrokeLine {
            x1: plot.x,
            y1: tick.pixel,
            x2: plot.x + plot.w,
            y2: tick.pixel,
            color: colors.grid,
            stroke_width: 1.0,
            stroke_dash: None,
            stroke_gap: None,
            stroke_linecap: None,
        });

        // Tick label: right-aligned, just left of the Y axis.
        let label = format_tick_label(tick.value);
        let req = ShapeRequest {
            text: &label,
            families: &label_families,
            weight: 400,
            style: FontStyle::Normal,
            font_size: 9.0,
            direction: TextDirection::Ltr,
        };

        match cx.engine.shape_with_fallback(&req, cx.fonts) {
            Err(e) => {
                diagnostics.push(Diagnostic::advisory(
                    "scene.text_unshaped",
                    format!(
                        "chart '{}' axis tick label '{}' could not be shaped: {}",
                        chart_id, label, e.message
                    ),
                    None,
                    Some(chart_id.to_owned()),
                ));
                // Skip the label; frame lines still emitted.
            }
            Ok(result) => {
                // Compute total advance width for right-alignment.
                let total_advance: f64 = result.runs.iter().map(|r| r.advance_width as f64).sum();

                // Ascent from the first run for vertical centering on the tick line.
                let ascent: f64 = result.runs.first().map(|r| r.ascent as f64).unwrap_or(0.0);

                // Right-aligned: start x is total_advance left of the Y axis.
                let mut label_x = plot.x - 4.0 - total_advance;
                // Baseline: center the cap height on the tick pixel.
                let baseline_y = tick.pixel + ascent * 0.5;

                for run in result.runs {
                    let advance = run.advance_width as f64;
                    let glyphs = run_to_scene_glyphs(&run);
                    commands.push(SceneCommand::DrawGlyphRun {
                        x: label_x,
                        y: baseline_y,
                        font_id: run.font_id.clone(),
                        font_size: run.font_size,
                        color: colors.label,
                        stroke_color: None,
                        stroke_width: None,
                        glyphs,
                    });
                    label_x += advance;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::format_tick_label;

    #[test]
    fn format_integer() {
        assert_eq!(format_tick_label(0.0), "0");
        assert_eq!(format_tick_label(42.0), "42");
        assert_eq!(format_tick_label(-10.0), "-10");
        assert_eq!(format_tick_label(100.0), "100");
    }

    #[test]
    fn format_non_integer() {
        assert_eq!(format_tick_label(42.5), "42.5");
        assert_eq!(format_tick_label(-0.25), "-0.25");
        assert_eq!(format_tick_label(1.1), "1.1");
    }

    #[test]
    fn format_trailing_zero_trimmed() {
        // 1.0 is an integer, so no decimal point.
        assert_eq!(format_tick_label(1.0), "1");
        // 1.50 should trim to "1.5".
        assert_eq!(format_tick_label(1.5), "1.5");
    }
}
