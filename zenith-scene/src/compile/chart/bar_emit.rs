//! Vertical bar emission (bars and value labels) and the category labels
//! shared by bar, line, and area charts.

use zenith_core::{ChartNode, Diagnostic};

use crate::ir::{Color, Paint, SceneCommand};

use super::super::paint::resolve_property_color;
use super::axis::format_tick_label;
use super::bar::{BarMode, BarRect, ValueLabelMode, bar_rects};
use super::frame::PlotArea;
use super::look::{ChartLook, black_or_white};
use super::palette::series_color;
use super::role::ChartTextRole;
use super::scale::LinearScale;
use super::text::ChartText;

/// Gap between a bar end and an outside value label: `0.3 b`.
const VALUE_GAP: f64 = 0.3;
/// Extra room a value label needs inside its segment: `0.4 b`.
const INSIDE_ROOM: f64 = 0.4;

/// Colour of a value label: the explicit override (series `label-color`,
/// else chart `value-color`), else black or white by contrast with the mark
/// fill when the label sits on it (`on_fill`), else the chart ink.
pub(super) fn value_ink(
    explicit: Option<Color>,
    on_fill: Option<Color>,
    look: &ChartLook,
) -> Color {
    explicit.unwrap_or_else(|| on_fill.map_or(look.ink, black_or_white))
}

/// The value-label role: inside the mark or on the plot background.
pub(super) fn value_role(inside: bool) -> ChartTextRole {
    if inside {
        ChartTextRole::ValueInside
    } else {
        ChartTextRole::Value
    }
}

/// The explicit value-label colour of series `s`: its `label-color`, else the
/// chart `value-color`.
pub(super) fn explicit_value_color(
    chart: &ChartNode,
    s: usize,
    text: ChartText<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Color> {
    let resolved = text.cx.resolved;
    let mut resolve = |p: &zenith_core::PropertyValue| {
        resolve_property_color(p, resolved, diagnostics, &chart.id)
    };
    chart
        .series
        .get(s)
        .and_then(|sr| sr.label_color.as_ref())
        .and_then(&mut resolve)
        .or_else(|| chart.value_color.as_ref().and_then(&mut resolve))
}

// ── emit_bars ─────────────────────────────────────────────────────────────────

/// Resolve series colors and emit `FillRect` commands for every bar, followed
/// by a value label placed per the chart's `value-labels` mode.
///
/// Bars with `w <= 0` or `h < 0.5` are skipped. An empty series list or an
/// empty series produces no commands.
pub(super) fn emit_bars(
    chart: &ChartNode,
    plot: &PlotArea,
    y_scale: &LinearScale,
    text: ChartText<'_>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let series_values: Vec<&[f64]> = chart.series.iter().map(|s| s.values.as_slice()).collect();
    let mode = BarMode::from_opt(chart.bar_mode.as_deref());
    let rects = bar_rects(plot, y_scale, &series_values, mode);
    if rects.is_empty() {
        return;
    }
    let n_categories = series_values.iter().map(|s| s.len()).max().unwrap_or(0);
    let label_mode =
        ValueLabelMode::resolve(chart.value_labels.as_deref(), mode == BarMode::Stacked);

    for (s, series) in chart.series.iter().enumerate() {
        let color = series_color(series, s, text.cx.resolved, diagnostics, &chart.id);
        let paint = Paint::solid(color);
        let explicit = explicit_value_color(chart, s, text, diagnostics);
        let Some(series_rects) = rects.get(s) else {
            continue;
        };
        for (c, rect) in series_rects.iter().enumerate() {
            if rect.w <= 0.0 || rect.h < 0.5 {
                continue;
            }
            commands.push(SceneCommand::FillRect {
                x: rect.x,
                y: rect.y,
                w: rect.w,
                h: rect.h,
                paint: paint.clone(),
            });
            if label_mode == ValueLabelMode::Off {
                continue;
            }
            let Some(value) = series.values.get(c).copied() else {
                continue;
            };
            emit_value_label(
                value,
                *rect,
                LabelCtx {
                    plot,
                    placement: label_mode,
                    explicit,
                    fill: color,
                    index: s * n_categories + c,
                },
                text,
                commands,
                diagnostics,
            );
        }
    }
}

// ── emit_value_label ──────────────────────────────────────────────────────────

/// Per-bar inputs of a value label, bundled to keep the argument list short.
#[derive(Clone, Copy)]
struct LabelCtx<'a> {
    plot: &'a PlotArea,
    placement: ValueLabelMode,
    /// Resolved label-color override.
    explicit: Option<Color>,
    /// The bar fill.
    fill: Color,
    /// Source-id index: `series × categories + category`.
    index: usize,
}

/// Shape and emit a numeric value label for one bar.
///
/// Placement follows `lc.placement`:
/// - `Top`: `0.3 b` above the bar; if that clips the plot top it falls inside.
/// - `Center`: vertically centered inside the bar/segment. A segment too
///   short for the label gets none.
fn emit_value_label(
    value: f64,
    rect: BarRect,
    lc: LabelCtx<'_>,
    text: ChartText<'_>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let label = format_tick_label(value);
    let Some(shaped) = text.shape(&label, ChartTextRole::Value, diagnostics) else {
        return;
    };
    let b = text.base();
    let ascent = shaped.ascent;
    let (baseline, inside) = match lc.placement {
        ValueLabelMode::Center => {
            if rect.h < ascent + INSIDE_ROOM * b {
                return;
            }
            (rect.y + rect.h / 2.0 + ascent * 0.35, true)
        }
        // Off never reaches here (emit_bars skips it); it shares the Top arm
        // so the match stays exhaustive.
        ValueLabelMode::Top | ValueLabelMode::Off => {
            if rect.y - VALUE_GAP * b - ascent >= lc.plot.y {
                (rect.y - VALUE_GAP * b, false)
            } else {
                (rect.y + ascent + VALUE_GAP * b, true)
            }
        }
    };
    let color = value_ink(lc.explicit, inside.then_some(lc.fill), text.look);
    let x = rect.x + rect.w / 2.0 - shaped.advance / 2.0;
    text.emit(
        shaped.with_role(value_role(inside)),
        (x, baseline),
        color,
        lc.index,
        commands,
    );
}

// ── emit_category_labels ──────────────────────────────────────────────────────

/// Layout inputs for category labels, bundled to keep the emitter's argument
/// list within bounds.
#[derive(Clone, Copy)]
pub(super) struct CatLabels<'a> {
    /// Plot rectangle the labels sit beneath.
    pub(super) plot: &'a PlotArea,
    /// `true` → labels under category-band centers (bars); `false` → under
    /// edge-to-edge vertex positions (line/area).
    pub(super) slot_center: bool,
    /// Gap between the X axis and the label ink top.
    pub(super) gap: f64,
}

/// Emit X-axis category labels under each category.
///
/// When `slot_center` is true, each label is centered under its category band
/// (`plot.x + (c + 0.5) * slot_w`) — the placement bars use. When false, labels
/// sit at the edge-to-edge vertex positions (`plot.x + c * plot.w/(n-1)`, single
/// category centered) so they line up under line/area vertices.
///
/// When `categories` is shorter than `n_categories`, the remaining slots are
/// labelled by 1-based index (`"1"`, `"2"`, …). Empty label strings are
/// skipped.
pub(super) fn emit_category_labels(
    categories: &[String],
    n_categories: usize,
    layout: CatLabels<'_>,
    text: ChartText<'_>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let plot = layout.plot;
    if n_categories == 0 || plot.w <= 0.0 {
        return;
    }
    for c in 0..n_categories {
        let label: String = categories
            .get(c)
            .cloned()
            .unwrap_or_else(|| (c + 1).to_string());
        if label.is_empty() {
            continue;
        }
        let Some(shaped) = text.shape(&label, ChartTextRole::Category, diagnostics) else {
            continue;
        };
        // Match line_points' X placement so labels sit under vertices.
        let center_x = if layout.slot_center {
            plot.x + (c as f64 + 0.5) * (plot.w / n_categories as f64)
        } else if n_categories <= 1 {
            plot.x + plot.w / 2.0
        } else {
            plot.x + c as f64 * (plot.w / (n_categories - 1) as f64)
        };
        let baseline = plot.y + plot.h + layout.gap + shaped.ascent;
        let x = center_x - shaped.advance / 2.0;
        text.emit(shaped, (x, baseline), text.look.ink, c, commands);
    }
}
