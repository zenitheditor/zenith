//! Axis-bearing chart kinds: `bar` (vertical and horizontal), `line`, and
//! `area`. Lays out legend and plot inside the content box (the chart box
//! minus the title and caption bands), then draws gridlines, marks, labels,
//! and axis lines.

use zenith_core::{ChartNode, Diagnostic};

use crate::ir::{Color, SceneCommand};

use super::axis::{emit_axis_lines, emit_gridlines_and_labels, max_tick_advance};
use super::bar::{BarMode, stacked_max};
use super::bar_emit::{CatLabels, emit_bars, emit_category_labels};
use super::frame::{Bands, BoxPx};
use super::hbar::emit_hbar;
use super::legend::{LegendConfig, emit_legend, legend_split};
use super::line::{emit_area_fill, emit_line_series, line_points};
use super::palette::series_color;
use super::scale::{LinearScale, data_range, nice_ticks};
use super::text::ChartText;

/// Target tick count on the value axis.
const TICKS: u32 = 5;

/// Emit a `bar`, `line`, or `area` chart into `content`.
///
/// Z-order: bar: gridlines → bars → category labels → axis lines.
/// Line/area: gridlines → area fills → line strokes → axis lines → category
/// labels. The legend draws last.
pub(super) fn emit_cartesian(
    chart: &ChartNode,
    content: BoxPx,
    text: ChartText<'_>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let config = LegendConfig::of(chart);
    let entries: Vec<(String, Color)> = match config {
        Some(_) => chart
            .series
            .iter()
            .enumerate()
            .map(|(s, sr)| {
                let label = sr
                    .label
                    .clone()
                    .unwrap_or_else(|| format!("Series {}", s + 1));
                let color = series_color(sr, s, text.cx.resolved, diagnostics, &chart.id);
                (label, color)
            })
            .collect(),
        None => Vec::new(),
    };
    let (bbox, legend) = legend_split(&entries, config, content, text);

    if chart.kind.as_str() == "bar" && chart.orientation.as_deref() == Some("horizontal") {
        emit_hbar(chart, bbox, text, commands, diagnostics);
    } else {
        emit_vertical(chart, bbox, text, commands, diagnostics);
    }

    if let Some((area, config)) = legend {
        emit_legend(&entries, area, config, text, commands, diagnostics);
    }
}

/// The value domain of a vertical chart: the data range (or `(0, 1)` with no
/// data), widened to include 0 for bars and to the stacked total for
/// stacked bars, unless the author pinned `axis-min` / `axis-max`.
fn value_domain(chart: &ChartNode) -> (f64, f64) {
    let (mut lo, mut hi) =
        data_range(&chart.series, chart.axis_min, chart.axis_max).unwrap_or((0.0, 1.0));
    let is_bar = chart.kind.as_str() == "bar";
    // Bars grow from a zero baseline: a bar drawn from a non-zero floor
    // misrepresents magnitude.
    if is_bar && chart.axis_min.is_none() {
        lo = lo.min(0.0);
    }
    // Stacked bars reach the per-category sum, not the largest single value.
    if is_bar
        && BarMode::from_opt(chart.bar_mode.as_deref()) == BarMode::Stacked
        && chart.axis_max.is_none()
    {
        hi = hi.max(stacked_max(chart));
    }
    (lo, hi)
}

/// Emit a vertical `bar`, `line`, or `area` chart into `bbox`.
fn emit_vertical(
    chart: &ChartNode,
    bbox: BoxPx,
    text: ChartText<'_>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let bands = Bands { base: text.base() };
    let (data_lo, data_hi) = value_domain(chart);

    // Tick values depend on the domain only, so a unit pixel span measures
    // the label margin before the plot exists.
    let probe = LinearScale {
        data_min: data_lo,
        data_max: data_hi,
        pixel_min: 1.0,
        pixel_max: 0.0,
    };
    let label_w = max_tick_advance(&nice_ticks(&probe, TICKS), text);
    let plot = bands.plot(bbox, label_w);

    // Inverted Y: data_min → pixel bottom, data_max → pixel top.
    let y_scale = LinearScale {
        data_min: data_lo,
        data_max: data_hi,
        pixel_min: plot.y + plot.h,
        pixel_max: plot.y,
    };
    let y_ticks = nice_ticks(&y_scale, TICKS);
    let n_categories = chart
        .series
        .iter()
        .map(|s| s.values.len())
        .max()
        .unwrap_or(0);
    let gap = bands.label_gap();

    emit_gridlines_and_labels(&plot, &y_ticks, gap, text, commands, diagnostics);

    if chart.kind.as_str() == "bar" {
        emit_bars(chart, &plot, &y_scale, text, commands, diagnostics);
        emit_category_labels(
            &chart.categories,
            n_categories,
            CatLabels {
                plot: &plot,
                slot_center: true,
                gap,
            },
            text,
            commands,
            diagnostics,
        );
        emit_axis_lines(&plot, text.look, commands);
        return;
    }

    // Line / area. Default edge-to-edge (first point on the value axis, last
    // at the right edge); point-placement="center" insets onto category bands.
    let slot_center = chart.point_placement.as_deref() == Some("center");
    let series_geom: Vec<(Vec<(f64, f64)>, Color)> = chart
        .series
        .iter()
        .enumerate()
        .map(|(idx, series)| {
            let c = series_color(series, idx, text.cx.resolved, diagnostics, &chart.id);
            (line_points(&series.values, &plot, &y_scale, slot_center), c)
        })
        .collect();

    // Area fills first (drawn below the line strokes), at ~25% alpha.
    if chart.kind.as_str() == "area" {
        for (pts, c) in &series_geom {
            emit_area_fill(pts, &plot, Color::srgb(c.r, c.g, c.b, 64), commands);
        }
    }
    for (pts, c) in &series_geom {
        emit_line_series(pts, *c, 2.0, commands);
    }
    emit_axis_lines(&plot, text.look, commands);
    if !chart.categories.is_empty() {
        emit_category_labels(
            &chart.categories,
            n_categories,
            CatLabels {
                plot: &plot,
                slot_center,
                gap,
            },
            text,
            commands,
            diagnostics,
        );
    }
}
