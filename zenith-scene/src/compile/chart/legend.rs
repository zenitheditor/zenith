//! Legend rendering for chart nodes.
//!
//! `legend_reserve` measures the pixel space (width or height) needed for a
//! legend strip; `emit_legend` pushes colored swatches and label glyph runs
//! into the command buffer. Both functions are no-ops when no entries are
//! supplied. Placement, layout, and alignment are controlled via `LegendConfig`.

use zenith_core::{Diagnostic, FontStyle};
use zenith_layout::{ShapeRequest, TextDirection, TextLayoutEngine};

use crate::ir::{Color, Paint, SceneCommand};

use super::super::NodeCtx;
use super::super::text::run_to_scene_glyphs;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Left padding inside the legend strip, before the swatch.
const PAD_L: f64 = 10.0;
/// Right padding inside the legend strip, after the label.
const PAD_R: f64 = 10.0;
/// Swatch square edge length (px).
const SWATCH: f64 = 11.0;
/// Gap between the right edge of the swatch and the start of the label.
const GAP: f64 = 6.0;
/// Vertical slot height per legend entry.
const LINE_H: f64 = 18.0;
/// Font size for legend labels (px).
const FONT: f32 = 10.0;
/// Default legend label text color (dark gray).
const LEGEND_TEXT_COLOR: Color = Color::srgb(60, 60, 60, 255);
/// Horizontal gap between wrapped entries within a single row.
const ENTRY_GAP: f64 = 16.0;
/// Top and bottom padding inside a top/bottom legend band.
const PAD_V: f64 = 8.0;

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

/// Compute the total legend strip width from the widest label advance.
///
/// `width = PAD_L + SWATCH + GAP + max_advance + PAD_R`
///
/// This pure helper is separated from the shaping loop so it can be unit-tested
/// without constructing a `NodeCtx`.
pub(super) fn legend_width_from_advance(max_advance: f64) -> f64 {
    PAD_L + SWATCH + GAP + max_advance + PAD_R
}

// ── entry_advances ────────────────────────────────────────────────────────────

/// Shape each label and return its advance width (px). Shaping errors yield `0.0`.
fn entry_advances(entries: &[(String, Color)], cx: NodeCtx<'_>) -> Vec<f64> {
    let families = [String::from("Noto Sans")];
    entries
        .iter()
        .map(|(label, _)| {
            let req = ShapeRequest {
                text: label,
                families: &families,
                weight: 400,
                style: FontStyle::Normal,
                font_size: FONT,
                direction: TextDirection::Ltr,
                features: &[],
            };
            match cx.engine.shape_with_fallback(&req, cx.fonts) {
                Ok(result) => result.runs.iter().map(|r| r.advance_width as f64).sum(),
                Err(_) => 0.0,
            }
        })
        .collect()
}

/// Pixel width that a single entry occupies (swatch + gap + label advance).
fn entry_content_w(advance: f64) -> f64 {
    SWATCH + GAP + advance
}

// ── legend_reserve ────────────────────────────────────────────────────────────

/// Compute the `(width_reserve, height_reserve)` the legend needs.
///
/// - Side (`Left`/`Right`): returns `(strip_w, 0.0)`.
/// - Band (`Top`/`Bottom`): returns `(0.0, band_h)`.
///
/// Returns `(0.0, 0.0)` when `entries` is empty.
pub(super) fn legend_reserve(
    entries: &[(String, Color)],
    config: LegendConfig,
    avail_w: f64,
    cx: NodeCtx<'_>,
) -> (f64, f64) {
    if entries.is_empty() {
        return (0.0, 0.0);
    }

    if config.position.is_side() {
        let advances = entry_advances(entries, cx);
        let max_advance = advances.into_iter().fold(0.0_f64, f64::max);
        return (legend_width_from_advance(max_advance), 0.0);
    }

    // Top / Bottom band.
    let height = match config.layout {
        LegendLayout::List => entries.len() as f64 * LINE_H + 2.0 * PAD_V,
        LegendLayout::Wrapped => {
            let advances = entry_advances(entries, cx);
            let rows = wrapped_row_count(&advances, avail_w);
            rows as f64 * LINE_H + 2.0 * PAD_V
        }
    };
    (0.0, height)
}

/// Count the number of rows needed to wrap `advances` into `avail_w`.
fn wrapped_row_count(advances: &[f64], avail_w: f64) -> usize {
    let row_avail = (avail_w - 2.0 * PAD_L).max(1.0);
    let mut rows: usize = 1;
    let mut cur: f64 = 0.0;

    for &adv in advances {
        let cw = entry_content_w(adv);
        if cur > 0.0 && cur + ENTRY_GAP + cw > row_avail {
            rows += 1;
            cur = cw;
        } else if cur > 0.0 {
            cur += ENTRY_GAP + cw;
        } else {
            cur = cw;
        }
    }

    rows.max(1)
}

/// Group entry indices into rows using the same greedy rule as `wrapped_row_count`.
fn wrapped_rows(advances: &[f64], avail_w: f64) -> Vec<Vec<usize>> {
    let row_avail = (avail_w - 2.0 * PAD_L).max(1.0);
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut cur_row: Vec<usize> = Vec::new();
    let mut cur: f64 = 0.0;

    for (i, &adv) in advances.iter().enumerate() {
        let cw = entry_content_w(adv);
        if cur > 0.0 && cur + ENTRY_GAP + cw > row_avail {
            rows.push(cur_row);
            cur_row = vec![i];
            cur = cw;
        } else if cur > 0.0 {
            cur += ENTRY_GAP + cw;
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

/// Shared text context for the legend draw passes — the caller-allocated font
/// family list plus the node context. Bundled so `draw_entry` stays within the
/// argument limit without re-allocating `families` per entry.
#[derive(Clone, Copy)]
struct DrawCtx<'a> {
    families: &'a [String],
    cx: NodeCtx<'a>,
}

/// Emit one swatch + label glyph run at `(swatch_x, line_top)`.
fn draw_entry(
    swatch_x: f64,
    line_top: f64,
    label: &str,
    color: Color,
    dctx: DrawCtx<'_>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let cx = dctx.cx;
    // Colored swatch square.
    let swatch_y = line_top + (LINE_H - SWATCH) / 2.0;
    commands.push(SceneCommand::FillRect {
        x: swatch_x,
        y: swatch_y,
        w: SWATCH,
        h: SWATCH,
        paint: Paint::solid(color),
    });

    // Label glyph run.
    let req = ShapeRequest {
        text: label,
        families: dctx.families,
        weight: 400,
        style: FontStyle::Normal,
        font_size: FONT,
        direction: TextDirection::Ltr,
        features: &[],
    };

    match cx.engine.shape_with_fallback(&req, cx.fonts) {
        Err(e) => {
            diagnostics.push(Diagnostic::advisory(
                "scene.text_unshaped",
                format!(
                    "chart legend label '{}' could not be shaped: {}",
                    label, e.message
                ),
                None,
                None,
            ));
        }
        Ok(result) => {
            let ascent: f64 = result.runs.first().map(|r| r.ascent as f64).unwrap_or(8.0);
            let baseline_y = line_top + LINE_H / 2.0 + ascent * 0.35;
            let mut text_x = swatch_x + SWATCH + GAP;

            for run in result.runs {
                let advance = run.advance_width as f64;
                let glyphs = run_to_scene_glyphs(&run);
                commands.push(SceneCommand::DrawGlyphRun {
                    x: text_x,
                    y: baseline_y,
                    font_id: run.font_id,
                    font_size: run.font_size,
                    color: LEGEND_TEXT_COLOR,
                    stroke_color: None,
                    stroke_width: None,
                    link: None,
                    selectable: true,
                    glyphs,
                });
                text_x += advance;
            }
        }
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
    cx: NodeCtx<'_>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if area.w <= 0.0 || area.h <= 0.0 || entries.is_empty() {
        return;
    }

    let area_bottom = area.y + area.h;
    let left_edge = area.x + PAD_L;
    let right_edge = area.x + area.w - PAD_R;

    if config.position.is_side() {
        emit_legend_side(entries, area, area_bottom, cx, commands, diagnostics);
    } else {
        emit_legend_band(
            entries,
            area,
            config,
            BandGeom {
                area_bottom,
                left_edge,
                right_edge,
            },
            cx,
            commands,
            diagnostics,
        );
    }
}

/// Emit a vertical list strip (Left / Right positions).
fn emit_legend_side(
    entries: &[(String, Color)],
    area: LegendArea,
    area_bottom: f64,
    cx: NodeCtx<'_>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let n = entries.len() as f64;
    let total_h = n * LINE_H;
    let start_y = (area.y + (area.h - total_h) / 2.0).max(area.y);
    let swatch_x = area.x + PAD_L;
    let families = [String::from("Noto Sans")];
    let dctx = DrawCtx {
        families: &families,
        cx,
    };

    for (i, (label, color)) in entries.iter().enumerate() {
        let line_top = start_y + i as f64 * LINE_H;
        if line_top >= area_bottom {
            break;
        }
        draw_entry(
            swatch_x,
            line_top,
            label,
            *color,
            dctx,
            commands,
            diagnostics,
        );
    }
}

/// Geometry derived from `LegendArea` for band layout; bundles three computed
/// scalars so `emit_legend_band` stays within the 7-arg limit.
struct BandGeom {
    area_bottom: f64,
    left_edge: f64,
    right_edge: f64,
}

/// Emit a horizontal band (Top / Bottom positions) in List or Wrapped layout.
fn emit_legend_band(
    entries: &[(String, Color)],
    area: LegendArea,
    config: LegendConfig,
    geom: BandGeom,
    cx: NodeCtx<'_>,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let BandGeom {
        area_bottom,
        left_edge,
        right_edge,
    } = geom;
    let advances = entry_advances(entries, cx);
    let families = [String::from("Noto Sans")];
    let dctx = DrawCtx {
        families: &families,
        cx,
    };

    match config.layout {
        LegendLayout::List => {
            let block_w = advances
                .iter()
                .map(|&a| entry_content_w(a))
                .fold(0.0_f64, f64::max);
            let total_h = entries.len() as f64 * LINE_H;
            let start_y = (area.y + (area.h - total_h) / 2.0).max(area.y);
            let block_x = align_x(config.align, block_w, left_edge, right_edge);

            for (i, (label, color)) in entries.iter().enumerate() {
                let line_top = start_y + i as f64 * LINE_H;
                if line_top >= area_bottom {
                    break;
                }
                draw_entry(
                    block_x,
                    line_top,
                    label,
                    *color,
                    dctx,
                    commands,
                    diagnostics,
                );
            }
        }
        LegendLayout::Wrapped => {
            let rows = wrapped_rows(&advances, area.w);
            let total_rows_h = rows.len() as f64 * LINE_H;
            let start_y = (area.y + (area.h - total_rows_h) / 2.0).max(area.y);

            for (row_idx, row_indices) in rows.iter().enumerate() {
                let line_top = start_y + row_idx as f64 * LINE_H;
                if line_top >= area_bottom {
                    break;
                }

                // Compute this row's total content width.
                let row_w: f64 = row_indices.iter().enumerate().fold(0.0, |acc, (j, &ei)| {
                    let cw = entry_content_w(advances.get(ei).copied().unwrap_or(0.0));
                    if j == 0 {
                        acc + cw
                    } else {
                        acc + ENTRY_GAP + cw
                    }
                });

                let row_x0 = align_x(config.align, row_w, left_edge, right_edge);
                let mut x = row_x0;

                for (j, &ei) in row_indices.iter().enumerate() {
                    if j > 0 {
                        x += ENTRY_GAP;
                    }
                    let (label, color) = match entries.get(ei) {
                        Some(e) => e,
                        None => continue,
                    };
                    draw_entry(x, line_top, label, *color, dctx, commands, diagnostics);
                    x += entry_content_w(advances.get(ei).copied().unwrap_or(0.0));
                }
            }
        }
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── legend_width_from_advance ─────────────────────────────────────────────

    #[test]
    fn width_from_advance_zero() {
        // Zero advance → PAD_L + SWATCH + GAP + 0 + PAD_R
        let expected = PAD_L + SWATCH + GAP + PAD_R;
        let got = legend_width_from_advance(0.0);
        assert!(
            (got - expected).abs() < 1e-9,
            "expected {expected}, got {got}"
        );
    }

    #[test]
    fn width_from_advance_nonzero() {
        let advance = 42.5;
        let expected = PAD_L + SWATCH + GAP + advance + PAD_R;
        let got = legend_width_from_advance(advance);
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

    // ── emit_legend (engine-free) ─────────────────────────────────────────────
    // Engine-dependent shaping tests are omitted: building a NodeCtx requires
    // a RustybuzzEngine and FontProvider which are not available in unit-test
    // context. The pure arithmetic is exercised above; integration/conformance
    // tests cover the full shaping + rendering path.

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
        // Three small entries that all fit in 200 px.
        let advances = vec![20.0, 20.0, 20.0];
        // Each entry_content_w = 11+6+20 = 37; row_avail = (200-2*10).max(1) = 180
        // 37 + 16+37 + 16+37 = 143 ≤ 180 → one row
        let rows = wrapped_row_count(&advances, 200.0);
        assert_eq!(rows, 1, "expected 1 row, got {rows}");
    }

    #[test]
    fn wrapped_row_count_wraps() {
        // Very large entries that each need their own row.
        let advances = vec![200.0, 200.0];
        // Each entry_content_w = 11+6+200 = 217; row_avail = (100-20).max(1) = 80
        // first entry: cur=217; second: 217+16+217 > 80 → new row
        let rows = wrapped_row_count(&advances, 100.0);
        assert_eq!(rows, 2, "expected 2 rows, got {rows}");
    }

    #[test]
    fn wrapped_row_count_empty() {
        let rows = wrapped_row_count(&[], 200.0);
        assert_eq!(rows, 1, "empty advances: expected min-1 row, got {rows}");
    }
}
