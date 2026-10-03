//! Legend rendering for chart nodes.
//!
//! `legend_reserve` measures the pixel space (width or height) needed for a
//! legend strip; `emit_legend` pushes colored swatches and label glyph runs
//! into the command buffer. Both functions are no-ops when no entries are
//! supplied. Placement, layout, and alignment are controlled via `LegendConfig`.
//! Every legend measure is a multiple of the chart base size (see [`Metrics`]),
//! and labels draw at the `legend` role size in the chart ink.

use zenith_core::{ChartNode, Diagnostic};

use crate::ir::{Color, Paint, SceneCommand};

use super::frame::BoxPx;
use super::role::ChartTextRole;
use super::text::ChartText;

// ── Metrics ───────────────────────────────────────────────────────────────────

/// Legend measures in px, each a fixed multiple of the chart base size `b`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Metrics {
    /// Left and right padding inside the strip: `1.0 b`.
    pad: f64,
    /// Swatch square edge: `1.1 b`.
    swatch: f64,
    /// Gap between swatch and label: `0.6 b`.
    gap: f64,
    /// Vertical slot per entry: `1.8 b`.
    line_h: f64,
    /// Horizontal gap between wrapped entries: `1.6 b`.
    entry_gap: f64,
    /// Top and bottom padding inside a band: `0.8 b`.
    pad_v: f64,
}

impl Metrics {
    fn of(base: f64) -> Self {
        Metrics {
            pad: base,
            swatch: base * 1.1,
            gap: base * 0.6,
            line_h: base * 1.8,
            entry_gap: base * 1.6,
            pad_v: base * 0.8,
        }
    }
}

// ── LegendArea ────────────────────────────────────────────────────────────────

/// The rectangular strip reserved for the legend.
#[derive(Clone, Copy)]
pub(super) struct LegendArea {
    /// Left edge of the legend strip in device-space pixels.
    pub(super) x: f64,
    /// Top edge of the legend strip in device-space pixels.
    pub(super) y: f64,
    /// Width of the legend strip in device-space pixels.
    pub(super) w: f64,
    /// Height of the legend strip in device-space pixels.
    pub(super) h: f64,
}

// ── Enums ─────────────────────────────────────────────────────────────────────

/// Which side of the chart the legend is placed on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum LegendPosition {
    Left,
    Right,
    Top,
    Bottom,
}

impl LegendPosition {
    /// Resolve from an `Option<&str>` node field; unknown values fall back to `Right`.
    pub(super) fn from_opt(s: Option<&str>) -> Self {
        match s {
            Some("left") => Self::Left,
            Some("top") => Self::Top,
            Some("bottom") => Self::Bottom,
            _ => Self::Right,
        }
    }

    /// `true` for `Left` and `Right` (vertical strip); `false` for `Top`/`Bottom` (band).
    pub(super) fn is_side(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

/// Entry layout within the legend area.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum LegendLayout {
    /// Single vertical column (or centered horizontal column for top/bottom).
    List,
    /// Entries flow left-to-right, wrapping onto new rows.
    Wrapped,
}

impl LegendLayout {
    /// Resolve from an `Option<&str>` node field; unknown values fall back to `Wrapped`.
    pub(super) fn from_opt(s: Option<&str>) -> Self {
        match s {
            Some("list") => Self::List,
            _ => Self::Wrapped,
        }
    }
}

/// Horizontal alignment of the legend block within its area.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum LegendAlign {
    Start,
    Center,
    End,
}

impl LegendAlign {
    /// Resolve from an `Option<&str>` node field; unknown values fall back to `Center`.
    pub(super) fn from_opt(s: Option<&str>) -> Self {
        match s {
            Some("left") => Self::Start,
            Some("right") => Self::End,
            _ => Self::Center,
        }
    }
}

/// All legend presentation options bundled for forwarding through helpers.
#[derive(Clone, Copy)]
pub(super) struct LegendConfig {
    pub(super) position: LegendPosition,
    pub(super) layout: LegendLayout,
    pub(super) align: LegendAlign,
}

// ── Pure width arithmetic ─────────────────────────────────────────────────────

/// Compute the total legend strip width from the widest label advance:
/// `pad + swatch + gap + max_advance + pad`.
fn legend_width_from_advance(max_advance: f64, m: Metrics) -> f64 {
    m.pad + m.swatch + m.gap + max_advance + m.pad
}

/// Shape each label and return its advance width (px). Shaping errors yield `0.0`.
fn entry_advances(entries: &[(String, Color)], text: ChartText<'_>) -> Vec<f64> {
    entries
        .iter()
        .map(|(label, _)| text.advance(label, ChartTextRole::Legend))
        .collect()
}

/// Pixel width that a single entry occupies (swatch + gap + label advance).
fn entry_content_w(advance: f64, m: Metrics) -> f64 {
    m.swatch + m.gap + advance
}

// ── legend_split ──────────────────────────────────────────────────────────────

/// Split `content` between the plot and the legend. With no legend (or no
/// entries) the plot keeps all of `content`. A side strip is capped at 40% of
/// the width, a band at 50% of the height.
pub(super) fn legend_split(
    entries: &[(String, Color)],
    config: Option<LegendConfig>,
    content: BoxPx,
    text: ChartText<'_>,
) -> (BoxPx, Option<(LegendArea, LegendConfig)>) {
    let Some(config) = config else {
        return (content, None);
    };
    let (x, y, w, h) = content;
    let (wr, hr) = legend_reserve(entries, config, w, text);
    let (w_res, h_res) = if config.position.is_side() {
        (wr.min(w * 0.4), hr)
    } else {
        (wr, hr.min(h * 0.5))
    };
    if w_res <= 0.0 && h_res <= 0.0 {
        return (content, None);
    }
    let (plot, area) = match config.position {
        LegendPosition::Right => ((x, y, w - w_res, h), (x + w - w_res, y, w_res, h)),
        LegendPosition::Left => ((x + w_res, y, w - w_res, h), (x, y, w_res, h)),
        LegendPosition::Top => ((x, y + h_res, w, h - h_res), (x, y, w, h_res)),
        LegendPosition::Bottom => ((x, y, w, h - h_res), (x, y + h - h_res, w, h_res)),
    };
    let (ax, ay, aw, ah) = area;
    let area = LegendArea {
        x: ax,
        y: ay,
        w: aw,
        h: ah,
    };
    (plot, Some((area, config)))
}

impl LegendConfig {
    /// The legend config of `chart`, or `None` when `legend` is not `#true`.
    pub(super) fn of(chart: &ChartNode) -> Option<Self> {
        (chart.legend == Some(true)).then(|| LegendConfig {
            position: LegendPosition::from_opt(chart.legend_position.as_deref()),
            layout: LegendLayout::from_opt(chart.legend_layout.as_deref()),
            align: LegendAlign::from_opt(chart.legend_align.as_deref()),
        })
    }
}

// ── legend_reserve ────────────────────────────────────────────────────────────

/// Compute the `(width_reserve, height_reserve)` the legend needs.
///
/// - Side (`Left`/`Right`): returns `(strip_w, 0.0)`.
/// - Band (`Top`/`Bottom`): returns `(0.0, band_h)`.
///
/// Returns `(0.0, 0.0)` when `entries` is empty.
fn legend_reserve(
    entries: &[(String, Color)],
    config: LegendConfig,
    avail_w: f64,
    text: ChartText<'_>,
) -> (f64, f64) {
    if entries.is_empty() {
        return (0.0, 0.0);
    }
    let m = Metrics::of(text.base());

    if config.position.is_side() {
        let advances = entry_advances(entries, text);
        let max_advance = advances.into_iter().fold(0.0_f64, f64::max);
        return (legend_width_from_advance(max_advance, m), 0.0);
    }

    // Top / Bottom band.
    let height = match config.layout {
        LegendLayout::List => entries.len() as f64 * m.line_h + 2.0 * m.pad_v,
        LegendLayout::Wrapped => {
            let advances = entry_advances(entries, text);
            let rows = wrapped_row_count(&advances, avail_w, m);
            rows as f64 * m.line_h + 2.0 * m.pad_v
        }
    };
    (0.0, height)
}

/// Count the number of rows needed to wrap `advances` into `avail_w`.
fn wrapped_row_count(advances: &[f64], avail_w: f64, m: Metrics) -> usize {
    wrapped_rows(advances, avail_w, m).len().max(1)
}

/// Group entry indices into rows: greedy left-to-right flow.
fn wrapped_rows(advances: &[f64], avail_w: f64, m: Metrics) -> Vec<Vec<usize>> {
    let row_avail = (avail_w - 2.0 * m.pad).max(1.0);
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut cur_row: Vec<usize> = Vec::new();
    let mut cur: f64 = 0.0;

    for (i, &adv) in advances.iter().enumerate() {
        let cw = entry_content_w(adv, m);
        if cur > 0.0 && cur + m.entry_gap + cw > row_avail {
            rows.push(cur_row);
            cur_row = vec![i];
            cur = cw;
        } else if cur > 0.0 {
            cur += m.entry_gap + cw;
            cur_row.push(i);
        } else {
            cur = cw;
            cur_row.push(i);
        }
    }

    if !cur_row.is_empty() {
        rows.push(cur_row);
    }

    if rows.is_empty() {
        rows.push(Vec::new());
    }

    rows
}

// ── draw_entry helper ─────────────────────────────────────────────────────────

/// One legend entry to draw: its index, label, swatch colour, and slot origin.
struct Entry<'e> {
    index: usize,
    label: &'e str,
    color: Color,
    swatch_x: f64,
    line_top: f64,
}

/// Emit one swatch + label glyph run.
fn draw_entry(
    entry: Entry<'_>,
    text: ChartText<'_>,
    m: Metrics,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let swatch_y = entry.line_top + (m.line_h - m.swatch) / 2.0;
    commands.push(SceneCommand::FillRect {
        x: entry.swatch_x,
        y: swatch_y,
        w: m.swatch,
        h: m.swatch,
        paint: Paint::solid(entry.color),
    });

    if let Some(shaped) = text.shape(entry.label, ChartTextRole::Legend, diagnostics) {
        let baseline_y = entry.line_top + m.line_h / 2.0 + shaped.ascent * 0.35;
        let text_x = entry.swatch_x + m.swatch + m.gap;
        text.emit(
            shaped,
            (text_x, baseline_y),
            text.look.ink,
            entry.index,
            commands,
        );
    }
}

// ── align_x helper ────────────────────────────────────────────────────────────

/// Compute the left edge of a block of width `block_w` within `[left_edge .. right_edge]`.
/// The result is clamped to `left_edge` so the block never overflows on the start side.
fn align_x(align: LegendAlign, block_w: f64, left_edge: f64, right_edge: f64) -> f64 {
    let x = match align {
        LegendAlign::Start => left_edge,
        LegendAlign::Center => left_edge + (right_edge - left_edge - block_w) / 2.0,
        LegendAlign::End => right_edge - block_w,
    };
    x.max(left_edge)
}

// ── emit_legend ───────────────────────────────────────────────────────────────

/// Emit swatches and labels for `entries` into `area`.
///
/// Dispatch rules:
/// - **Side** (`Left`/`Right`): vertical list centered in `area.h`; alignment
///   and layout fields are ignored (the strip is always as wide as the longest label).
/// - **Band** (`Top`/`Bottom`):
///   - `List` — single vertical column, aligned by `config.align` within the band.
///   - `Wrapped` — greedy-flow rows, each row aligned by `config.align`.
///
/// No-op when `area.w <= 0.0`, `area.h <= 0.0`, or `entries` is empty.
pub(super) fn emit_legend(
    entries: &[(String, Color)],
    area: LegendArea,
    config: LegendConfig,
    text: ChartText<'_>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if area.w <= 0.0 || area.h <= 0.0 || entries.is_empty() {
        return;
    }
    let m = Metrics::of(text.base());
    let area_bottom = area.y + area.h;
    let left_edge = area.x + m.pad;
    let right_edge = area.x + area.w - m.pad;

    // Rows of entry indices and the left edge of each row's block.
    let advances = entry_advances(entries, text);
    let column = |count: usize| -> Vec<Vec<usize>> { (0..count).map(|i| vec![i]).collect() };
    let rows: Vec<Vec<usize>> = match (config.position.is_side(), config.layout) {
        (true, _) | (false, LegendLayout::List) => column(entries.len()),
        (false, LegendLayout::Wrapped) => wrapped_rows(&advances, area.w, m),
    };
    let total_h = rows.len() as f64 * m.line_h;
    let start_y = (area.y + (area.h - total_h) / 2.0).max(area.y);
    // A list block aligns as one column, as wide as its widest entry.
    let list_w = advances
        .iter()
        .map(|&a| entry_content_w(a, m))
        .fold(0.0_f64, f64::max);

    for (row_idx, row) in rows.iter().enumerate() {
        let line_top = start_y + row_idx as f64 * m.line_h;
        if line_top >= area_bottom {
            break;
        }
        let row_w: f64 = row.iter().enumerate().fold(0.0, |acc, (j, &ei)| {
            let cw = entry_content_w(advances.get(ei).copied().unwrap_or(0.0), m);
            if j == 0 {
                acc + cw
            } else {
                acc + m.entry_gap + cw
            }
        });
        let mut x = match (config.position.is_side(), config.layout) {
            (true, _) => left_edge,
            (false, LegendLayout::List) => align_x(config.align, list_w, left_edge, right_edge),
            (false, LegendLayout::Wrapped) => align_x(config.align, row_w, left_edge, right_edge),
        };
        for (j, &ei) in row.iter().enumerate() {
            if j > 0 {
                x += m.entry_gap;
            }
            let Some((label, color)) = entries.get(ei) else {
                continue;
            };
            draw_entry(
                Entry {
                    index: ei,
                    label,
                    color: *color,
                    swatch_x: x,
                    line_top,
                },
                text,
                m,
                commands,
                diagnostics,
            );
            x += entry_content_w(advances.get(ei).copied().unwrap_or(0.0), m);
        }
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Metrics at base 10: pad 10, swatch 11, gap 6, line 18, entry gap 16.
    fn m10() -> Metrics {
        Metrics::of(10.0)
    }

    // ── Metrics ───────────────────────────────────────────────────────────────

    #[test]
    fn metrics_scale_with_the_base() {
        let m = m10();
        assert!((m.swatch - 11.0).abs() < 1e-9);
        assert!((m.line_h - 18.0).abs() < 1e-9);
        let big = Metrics::of(20.0);
        assert!((big.line_h - 36.0).abs() < 1e-9);
        assert!((big.entry_gap - 32.0).abs() < 1e-9);
    }

    // ── legend_width_from_advance ─────────────────────────────────────────────

    #[test]
    fn width_from_advance_zero() {
        let m = m10();
        let expected = m.pad + m.swatch + m.gap + m.pad;
        let got = legend_width_from_advance(0.0, m);
        assert!(
            (got - expected).abs() < 1e-9,
            "expected {expected}, got {got}"
        );
    }

    #[test]
    fn width_from_advance_nonzero() {
        let m = m10();
        let advance = 42.5;
        let expected = m.pad + m.swatch + m.gap + advance + m.pad;
        let got = legend_width_from_advance(advance, m);
        assert!(
            (got - expected).abs() < 1e-9,
            "expected {expected}, got {got}"
        );
    }

    // ── LegendPosition::from_opt ──────────────────────────────────────────────

    #[test]
    fn position_from_opt_known() {
        assert_eq!(LegendPosition::from_opt(Some("left")), LegendPosition::Left);
        assert_eq!(
            LegendPosition::from_opt(Some("right")),
            LegendPosition::Right
        );
        assert_eq!(LegendPosition::from_opt(Some("top")), LegendPosition::Top);
        assert_eq!(
            LegendPosition::from_opt(Some("bottom")),
            LegendPosition::Bottom
        );
    }

    #[test]
    fn position_from_opt_default() {
        // None and unknown strings both default to Right.
        assert_eq!(LegendPosition::from_opt(None), LegendPosition::Right);
        assert_eq!(
            LegendPosition::from_opt(Some("unknown")),
            LegendPosition::Right
        );
        assert_eq!(LegendPosition::from_opt(Some("")), LegendPosition::Right);
    }

    // ── LegendPosition::is_side ───────────────────────────────────────────────

    #[test]
    fn position_is_side() {
        assert!(LegendPosition::Left.is_side());
        assert!(LegendPosition::Right.is_side());
        assert!(!LegendPosition::Top.is_side());
        assert!(!LegendPosition::Bottom.is_side());
    }

    // ── LegendLayout::from_opt ────────────────────────────────────────────────

    #[test]
    fn layout_from_opt_known() {
        assert_eq!(LegendLayout::from_opt(Some("list")), LegendLayout::List);
        assert_eq!(
            LegendLayout::from_opt(Some("wrapped")),
            LegendLayout::Wrapped
        );
    }

    #[test]
    fn layout_from_opt_default() {
        // None and unknown strings default to Wrapped.
        assert_eq!(LegendLayout::from_opt(None), LegendLayout::Wrapped);
        assert_eq!(
            LegendLayout::from_opt(Some("unknown")),
            LegendLayout::Wrapped
        );
    }

    // ── LegendAlign::from_opt ─────────────────────────────────────────────────

    #[test]
    fn align_from_opt_known() {
        assert_eq!(LegendAlign::from_opt(Some("left")), LegendAlign::Start);
        assert_eq!(LegendAlign::from_opt(Some("right")), LegendAlign::End);
        assert_eq!(LegendAlign::from_opt(Some("center")), LegendAlign::Center);
    }

    #[test]
    fn align_from_opt_default() {
        // None and unknown strings default to Center.
        assert_eq!(LegendAlign::from_opt(None), LegendAlign::Center);
        assert_eq!(LegendAlign::from_opt(Some("unknown")), LegendAlign::Center);
    }

    // ── align_x ──────────────────────────────────────────────────────────────

    #[test]
    fn align_x_start() {
        let x = align_x(LegendAlign::Start, 50.0, 10.0, 200.0);
        assert!((x - 10.0).abs() < 1e-9, "start: expected 10, got {x}");
    }

    #[test]
    fn align_x_center() {
        // left=10, right=110, block=40 → center x = 10 + (100-40)/2 = 40
        let x = align_x(LegendAlign::Center, 40.0, 10.0, 110.0);
        assert!((x - 40.0).abs() < 1e-9, "center: expected 40, got {x}");
    }

    #[test]
    fn align_x_end() {
        // right=110, block=40 → end x = 110-40 = 70
        let x = align_x(LegendAlign::End, 40.0, 10.0, 110.0);
        assert!((x - 70.0).abs() < 1e-9, "end: expected 70, got {x}");
    }

    #[test]
    fn align_x_clamps_to_left_edge() {
        // Block wider than the available range → clamp to left_edge.
        let x = align_x(LegendAlign::End, 300.0, 10.0, 110.0);
        assert!((x - 10.0).abs() < 1e-9, "clamp: expected 10, got {x}");
    }

    // ── wrapped_row_count ─────────────────────────────────────────────────────

    #[test]
    fn wrapped_row_count_single_row() {
        // Each entry_content_w = 11+6+20 = 37; row_avail = 200-2*10 = 180.
        // 37 + 16+37 + 16+37 = 143 ≤ 180 → one row.
        let rows = wrapped_row_count(&[20.0, 20.0, 20.0], 200.0, m10());
        assert_eq!(rows, 1, "expected 1 row, got {rows}");
    }

    #[test]
    fn wrapped_row_count_wraps() {
        // Each entry_content_w = 11+6+200 = 217; row_avail = 80 → two rows.
        let rows = wrapped_row_count(&[200.0, 200.0], 100.0, m10());
        assert_eq!(rows, 2, "expected 2 rows, got {rows}");
    }

    #[test]
    fn wrapped_row_count_empty() {
        let rows = wrapped_row_count(&[], 200.0, m10());
        assert_eq!(rows, 1, "empty advances: expected min-1 row, got {rows}");
    }
}
