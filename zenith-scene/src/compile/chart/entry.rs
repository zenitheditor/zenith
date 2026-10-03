//! `compile_chart` entry point.
//!
//! Resolves geometry and the chart look, cuts the title and caption bands
//! from the chart box, and hands the rest to the kind emitter: `cartesian`
//! for `bar`, `line`, and `area`, `pie` for `pie` and `donut`. Sparklines
//! render directly into their bbox with a small inset, no axes, and no text.
//!
//! Returns `0.0`: charts are absolute-positioned and do not participate in
//! flow layout (same contract as `compile_pattern`).

use std::collections::BTreeMap;

use zenith_core::{ChartNode, Diagnostic, ResolvedToken};

use crate::ir::{Color, SceneCommand};

use super::super::NodeCtx;
use super::super::RenderCtx;
use super::super::paint::{NodeEffect, emit_node_with_effects, resolve_property_shadow};
use super::super::style_prop;
use super::super::util::{
    AxisTarget, missing_geometry_diag, resolve_anchored_axis, resolve_geometry_px,
    unsupported_unit_diag,
};
use super::cartesian::emit_cartesian;
use super::frame::{Bands, BoxPx, PlotArea};
use super::legend::{LegendConfig, emit_legend, legend_split};
use super::line::{emit_line_series, line_points};
use super::look::chart_look;
use super::palette::series_color;
use super::pie::{emit_pie, resolve_slice_color};
use super::role::ChartTextRole;
use super::scale::{LinearScale, data_range};
use super::text::ChartText;

// ── compile_chart ─────────────────────────────────────────────────────────────

/// Compile a `chart` node.
///
/// Axis-bearing kinds (`bar`, `line`, `area`) emit the axis frame, value
/// gridlines and tick labels, series geometry, and category labels. Pie and
/// donut charts tessellate wedge polygons with percentage labels. Every kind
/// but `sparkline` draws the title above and the caption below. Sparklines
/// render directly into their bbox (no axes, no text). Any unknown kind string
/// emits nothing.
///
/// A `shadow` (node attribute, else the style's) brackets the whole chart
/// ink as one unit, like a group. Without a shadow the ink lands verbatim.
///
/// Returns `0.0`: charts are absolute-positioned and do not participate in
/// flow layout.
pub(in crate::compile) fn compile_chart(
    chart: &ChartNode,
    cx: NodeCtx,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) -> f64 {
    let effect = chart
        .shadow
        .as_ref()
        .or_else(|| style_prop(&chart.style, cx.style_map, "shadow"))
        .and_then(|p| resolve_property_shadow(p, cx.resolved, &chart.id))
        .map(NodeEffect::Shadow);
    let Some(effect) = effect else {
        return compile_chart_ink(chart, cx, commands, diagnostics, ctx);
    };
    let mut ink: Vec<SceneCommand> = Vec::new();
    let height = compile_chart_ink(chart, cx, &mut ink, diagnostics, ctx);
    if !ink.is_empty() {
        emit_node_with_effects(commands, ink, Some(effect), None);
    }
    height
}

/// Emit the chart ink (see [`compile_chart`]).
fn compile_chart_ink(
    chart: &ChartNode,
    cx: NodeCtx,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: RenderCtx,
) -> f64 {
    // Exclude invisible charts.
    if chart.visible == Some(false) {
        return 0.0;
    }

    // Known kinds proceed past this gate; unknown kind strings emit nothing
    // (validation reports them).
    match chart.kind.as_str() {
        "bar" | "line" | "area" | "sparkline" | "pie" | "donut" => {}
        _ => return 0.0,
    }

    // ── Geometry resolution ──────────────────────────────────────────────────
    // Mirrors compile_shape: require w+h, resolve x/y with anchor fallback,
    // apply ctx.dx/ctx.dy.
    let (Some(w_dim), Some(h_dim)) = (&chart.w, &chart.h) else {
        diagnostics.push(missing_geometry_diag("chart", &chart.id, chart.source_span));
        return 0.0;
    };
    let Some(w) = resolve_geometry_px(Some(w_dim), cx.resolved) else {
        diagnostics.push(unsupported_unit_diag(
            "chart",
            &chart.id,
            "w",
            chart.source_span,
        ));
        return 0.0;
    };
    let Some(h) = resolve_geometry_px(Some(h_dim), cx.resolved) else {
        diagnostics.push(unsupported_unit_diag(
            "chart",
            &chart.id,
            "h",
            chart.source_span,
        ));
        return 0.0;
    };

    let anchor_xy = cx.anchors.get(&chart.id).copied();

    let Some(x_raw) = resolve_anchored_axis(
        AxisTarget {
            kind: "chart",
            node_id: &chart.id,
            axis: "x",
        },
        chart.x.as_ref(),
        cx.resolved,
        anchor_xy.map(|(ax, _)| ax),
        chart.source_span,
        diagnostics,
    ) else {
        return 0.0;
    };
    let Some(y_raw) = resolve_anchored_axis(
        AxisTarget {
            kind: "chart",
            node_id: &chart.id,
            axis: "y",
        },
        chart.y.as_ref(),
        cx.resolved,
        anchor_xy.map(|(_, ay)| ay),
        chart.source_span,
        diagnostics,
    ) else {
        return 0.0;
    };

    let chart_box = (x_raw + ctx.dx, y_raw + ctx.dy, w, h);

    if chart.kind.as_str() == "sparkline" {
        emit_sparkline(chart, chart_box, cx, commands, diagnostics);
        return 0.0;
    }

    let look = chart_look(chart, (w, h), cx, diagnostics);
    let text = ChartText {
        cx,
        look: &look,
        chart_id: &chart.id,
    };
    let bands = Bands { base: look.base };
    let content = bands.content(chart_box, chart.title.is_some(), chart.caption.is_some());

    if matches!(chart.kind.as_str(), "pie" | "donut") {
        emit_round(chart, content, text, commands, diagnostics);
    } else {
        emit_cartesian(chart, content, text, commands, diagnostics);
    }
    emit_title_and_caption(chart, chart_box, bands, text, commands, diagnostics);

    0.0
}

// ── Sparkline emitter ─────────────────────────────────────────────────────────

/// Emit a sparkline into `[x, y, w, h]` with a small inset on all four sides.
///
/// Sparklines are compact, axis-free series previews — no gridlines, no tick
/// labels, no title. One 1.5 px stroke per series, colored by the shared
/// series-color resolver.
fn emit_sparkline(
    chart: &ChartNode,
    bbox: BoxPx,
    cx: NodeCtx,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    const INSET: f64 = 4.0;
    let (x, y, w, h) = bbox;

    let spark_plot = PlotArea {
        x: x + INSET,
        y: y + INSET,
        w: (w - 2.0 * INSET).max(0.0),
        h: (h - 2.0 * INSET).max(0.0),
    };

    // Auto-fit data range (no zero-fold, no stacked expansion).
    let (data_lo, data_hi) =
        data_range(&chart.series, chart.axis_min, chart.axis_max).unwrap_or((0.0, 1.0));

    let y_scale = LinearScale {
        data_min: data_lo,
        data_max: data_hi,
        pixel_min: spark_plot.y + spark_plot.h,
        pixel_max: spark_plot.y,
    };

    for (idx, series) in chart.series.iter().enumerate() {
        let color = series_color(series, idx, cx.resolved, diagnostics, &chart.id);
        let pts = line_points(&series.values, &spark_plot, &y_scale, false);
        emit_line_series(&pts, color, 1.5, commands);
    }
}

// ── Pie / donut ───────────────────────────────────────────────────────────────

/// Emit a pie or donut chart and its legend into `content`.
fn emit_round(
    chart: &ChartNode,
    content: BoxPx,
    text: ChartText<'_>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let config = LegendConfig::of(chart);
    let entries = match config {
        Some(_) => {
            let n = chart.series.first().map_or(0, |s| s.values.len());
            pie_legend_entries(chart, n, text.cx.resolved, diagnostics)
        }
        None => Vec::new(),
    };
    let (bbox, legend) = legend_split(&entries, config, content, text);
    let is_donut = chart.kind.as_str() == "donut";
    emit_pie(chart, bbox, is_donut, text, commands, diagnostics);
    if let Some((area, config)) = legend {
        emit_legend(&entries, area, config, text, commands, diagnostics);
    }
}

// ── Title and caption ─────────────────────────────────────────────────────────

/// Emit the title at the top-left of the chart box and the caption at its
/// bottom-left, each inset by the edge padding.
fn emit_title_and_caption(
    chart: &ChartNode,
    chart_box: BoxPx,
    bands: Bands,
    text: ChartText<'_>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (x, y, _, h) = chart_box;
    let left = x + bands.edge_pad();
    if let Some(title) = &chart.title
        && let Some(shaped) = text.shape(title, ChartTextRole::Title, diagnostics)
    {
        let baseline = y + bands.title_inset() + shaped.ascent;
        text.emit(shaped, (left, baseline), text.look.ink, 0, commands);
    }
    if let Some(caption) = &chart.caption
        && let Some(shaped) = text.shape(caption, ChartTextRole::Caption, diagnostics)
    {
        let baseline = y + h - bands.caption_inset();
        text.emit(shaped, (left, baseline), text.look.ink, 0, commands);
    }
}

// ── Legend entry builders ─────────────────────────────────────────────────────

/// Build legend entries for a pie or donut chart.
///
/// One entry per category (indexed into `series[0].values`): the label comes
/// from `chart.categories[i]` when available, falling back to the 1-based
/// ordinal string `"1"`, `"2"`, … Slice colors are resolved via the same
/// `resolve_slice_color` path used by `emit_pie`, so legend swatches always
/// agree with the slice fills.
fn pie_legend_entries(
    chart: &ChartNode,
    n: usize,
    resolved: &BTreeMap<String, ResolvedToken>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<(String, Color)> {
    (0..n)
        .map(|i| {
            let label = chart
                .categories
                .get(i)
                .cloned()
                .unwrap_or_else(|| (i + 1).to_string());
            let color = resolve_slice_color(chart, i, resolved, diagnostics);
            (label, color)
        })
        .collect()
}
