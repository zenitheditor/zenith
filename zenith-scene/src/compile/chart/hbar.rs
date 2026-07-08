//! Horizontal bar chart emission for `kind="bar" orientation="horizontal"`.
//!
//! `hbar_rects` is a pure geometry function (no engine, no I/O) that computes
//! pixel rectangles for grouped or stacked horizontal bars — bars grow RIGHT
//! from a left value-axis baseline. `emit_hbar` resolves series colors and
//! pushes `FillRect`, `StrokeLine`, and `DrawGlyphRun` commands.
//!
//! The outer `compile_chart` in `entry.rs` handles title and legend for all
//! chart kinds; `emit_hbar` only draws the plot content (axes + bars + labels).

use zenith_core::{ChartNode, Diagnostic, FontStyle};
use zenith_layout::{ShapeRequest, TextDirection, TextLayoutEngine};

use crate::ir::{Color, Paint, SceneCommand};

use super::super::NodeCtx;
use super::super::paint::resolve_property_color;
use super::super::text::run_to_scene_glyphs;
use super::axis::{AxisColors, format_tick_label};
use super::bar::{BarMode, ON_FILL_LABEL_COLOR, VALUE_LABEL_COLOR, ValueLabelMode, stacked_max};
use super::frame::PlotArea;
use super::palette::series_color;
use super::scale::{LinearScale, data_range, nice_ticks};

// ── Layout constants ───────────────────────────────────────────────────────────

/// Fraction of a category band that is padding (split equally top and bottom).
const CAT_PAD_FRAC: f64 = 0.20;

/// Gap between adjacent sub-rows within a grouped band, as a fraction of
/// `sub_h` (applied once between each pair).
const BAR_GAP_FRAC: f64 = 0.15;

/// Minimum width (px) for a stacked segment to receive a center value label.
const STACKED_LABEL_MIN_W: f64 = 14.0;

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
/// pixel coordinates (data_min → left, data_max → right). `baseline_px` is
/// `x_scale.map(0.0).round()` — the x pixel for the zero line.
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

// ── HBarCtx ───────────────────────────────────────────────────────────────────

/// Per-chart context shared across the value-label emitter — bundles fields
/// that would otherwise push the argument count above the project limit.
#[derive(Clone, Copy)]
struct HBarCtx<'a> {
    plot: &'a PlotArea,
    families: &'a [String],
    chart_id: &'a str,
    placement: ValueLabelMode,
    /// Resolved per-series label color override; `None` → use placement default.
    explicit: Option<Color>,
}

// ── emit_hbar ─────────────────────────────────────────────────────────────────

/// Emit a horizontal bar chart into `bbox`.
///
/// Computes its own plot rect and X value scale; does NOT reuse the vertical
/// `y_scale` / `y_ticks` computed by the outer `compile_chart`.
///
/// Z-order: gridlines + X tick labels → bars → value labels → category labels
/// → axis lines.
pub(in crate::compile) fn emit_hbar(
    chart: &ChartNode,
    bbox: (f64, f64, f64, f64),
    colors: AxisColors,
    cx: NodeCtx,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (bx, by, bw, bh) = bbox;
    let has_title = chart.title.is_some();
    let has_caption = chart.caption.is_some();

    // ── Measure category labels to size the left margin ──────────────────────
    let n_categories = chart
        .series
        .iter()
        .map(|s| s.values.len())
        .max()
        .unwrap_or(0);

    if n_categories == 0 {
        return;
    }

    let cat_families = [String::from("Noto Sans")];
    let mut max_cat_advance = 0.0_f64;

    for c in 0..n_categories {
        let label: String = chart
            .categories
            .get(c)
            .cloned()
            .unwrap_or_else(|| (c + 1).to_string());

        if label.is_empty() {
            continue;
        }

        let req = ShapeRequest {
            text: &label,
            families: &cat_families,
            weight: 400,
            style: FontStyle::Normal,
            font_size: 9.0,
            direction: TextDirection::Ltr,
            features: &[],
            letter_spacing_px: 0.0,
        };

        if let Ok(result) = cx.engine.shape_with_fallback(&req, cx.fonts) {
            let advance: f64 = result.runs.iter().map(|r| r.advance_width as f64).sum();
            if advance > max_cat_advance {
                max_cat_advance = advance;
            }
        }
    }

    // Left margin: measured category label advance + 14 px gap, min 40 px.
    let left_margin = (max_cat_advance + 14.0).max(40.0);

    // Top/bottom/right margins.
    let top = if has_title { 24.0 } else { 10.0 };
    let bottom = 28.0 + if has_caption { 18.0 } else { 0.0 };
    let right = 18.0; // breathing room for value labels at bar ends

    let plot = PlotArea {
        x: bx + left_margin,
        y: by + top,
        w: (bw - left_margin - right).max(0.0),
        h: (bh - top - bottom).max(0.0),
    };

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

    // Non-inverted X scale: data_min → pixel left, data_max → pixel right.
    let x_scale = LinearScale {
        data_min: data_lo,
        data_max: data_hi,
        pixel_min: plot.x,
        pixel_max: plot.x + plot.w,
    };

    let x_ticks = nice_ticks(&x_scale, 5);

    // ── Gridlines + X tick labels (value axis along bottom) ───────────────────
    let tick_families = [String::from("Noto Sans")];

    for tick in &x_ticks {
        let eps = 0.5;
        if tick.pixel < plot.x - eps || tick.pixel > plot.x + plot.w + eps {
            continue;
        }

        // Vertical gridline spanning the plot height.
        let tick_px = tick.pixel.round();
        commands.push(SceneCommand::StrokeLine {
            x1: tick_px,
            y1: plot.y,
            x2: tick_px,
            y2: plot.y + plot.h,
            color: colors.grid,
            stroke_width: 1.0,
            stroke_dash: None,
            stroke_gap: None,
            stroke_linecap: None,
        });

        // Numeric tick label centered horizontally at tick.pixel, below the plot.
        let label = format_tick_label(tick.value);
        let req = ShapeRequest {
            text: &label,
            families: &tick_families,
            weight: 400,
            style: FontStyle::Normal,
            font_size: 9.0,
            direction: TextDirection::Ltr,
            features: &[],
            letter_spacing_px: 0.0,
        };

        match cx.engine.shape_with_fallback(&req, cx.fonts) {
            Err(e) => {
                diagnostics.push(Diagnostic::advisory(
                    "scene.text_unshaped",
                    format!(
                        "chart '{}' hbar X tick label '{}' could not be shaped: {}",
                        chart.id, label, e.message
                    ),
                    None,
                    Some(chart.id.clone()),
                ));
            }
            Ok(result) => {
                let total_advance: f64 = result.runs.iter().map(|r| r.advance_width as f64).sum();
                // Baseline: 14 px below the plot bottom (ascent already baked into the constant).
                let baseline_y = plot.y + plot.h + 14.0;
                let mut label_x = tick.pixel - total_advance / 2.0;

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
                        source_node_id: None,
                        glyphs,
                    });
                    label_x += advance;
                }
            }
        }
    }

    // ── Bars ──────────────────────────────────────────────────────────────────
    let series_values: Vec<&[f64]> = chart.series.iter().map(|s| s.values.as_slice()).collect();
    let rects = hbar_rects(&plot, &x_scale, &series_values, mode);

    if rects.is_empty() {
        // No data — still emit axis frame.
        emit_hbar_axis_lines(&plot, colors.axis, commands);
        return;
    }

    let label_mode = ValueLabelMode::resolve(chart.value_labels.as_deref(), is_stacked);
    let explicit_label_color = chart
        .value_color
        .as_ref()
        .and_then(|p| resolve_property_color(p, cx.resolved, diagnostics, &chart.id));

    let value_label_families = [String::from("Noto Sans")];

    for s in 0..chart.series.len() {
        let color = match chart.series.get(s) {
            Some(series) => series_color(series, s, cx.resolved, diagnostics, &chart.id),
            None => continue,
        };

        let paint = Paint::solid(color);

        // Per-series label color: series.label_color → chart.value_color → default.
        let label_explicit = chart
            .series
            .get(s)
            .and_then(|sr| sr.label_color.as_ref())
            .and_then(|p| resolve_property_color(p, cx.resolved, diagnostics, &chart.id))
            .or(explicit_label_color);

        if let Some(series_rects) = rects.get(s) {
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

                let value = match chart.series.get(s).and_then(|sr| sr.values.get(c)) {
                    Some(v) => *v,
                    None => continue,
                };

                emit_hbar_value_label(
                    value,
                    *rect,
                    HBarCtx {
                        plot: &plot,
                        families: &value_label_families,
                        chart_id: &chart.id,
                        placement: label_mode,
                        explicit: label_explicit,
                    },
                    cx,
                    commands,
                    diagnostics,
                );
            }
        }
    }

    // ── Category labels (Y axis, right-aligned, centered in band) ─────────────
    let band_h = plot.h / n_categories as f64;

    for c in 0..n_categories {
        let label: String = chart
            .categories
            .get(c)
            .cloned()
            .unwrap_or_else(|| (c + 1).to_string());

        if label.is_empty() {
            continue;
        }

        let req = ShapeRequest {
            text: &label,
            families: &cat_families,
            weight: 400,
            style: FontStyle::Normal,
            font_size: 9.0,
            direction: TextDirection::Ltr,
            features: &[],
            letter_spacing_px: 0.0,
        };

        match cx.engine.shape_with_fallback(&req, cx.fonts) {
            Err(e) => {
                diagnostics.push(Diagnostic::advisory(
                    "scene.text_unshaped",
                    format!(
                        "chart '{}' hbar category label '{}' could not be shaped: {}",
                        chart.id, label, e.message
                    ),
                    None,
                    Some(chart.id.clone()),
                ));
            }
            Ok(result) => {
                let total_advance: f64 = result.runs.iter().map(|r| r.advance_width as f64).sum();
                let ascent: f64 = result.runs.first().map(|r| r.ascent as f64).unwrap_or(7.0);

                let band_top = plot.y + c as f64 * band_h;
                // Right-align: end 6 px left of the plot left edge.
                let mut label_x = plot.x - 6.0 - total_advance;
                // Vertically center within the band (cap-height trick).
                let baseline_y = band_top + band_h / 2.0 + ascent * 0.35;

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
                        source_node_id: None,
                        glyphs,
                    });
                    label_x += advance;
                }
            }
        }
    }

    // ── Axis lines (drawn last, on top of bars) ────────────────────────────────
    emit_hbar_axis_lines(&plot, colors.axis, commands);
}

// ── emit_hbar_axis_lines ──────────────────────────────────────────────────────

/// Emit the Y (left) and X (bottom) axis lines for a horizontal bar chart.
///
/// The Y axis is vertical at `plot.x`; the X axis is horizontal at
/// `plot.y + plot.h`. Drawn last so they paint over bar edges.
fn emit_hbar_axis_lines(plot: &PlotArea, axis_color: Color, commands: &mut Vec<SceneCommand>) {
    if plot.w <= 0.0 || plot.h <= 0.0 {
        return;
    }

    // Y (category) axis: left edge, top-to-bottom.
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

    // X (value) axis: bottom edge, left-to-right.
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

// ── emit_hbar_value_label ─────────────────────────────────────────────────────

/// Shape and emit a numeric value label for one horizontal bar.
///
/// Placement follows `hc.placement`:
/// - `Top` (used for grouped): 3 px right of the bar end.
/// - `Center`: horizontally centered inside the segment; skips segments
///   narrower than [`STACKED_LABEL_MIN_W`] px.
/// - `Off` is unreachable here (the caller skips labels when mode is Off)
///   but is listed to avoid a wildcard over a Zenith enum.
///
/// Color: `hc.explicit` wins if set; otherwise `ON_FILL_LABEL_COLOR` (white)
/// for a centered (on-fill) label, `VALUE_LABEL_COLOR` (dark) for a label
/// to the right of the bar.
fn emit_hbar_value_label(
    value: f64,
    rect: HBarRect,
    hc: HBarCtx,
    cx: NodeCtx,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Skip tiny stacked segments that can't hold a label.
    if hc.placement == ValueLabelMode::Center && rect.w < STACKED_LABEL_MIN_W {
        return;
    }

    let label = format_tick_label(value);
    let req = ShapeRequest {
        text: &label,
        families: hc.families,
        weight: 400,
        style: FontStyle::Normal,
        font_size: 9.0,
        direction: TextDirection::Ltr,
        features: &[],
        letter_spacing_px: 0.0,
    };

    match cx.engine.shape_with_fallback(&req, cx.fonts) {
        Err(e) => {
            diagnostics.push(Diagnostic::advisory(
                "scene.text_unshaped",
                format!(
                    "chart '{}' hbar value label '{}' could not be shaped: {}",
                    hc.chart_id, label, e.message
                ),
                None,
                Some(hc.chart_id.to_owned()),
            ));
        }
        Ok(result) => {
            let total_advance: f64 = result.runs.iter().map(|r| r.advance_width as f64).sum();
            let ascent: f64 = result.runs.first().map(|r| r.ascent as f64).unwrap_or(7.0);

            // Vertical center within the bar row (same for both placements).
            let baseline_y = rect.y + rect.h / 2.0 + ascent * 0.35;

            let (label_x_start, on_fill) = match hc.placement {
                // Center: placed inside the segment, centered horizontally.
                ValueLabelMode::Center => {
                    let x = rect.x + rect.w / 2.0 - total_advance / 2.0;
                    (x, true)
                }
                // Top (grouped) / Off (unreachable): 3 px right of the bar end.
                ValueLabelMode::Top | ValueLabelMode::Off => {
                    let bar_right = rect.x + rect.w;
                    // If the label would exceed the plot right edge, tuck it inside.
                    let x = if bar_right + 3.0 + total_advance <= hc.plot.x + hc.plot.w {
                        bar_right + 3.0
                    } else {
                        bar_right - total_advance - 3.0
                    };
                    (x, false)
                }
            };

            let color = hc.explicit.unwrap_or(if on_fill {
                ON_FILL_LABEL_COLOR
            } else {
                VALUE_LABEL_COLOR
            });

            let mut label_x = label_x_start;

            for run in result.runs {
                let advance = run.advance_width as f64;
                let glyphs = run_to_scene_glyphs(&run);
                commands.push(SceneCommand::DrawGlyphRun {
                    x: label_x,
                    y: baseline_y,
                    font_id: run.font_id.clone(),
                    font_size: run.font_size,
                    color,
                    stroke_color: None,
                    stroke_width: None,
                    link: None,
                    selectable: true,
                    source_node_id: None,
                    glyphs,
                });
                label_x += advance;
            }
        }
    }
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
