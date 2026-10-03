//! Horizontal bar chart emission for `kind="bar" orientation="horizontal"`.
//!
//! `hbar_rects` is a pure geometry function (no engine, no I/O) that computes
//! pixel rectangles for grouped or stacked horizontal bars — bars grow RIGHT
//! from a left value-axis baseline. `emit_hbar` resolves series colors and
//! pushes `FillRect`, `StrokeLine`, and `DrawGlyphRun` commands.
//!
//! The outer `compile_chart` handles title, caption, and legend for all chart
//! kinds; `emit_hbar` only draws the plot content (axes + bars + labels).

use zenith_core::{ChartNode, Diagnostic};

use crate::ir::{Color, Paint, SceneCommand};

use super::axis::{emit_axis_lines, format_tick_label, line};
use super::bar::{BarMode, ValueLabelMode, stacked_max};
use super::bar_emit::{explicit_value_color, value_ink, value_role};
use super::frame::{Bands, BoxPx, PlotArea, inset};
use super::palette::series_color;
use super::role::ChartTextRole;
use super::scale::{LinearScale, data_range, nice_ticks};
use super::text::ChartText;

// ── Layout constants ───────────────────────────────────────────────────────────

/// Fraction of a category band that is padding (split equally top and bottom).
const CAT_PAD_FRAC: f64 = 0.20;

/// Gap between adjacent sub-rows within a grouped band, as a fraction of
/// `sub_h` (applied once between each pair).
const BAR_GAP_FRAC: f64 = 0.15;

/// Gap between a bar end and an outside value label: `0.3 b`.
const VALUE_GAP: f64 = 0.3;
/// Extra room a value label needs inside its segment: `0.4 b`.
const INSIDE_ROOM: f64 = 0.4;

// ── HBarRect ──────────────────────────────────────────────────────────────────

/// Pixel rectangle for a single horizontal bar.
///
/// A `w == 0.0` or `h < 0.5` sentinel means "nothing to draw here".
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct HBarRect {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) w: f64,
    pub(super) h: f64,
}

// ── hbar_rects ────────────────────────────────────────────────────────────────

/// Compute pixel rectangles for every horizontal bar.
///
/// Returns `rects[series_idx][category_idx]`. The outer `Vec` has one entry
/// per series; the inner `Vec` has one entry per category.
///
/// `plot` is the drawable data region. `x_scale` maps data values to horizontal
/// pixel coordinates (data_min → left, data_max → right).
///
/// Returns an empty `Vec` when `n_categories == 0` or `plot.h <= 0`.
///
/// PURE: no engine, no I/O, no side effects.
pub(super) fn hbar_rects(
    plot: &PlotArea,
    x_scale: &LinearScale,
    series_values: &[&[f64]],
    mode: BarMode,
) -> Vec<Vec<HBarRect>> {
    let n_categories = series_values.iter().map(|s| s.len()).max().unwrap_or(0);
    if n_categories == 0 || plot.h <= 0.0 {
        return Vec::new();
    }

    let n_series = series_values.len();
    if n_series == 0 {
        return Vec::new();
    }

    // Snap the value baseline to a whole device pixel. Bar segment edges are
    // rounded to integers so abutting stacked segments share an exact pixel
    // boundary (no 1-px anti-aliased seam between fills).
    let baseline_px = x_scale.map(0.0).round();

    let band_h = plot.h / n_categories as f64;
    let usable_h = band_h * (1.0 - CAT_PAD_FRAC);
    let top_pad = (band_h - usable_h) / 2.0;

    match mode {
        BarMode::Grouped => {
            // sub_h * (n_series + (n_series-1)*BAR_GAP_FRAC) = usable_h
            let sub_h = usable_h / (n_series as f64 * (1.0 + BAR_GAP_FRAC) - BAR_GAP_FRAC).max(1.0);

            if sub_h <= 0.0 {
                return Vec::new();
            }

            let step = sub_h * (1.0 + BAR_GAP_FRAC);

            series_values
                .iter()
                .enumerate()
                .map(|(s, sv)| {
                    (0..n_categories)
                        .map(|c| match sv.get(c) {
                            None => HBarRect {
                                x: 0.0,
                                y: 0.0,
                                w: 0.0,
                                h: 0.0,
                            },
                            Some(&value) => {
                                let band_top = plot.y + c as f64 * band_h;
                                let bar_y = band_top + top_pad + s as f64 * step;
                                let bar_h = sub_h * (1.0 - BAR_GAP_FRAC);
                                let x_end = x_scale.map(value).round();
                                let x = baseline_px.min(x_end);
                                let w = (x_end - baseline_px).abs();
                                HBarRect {
                                    x,
                                    y: bar_y,
                                    w,
                                    h: bar_h,
                                }
                            }
                        })
                        .collect()
                })
                .collect()
        }

        BarMode::Stacked => {
            // One cumulative accumulator per category.
            let mut cumulative = vec![0.0f64; n_categories];

            series_values
                .iter()
                .map(|sv| {
                    (0..n_categories)
                        .map(|c| match sv.get(c) {
                            None => HBarRect {
                                x: 0.0,
                                y: 0.0,
                                w: 0.0,
                                h: 0.0,
                            },
                            Some(&value) => {
                                let band_top = plot.y + c as f64 * band_h;
                                let bar_y = band_top + top_pad;
                                let bar_h = usable_h;
                                let lower = cumulative.get(c).copied().unwrap_or(0.0);
                                let upper = lower + value;
                                if let Some(slot) = cumulative.get_mut(c) {
                                    *slot = upper;
                                }
                                // Round both edges so abutting segments share exact boundaries.
                                let x0 = x_scale.map(lower).round();
                                let x1 = x_scale.map(upper).round();
                                let x = x0.min(x1);
                                let w = (x1 - x0).abs();
                                HBarRect {
                                    x,
                                    y: bar_y,
                                    w,
                                    h: bar_h,
                                }
                            }
                        })
                        .collect()
                })
                .collect()
        }
    }
}

// ── emit_hbar ─────────────────────────────────────────────────────────────────

/// The category label of slot `c`: the declared label, else its 1-based index.
fn category_label(chart: &ChartNode, c: usize) -> String {
    chart
        .categories
        .get(c)
        .cloned()
        .unwrap_or_else(|| (c + 1).to_string())
}

/// Emit a horizontal bar chart into `bbox`.
///
/// Computes its own plot rect and X value scale. The left margin fits the
/// widest category label; the bottom band holds the value tick labels.
///
/// Z-order: gridlines + X tick labels → bars → value labels → category labels
/// → axis lines.
pub(in crate::compile) fn emit_hbar(
    chart: &ChartNode,
    bbox: BoxPx,
    text: ChartText<'_>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let n_categories = chart
        .series
        .iter()
        .map(|s| s.values.len())
        .max()
        .unwrap_or(0);
    if n_categories == 0 {
        return;
    }
    let look = text.look;
    let bands = Bands { base: text.base() };

    let max_cat_advance = (0..n_categories)
        .map(|c| text.advance(&category_label(chart, c), ChartTextRole::Category))
        .fold(0.0_f64, f64::max);
    let left = max_cat_advance + bands.label_gap() + bands.edge_pad();
    let plot = inset(bbox, left, bands.top(), bands.right(), bands.label_band());
    if plot.w <= 0.0 || plot.h <= 0.0 {
        return;
    }

    // ── X value scale (horizontal; data_min → left, data_max → right) ────────
    let (mut data_lo, mut data_hi) =
        data_range(&chart.series, chart.axis_min, chart.axis_max).unwrap_or((0.0, 1.0));
    // Horizontal bars also grow from a zero baseline.
    if chart.axis_min.is_none() {
        data_lo = data_lo.min(0.0);
    }
    let mode = BarMode::from_opt(chart.bar_mode.as_deref());
    let is_stacked = mode == BarMode::Stacked;
    if is_stacked && chart.axis_max.is_none() {
        data_hi = data_hi.max(stacked_max(chart));
    }
    let x_scale = LinearScale {
        data_min: data_lo,
        data_max: data_hi,
        pixel_min: plot.x,
        pixel_max: plot.x + plot.w,
    };

    // ── Gridlines + X tick labels (value axis along bottom) ───────────────────
    let mut tick_index = 0;
    for tick in &nice_ticks(&x_scale, 5) {
        let eps = 0.5;
        if tick.pixel < plot.x - eps || tick.pixel > plot.x + plot.w + eps {
            continue;
        }
        let tick_px = tick.pixel.round();
        commands.push(line(
            (tick_px, plot.y),
            (tick_px, plot.y + plot.h),
            look.grid,
            look.line_w,
        ));
        let label = format_tick_label(tick.value);
        if let Some(shaped) = text.shape(&label, ChartTextRole::Axis, diagnostics) {
            let baseline = plot.y + plot.h + bands.label_gap() + shaped.ascent;
            let x = tick.pixel - shaped.advance / 2.0;
            text.emit(shaped, (x, baseline), look.ink, tick_index, commands);
            tick_index += 1;
        }
    }

    // ── Bars ──────────────────────────────────────────────────────────────────
    let series_values: Vec<&[f64]> = chart.series.iter().map(|s| s.values.as_slice()).collect();
    let rects = hbar_rects(&plot, &x_scale, &series_values, mode);
    let label_mode = ValueLabelMode::resolve(chart.value_labels.as_deref(), is_stacked);

    for (s, series) in chart.series.iter().enumerate() {
        let color = series_color(series, s, text.cx.resolved, diagnostics, &chart.id);
        let paint = Paint::solid(color);
        let explicit = explicit_value_color(chart, s, text, diagnostics);
        let Some(series_rects) = rects.get(s) else {
            continue;
        };
        for (c, rect) in series_rects.iter().enumerate() {
            if rect.w < 0.5 || rect.h < 0.5 {
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
            emit_hbar_value_label(
                value,
                *rect,
                HBarCtx {
                    plot: &plot,
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

    // ── Category labels (Y axis, right-aligned, centered in band) ─────────────
    let band_h = plot.h / n_categories as f64;
    for c in 0..n_categories {
        let label = category_label(chart, c);
        if label.is_empty() {
            continue;
        }
        let Some(shaped) = text.shape(&label, ChartTextRole::Category, diagnostics) else {
            continue;
        };
        let band_top = plot.y + c as f64 * band_h;
        let x = plot.x - bands.label_gap() - shaped.advance;
        let baseline = band_top + band_h / 2.0 + shaped.ascent * 0.35;
        text.emit(shaped, (x, baseline), look.ink, c, commands);
    }

    // ── Axis lines (drawn last, on top of bars) ────────────────────────────────
    emit_axis_lines(&plot, look, commands);
}

// ── emit_hbar_value_label ─────────────────────────────────────────────────────

/// Per-bar inputs of a value label, bundled to keep the argument list short.
#[derive(Clone, Copy)]
struct HBarCtx<'a> {
    plot: &'a PlotArea,
    placement: ValueLabelMode,
    /// Resolved label-color override.
    explicit: Option<Color>,
    /// The bar fill.
    fill: Color,
    /// Source-id index: `series × categories + category`.
    index: usize,
}

/// Shape and emit a numeric value label for one horizontal bar.
///
/// Placement follows `hc.placement`:
/// - `Top`: `0.3 b` right of the bar end; tucked inside the bar end when it
///   passes the plot's right edge.
/// - `Center`: centered inside the segment; a segment too narrow for the
///   label gets none.
fn emit_hbar_value_label(
    value: f64,
    rect: HBarRect,
    hc: HBarCtx<'_>,
    text: ChartText<'_>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let label = format_tick_label(value);
    let Some(shaped) = text.shape(&label, ChartTextRole::Value, diagnostics) else {
        return;
    };
    let b = text.base();
    let advance = shaped.advance;
    let baseline = rect.y + rect.h / 2.0 + shaped.ascent * 0.35;
    let (x, inside) = match hc.placement {
        ValueLabelMode::Center => {
            if rect.w < advance + INSIDE_ROOM * b {
                return;
            }
            (rect.x + rect.w / 2.0 - advance / 2.0, true)
        }
        // Off never reaches here (the caller skips it); it shares the Top arm
        // so the match stays exhaustive.
        ValueLabelMode::Top | ValueLabelMode::Off => {
            let bar_right = rect.x + rect.w;
            if bar_right + VALUE_GAP * b + advance <= hc.plot.x + hc.plot.w {
                (bar_right + VALUE_GAP * b, false)
            } else {
                (bar_right - advance - VALUE_GAP * b, true)
            }
        }
    };
    let color = value_ink(hc.explicit, inside.then_some(hc.fill), text.look);
    text.emit(
        shaped.with_role(value_role(inside)),
        (x, baseline),
        color,
        hc.index,
        commands,
    );
}

// ── Unit tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_plot() -> PlotArea {
        PlotArea {
            x: 80.0,
            y: 10.0,
            w: 300.0,
            h: 200.0,
        }
    }

    /// X scale: data [0, 100] → pixels [80, 380] (left-to-right, non-inverted).
    fn test_x_scale() -> LinearScale {
        LinearScale {
            data_min: 0.0,
            data_max: 100.0,
            pixel_min: 80.0,  // plot.x — left edge (data_min)
            pixel_max: 380.0, // plot.x + plot.w — right edge (data_max)
        }
    }

    #[test]
    fn hbar_rects_empty_series_returns_empty() {
        let plot = test_plot();
        let scale = test_x_scale();
        assert!(hbar_rects(&plot, &scale, &[], BarMode::Grouped).is_empty());
    }

    #[test]
    fn hbar_rects_zero_categories_returns_empty() {
        let plot = test_plot();
        let scale = test_x_scale();
        let empty: &[f64] = &[];
        assert!(hbar_rects(&plot, &scale, &[empty], BarMode::Grouped).is_empty());
    }

    #[test]
    fn hbar_rects_single_series_grouped_geometry() {
        let plot = test_plot();
        let scale = test_x_scale();
        let values: &[f64] = &[25.0, 50.0, 75.0];
        let rects = hbar_rects(&plot, &scale, &[values], BarMode::Grouped);

        assert_eq!(rects.len(), 1, "one series");
        assert_eq!(rects[0].len(), 3, "three categories");

        let baseline = scale.map(0.0);
        let eps = 0.5;

        for r in &rects[0] {
            // All bars start at or after the baseline (non-negative values).
            assert!((r.x - baseline).abs() < eps, "bar should start at baseline");
            // Bar right edge within plot.
            assert!(r.x + r.w <= plot.x + plot.w + eps, "bar right exceeds plot");
        }

        // Larger value → wider bar.
        let r0 = rects[0][0]; // 25
        let r1 = rects[0][1]; // 50
        let r2 = rects[0][2]; // 75
        assert!(r0.w < r1.w, "25 bar narrower than 50 bar");
        assert!(r1.w < r2.w, "50 bar narrower than 75 bar");
    }

    #[test]
    fn hbar_rects_grouped_two_series_no_vertical_overlap() {
        let plot = test_plot();
        let scale = test_x_scale();
        let s0: &[f64] = &[30.0, 60.0];
        let s1: &[f64] = &[10.0, 20.0];
        let rects = hbar_rects(&plot, &scale, &[s0, s1], BarMode::Grouped);

        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].len(), 2);
        assert_eq!(rects[1].len(), 2);

        for (c, (r0, r1)) in rects[0].iter().zip(rects[1].iter()).enumerate() {
            // Series 0 is above series 1 (lower y) within each band.
            assert!(r0.y < r1.y, "series 0 not above series 1 at cat {}", c);
            // No vertical overlap: r0 bottom edge <= r1 top edge.
            assert!(
                r0.y + r0.h <= r1.y + 0.5,
                "bars overlap vertically at cat {}: r0 bottom={} r1 top={}",
                c,
                r0.y + r0.h,
                r1.y
            );
        }
    }

    #[test]
    fn hbar_rects_stacked_same_y_abutting_widths() {
        let plot = test_plot();
        let scale = test_x_scale();
        let s0: &[f64] = &[20.0, 40.0];
        let s1: &[f64] = &[30.0, 10.0];
        let rects = hbar_rects(&plot, &scale, &[s0, s1], BarMode::Stacked);

        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0].len(), 2);
        assert_eq!(rects[1].len(), 2);

        let eps = 0.5;
        for c in 0..2 {
            let r0 = rects[0][c];
            let r1 = rects[1][c];

            // Same y and h (stacked, same band).
            assert!(
                (r0.y - r1.y).abs() < eps,
                "stacked bars differ in y at cat {}",
                c
            );
            assert!(
                (r0.h - r1.h).abs() < eps,
                "stacked bars differ in h at cat {}",
                c
            );

            // Series 1 is to the right of series 0 (x0 < x1).
            assert!(r0.x <= r1.x, "series 1 not right of series 0 at cat {}", c);

            // Combined widths equal the width for the summed value.
            let combined_value = s0[c] + s1[c];
            let expected_w = (scale.map(combined_value) - scale.map(0.0)).abs();
            let actual_w = r0.w + r1.w;
            assert!(
                (actual_w - expected_w).abs() < eps,
                "stacked widths don't sum at cat {}: got {} expected {}",
                c,
                actual_w,
                expected_w
            );
        }
    }

    #[test]
    fn hbar_rects_baseline_at_zero_pixel() {
        // For non-negative values, bar starts at scale.map(0.0) (baseline).
        let plot = test_plot();
        let scale = test_x_scale();
        let values: &[f64] = &[50.0];
        let rects = hbar_rects(&plot, &scale, &[values], BarMode::Grouped);

        let baseline = scale.map(0.0).round();
        let r = rects[0][0];
        let eps = 0.5;
        assert!(
            (r.x - baseline).abs() < eps,
            "bar x ({}) should be at baseline ({})",
            r.x,
            baseline
        );
        // Bar right edge should be at scale.map(50.0).
        let expected_right = scale.map(50.0).round();
        assert!(
            (r.x + r.w - expected_right).abs() < eps,
            "bar right edge ({}) should be at scale.map(50) ({})",
            r.x + r.w,
            expected_right
        );
    }
}
