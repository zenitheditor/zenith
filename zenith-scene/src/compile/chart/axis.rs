//! Axis frame emission: Y axis line, X axis line, Y gridlines, and Y tick
//! labels for axis-bearing chart kinds (bar, line, area).
//!
//! All emitters are pure (no side effects beyond pushing commands/diagnostics)
//! and deterministic.

use zenith_core::{Diagnostic, FontStyle};
use zenith_layout::{ShapeRequest, TextDirection, TextLayoutEngine};

use crate::ir::{Color, SceneCommand};

use super::super::NodeCtx;
use super::super::text::run_to_scene_glyphs;
use super::frame::PlotArea;
use super::scale::Tick;

// ── Color bundle ──────────────────────────────────────────────────────────────

/// Color bundle for the axis frame — avoids triggering the `too_many_arguments`
/// lint on the axis emitters without a suppression attribute.
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
pub(super) fn format_tick_label(value: f64) -> String {
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

// ── emit_gridlines_and_labels ─────────────────────────────────────────────────

/// Emit Y gridlines and Y tick labels for a chart plot area.
///
/// Pushes for each Y tick: a horizontal gridline in `colors.grid` and a
/// right-aligned numeric label in `colors.label` positioned just left of the
/// Y axis.
///
/// Separated from axis-line emission so that bars can be drawn OVER gridlines
/// but UNDER the axis lines (z-order: gridlines → bars → axis lines).
///
/// Shaping errors are collected as advisory diagnostics; the gridlines are
/// still emitted when a label fails to shape.
pub(super) fn emit_gridlines_and_labels(
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
            features: &[],
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
                // Skip the label; gridline still emitted.
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
                        link: None,
                        selectable: true,
                        glyphs,
                    });
                    label_x += advance;
                }
            }
        }
    }
}

// ── emit_axis_lines ───────────────────────────────────────────────────────────

/// Emit the Y axis line and X axis line for a chart plot area.
///
/// These are drawn LAST (after bars or series) so they paint over any bar that
/// touches the axis edge.
pub(super) fn emit_axis_lines(
    plot: &PlotArea,
    axis_color: Color,
    commands: &mut Vec<SceneCommand>,
) {
    if plot.w <= 0.0 || plot.h <= 0.0 {
        return;
    }

    // Y axis line: left edge, top-to-bottom.
    commands.push(SceneCommand::StrokeLine {
        x1: plot.x,
        y1: plot.y,
        x2: plot.x,
        y2: plot.y + plot.h,
        color: axis_color,
        stroke_width: 1.0,
        stroke_dash: None,
        stroke_gap: None,
        stroke_linecap: None,
    });

    // X axis line: bottom edge, left-to-right.
    commands.push(SceneCommand::StrokeLine {
        x1: plot.x,
        y1: plot.y + plot.h,
        x2: plot.x + plot.w,
        y2: plot.y + plot.h,
        color: axis_color,
        stroke_width: 1.0,
        stroke_dash: None,
        stroke_gap: None,
        stroke_linecap: None,
    });
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
