//! Axis frame emission: Y axis line, X axis line, value gridlines, and value
//! tick labels for axis-bearing chart kinds (bar, line, area).
//!
//! All emitters are pure (no side effects beyond pushing commands/diagnostics)
//! and deterministic. Lines take the chart look's axis and grid paint; tick
//! labels draw at the `axis` role size in the chart ink.

use zenith_core::Diagnostic;

use crate::ir::{Color, SceneCommand};

use super::frame::PlotArea;
use super::look::ChartLook;
use super::role::ChartTextRole;
use super::scale::Tick;
use super::text::ChartText;

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

/// The widest tick label advance, for sizing the label margin.
pub(super) fn max_tick_advance(ticks: &[Tick], text: ChartText<'_>) -> f64 {
    ticks
        .iter()
        .map(|t| text.advance(&format_tick_label(t.value), ChartTextRole::Axis))
        .fold(0.0_f64, f64::max)
}

/// One straight line in `color` and `width`.
pub(super) fn line(
    (x1, y1): (f64, f64),
    (x2, y2): (f64, f64),
    color: Color,
    width: f64,
) -> SceneCommand {
    SceneCommand::StrokeLine {
        x1,
        y1,
        x2,
        y2,
        color,
        stroke_width: width,
        stroke_dash: None,
        stroke_gap: None,
        stroke_linecap: None,
    }
}

// ── emit_gridlines_and_labels ─────────────────────────────────────────────────

/// Emit Y gridlines and Y tick labels for a chart plot area.
///
/// Pushes for each Y tick: a horizontal gridline in the grid colour and a
/// right-aligned numeric label positioned one label gap left of the Y axis.
///
/// Separated from axis-line emission so that bars can be drawn OVER gridlines
/// but UNDER the axis lines (z-order: gridlines → bars → axis lines).
///
/// Shaping errors are collected as advisory diagnostics; the gridlines are
/// still emitted when a label fails to shape.
pub(super) fn emit_gridlines_and_labels(
    plot: &PlotArea,
    y_ticks: &[Tick],
    gap: f64,
    text: ChartText<'_>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Skip emission entirely for a zero-size plot area.
    if plot.w <= 0.0 || plot.h <= 0.0 {
        return;
    }
    let look = text.look;
    let mut index = 0;
    for tick in y_ticks {
        // Skip ticks that land outside the plot area (with a small epsilon).
        let eps = 0.5;
        if tick.pixel < plot.y - eps || tick.pixel > plot.y + plot.h + eps {
            continue;
        }

        commands.push(line(
            (plot.x, tick.pixel),
            (plot.x + plot.w, tick.pixel),
            look.grid,
            look.line_w,
        ));

        let label = format_tick_label(tick.value);
        if let Some(shaped) = text.shape(&label, ChartTextRole::Axis, diagnostics) {
            // Right-aligned; the cap height centres on the tick line.
            let x = plot.x - gap - shaped.advance;
            let baseline = tick.pixel + shaped.ascent * 0.35;
            text.emit(shaped, (x, baseline), look.ink, index, commands);
            index += 1;
        }
    }
}

// ── emit_axis_lines ───────────────────────────────────────────────────────────

/// Emit the Y axis line (left edge) and X axis line (bottom edge) for a chart
/// plot area.
///
/// These are drawn LAST (after bars or series) so they paint over any bar that
/// touches the axis edge.
pub(super) fn emit_axis_lines(plot: &PlotArea, look: &ChartLook, commands: &mut Vec<SceneCommand>) {
    if plot.w <= 0.0 || plot.h <= 0.0 {
        return;
    }
    let bottom = plot.y + plot.h;
    commands.push(line(
        (plot.x, plot.y),
        (plot.x, bottom),
        look.axis,
        look.line_w,
    ));
    commands.push(line(
        (plot.x, bottom),
        (plot.x + plot.w, bottom),
        look.axis,
        look.line_w,
    ));
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
