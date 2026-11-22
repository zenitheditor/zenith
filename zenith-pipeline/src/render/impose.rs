//! Print sheet imposition: arrange reading-order pages (1, 2, 3, 4, …) onto
//! press-ready folded sheets (booklet / bi-fold, tri-fold, Z-fold, 2-up spread)
//! with outer bleed preservation, optional crop/fold marks, duplex rotation,
//! saddle-stitch creep compensation, and multi-signature grouping.

use zenith_core::PX_PER_INCH;
use zenith_scene::{Color, LineCap, Rect, Scene, SceneCommand};

use crate::error::PipelineError;

/// Millimeters per inch (exact ISO definition).
const MM_PER_INCH: f64 = 25.4;

/// Minimum outer slug margin (in px, ~6.35 mm) when `--marks` is active.
const MARKS_MIN_SLUG_PX: f64 = 24.0;

/// Gap between trim edge and crop/fold mark start (~2.1 mm).
const MARK_OFFSET_PX: f64 = 8.0;

/// Length of each crop/fold mark segment (~3.2 mm).
const MARK_LEN_PX: f64 = 12.0;

/// Hairline mark stroke width (`0.5 pt` at 96 px/in).
const MARK_STROKE_PX: f64 = 0.67;

/// How reading-order pages are arranged onto physical press sheets for PDF output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImposeMode {
    /// 2-up saddle-stitch / bi-fold imposition. Pads to a multiple of 4 pages.
    /// For 4 pages: sheet 1 = `[4 | 1]` (outside), sheet 2 = `[2 | 3]` (inside).
    /// For 8 pages: `[8 | 1]`, `[2 | 7]`, `[6 | 3]`, `[4 | 5]`.
    Booklet,
    /// 3-up roll-fold / letter-fold brochure imposition (6 panels per sheet pair).
    /// Pads to a multiple of 6 pages.
    /// Sheet 1 (outside) = `[5 | 6 | 1]`, sheet 2 (inside) = `[2 | 3 | 4]`.
    Trifold,
    /// 3-up Z-fold / accordion brochure imposition (6 panels per sheet pair).
    /// Pads to a multiple of 6 pages.
    /// Sheet 1 (front) = `[5 | 4 | 1]`, sheet 2 (back) = `[2 | 3 | 6]`.
    Zfold,
    /// Simple 2-up side-by-side spreads in reading order: `[1 | 2]`, `[3 | 4]`, …
    /// Pads to a multiple of 2 pages.
    Spread,
    /// Sequential N-up gang-up grid (`cols × rows` per sheet, left-to-right, top-to-bottom):
    /// sheet 1 = `[1, 2, …, N]`, sheet 2 = `[N+1, …, 2N]`, …
    Grid {
        /// Columns per sheet (`1..=32`).
        cols: usize,
        /// Rows per sheet (`1..=32`).
        rows: usize,
    },
    /// Stack-ordered N-up cut-and-stack imposition (`cols × rows` per sheet).
    /// For `P` pages and `N = cols * rows` slots, produces `S = ceil(P / N)` sheets
    /// where sheet `k` slot `i` holds page `i * S + k + 1` so guillotine-cutting
    /// the stack into `N` piles and stacking them yields sequential order `1..=P`.
    CutAndStack {
        /// Columns per sheet (`1..=32`).
        cols: usize,
        /// Rows per sheet (`1..=32`).
        rows: usize,
    },
}

/// Default N-up grid dimensions (`2×2`, 4-up) when `--grid` is omitted for
/// `grid` or `cut-and-stack` imposition.
pub const DEFAULT_GRID: (usize, usize) = (2, 2);

/// Maximum columns or rows allowed in a single N-up imposition grid.
const MAX_GRID_SIDE: usize = 32;

/// Parse a `<cols>x<rows>` grid specification (e.g. `"2x4"`, `"3x3"`).
///
/// Requires `1 <= cols <= 32`, `1 <= rows <= 32`, and `cols * rows >= 2`.
///
/// # Errors
///
/// Returns a descriptive message when the string is malformed or out of bounds.
pub fn parse_grid_spec(raw: &str) -> Result<(usize, usize), String> {
    let trimmed = raw.trim();
    let Some((c_str, r_str)) = trimmed.split_once(['x', 'X']) else {
        return Err(format!(
            "invalid grid '{raw}': expected <cols>x<rows> (e.g. 2x2, 2x4)"
        ));
    };
    let cols: usize = c_str
        .parse()
        .map_err(|_| format!("invalid grid columns '{c_str}' in '{raw}'"))?;
    let rows: usize = r_str
        .parse()
        .map_err(|_| format!("invalid grid rows '{r_str}' in '{raw}'"))?;
    if cols == 0 || rows == 0 || cols > MAX_GRID_SIDE || rows > MAX_GRID_SIDE {
        return Err(format!(
            "grid '{raw}' out of range: cols and rows must be in 1..={MAX_GRID_SIDE}"
        ));
    }
    if cols * rows < 2 {
        return Err(format!(
            "grid '{raw}' must place at least 2 pages per sheet (e.g. 2x1, 2x2)"
        ));
    }
    Ok((cols, rows))
}

/// Duplex flip orientation for back sheets of an imposed press PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DuplexMode {
    /// Standard short-edge flip (landscape booklets): front and back sheets stay upright.
    #[default]
    ShortEdge,
    /// Long-edge flip: back sheets (even 1-based sheets 2, 4, …) rotate 180° around sheet center.
    LongEdge,
}

/// Complete print sheet imposition configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImposeOptions {
    /// Sheet arrangement mode (`booklet`, `trifold`, `zfold`, `spread`).
    pub mode: ImposeMode,
    /// Duplex flip orientation for back sheets (default: `ShortEdge`).
    pub duplex: DuplexMode,
    /// Total saddle-stitch creep compensation in millimeters (`0.0` = off).
    /// Distributed linearly from `0` on the outermost sheet to `creep_mm` on
    /// the innermost centerfold sheet. Only applies to `ImposeMode::Booklet`.
    pub creep_mm: f64,
    /// Optional multi-signature size for `ImposeMode::Booklet` (positive multiple
    /// of 4, e.g. `16`). `None` imposes the entire document as one signature.
    pub signature: Option<usize>,
    /// Draw sheet-level K-only corner crop marks and fold-crease tick marks in
    /// the outer margin.
    pub marks: bool,
}

impl ImposeOptions {
    /// Default imposition options for `mode`: short-edge duplex, no creep,
    /// single signature, no crop/fold marks.
    #[must_use]
    pub fn new(mode: ImposeMode) -> Self {
        Self {
            mode,
            duplex: DuplexMode::ShortEdge,
            creep_mm: 0.0,
            signature: None,
            marks: false,
        }
    }
}

/// One planned sheet: panel slots (`None` = blank) in row-major order, grid
/// dimensions (`cols × rows`), creep shift (px) inward toward the spine, and
/// whether this is a back side.
struct SheetSpec {
    slots: Vec<Option<usize>>,
    cols: usize,
    rows: usize,
    creep_px: f64,
    is_back: bool,
}

/// Arrange reading-order `scenes` onto imposed press sheets according to `opts`.
///
/// Every page in `scenes` must share the same dimensions (within `0.5 px`).
/// When source pages declare a print bleed (`Scene.trim`), outer sheet edges
/// preserve that bleed while internal fold/spine edges meet flush at trim.
/// Missing trailing pages needed to complete a signature are left blank.
///
/// # Errors
///
/// Returns `render.impose_empty` if `scenes` is empty,
/// `render.impose_size_mismatch` if pages have differing dimensions, or
/// `render.impose_invalid` if `creep_mm` or `signature` is invalid.
pub(super) fn impose_scenes(
    scenes: &[Scene],
    opts: ImposeOptions,
) -> Result<Vec<Scene>, PipelineError> {
    let Some(first) = scenes.first() else {
        return Err(PipelineError::new(
            "render.impose_empty",
            "cannot impose an empty document; add at least one page",
            2,
        ));
    };
    if !opts.creep_mm.is_finite() || opts.creep_mm < 0.0 {
        return Err(PipelineError::new(
            "render.impose_invalid",
            format!(
                "impose creep must be finite and >= 0 mm, got {}",
                opts.creep_mm
            ),
            2,
        ));
    }
    if opts.creep_mm > 0.0 && opts.mode != ImposeMode::Booklet {
        return Err(PipelineError::new(
            "render.impose_invalid",
            "creep compensation is only supported with --impose booklet",
            2,
        ));
    }
    if let Some(sig) = opts.signature {
        if opts.mode != ImposeMode::Booklet {
            return Err(PipelineError::new(
                "render.impose_invalid",
                "multi-signature imposition is only supported with --impose booklet",
                2,
            ));
        }
        if sig == 0 || sig % 4 != 0 {
            return Err(PipelineError::new(
                "render.impose_invalid",
                format!("signature size must be a positive multiple of 4, got {sig}"),
                2,
            ));
        }
    }

    if let ImposeMode::Grid { cols, rows } | ImposeMode::CutAndStack { cols, rows } = opts.mode
        && (cols == 0
            || rows == 0
            || cols > MAX_GRID_SIDE
            || rows > MAX_GRID_SIDE
            || cols * rows < 2)
    {
        return Err(PipelineError::new(
            "render.impose_invalid",
            format!(
                "impose grid {cols}x{rows} is invalid: cols and rows must be in 1..={MAX_GRID_SIDE} with cols*rows >= 2"
            ),
            2,
        ));
    }

    let (page_w, page_h) = (first.width, first.height);
    let (bleed_x, bleed_y, trim_w, trim_h) = match first.trim {
        Some(t) => (t.x, t.y, t.w, t.h),
        None => (0.0, 0.0, page_w, page_h),
    };
    for (idx, scene) in scenes.iter().enumerate().skip(1) {
        if (scene.width - page_w).abs() > 0.5 || (scene.height - page_h).abs() > 0.5 {
            return Err(PipelineError::new(
                "render.impose_size_mismatch",
                format!(
                    "imposition requires uniform page sizes: page 1 is {page_w}x{page_h} px, but page {} is {}x{} px",
                    idx + 1,
                    scene.width,
                    scene.height
                ),
                2,
            ));
        }
    }

    let plan = sheet_plan(scenes.len(), opts);
    let mut out = Vec::with_capacity(plan.len());
    for spec in plan {
        out.push(compose_sheet(
            scenes,
            &spec,
            (bleed_x, bleed_y, trim_w, trim_h),
            opts,
        ));
    }
    Ok(out)
}

/// Compute the ordered list of press sheets (slots, per-sheet creep, back-side flag).
fn sheet_plan(page_count: usize, opts: ImposeOptions) -> Vec<SheetSpec> {
    let creep_total_px = opts.creep_mm * (PX_PER_INCH / MM_PER_INCH);
    let slot = |one_based: usize| -> Option<usize> {
        let zero_based = one_based.checked_sub(1)?;
        (zero_based < page_count).then_some(zero_based)
    };

    match opts.mode {
        ImposeMode::Booklet => {
            let sig_size = opts.signature.unwrap_or_else(|| page_count.div_ceil(4) * 4);
            let sig_count = page_count.div_ceil(sig_size);
            let pairs_per_sig = sig_size / 4;
            let mut sheets = Vec::with_capacity(sig_count * pairs_per_sig * 2);
            for s in 0..sig_count {
                let base = s * sig_size;
                for k in 0..pairs_per_sig {
                    let creep_px = if pairs_per_sig > 1 {
                        creep_total_px * (k as f64) / ((pairs_per_sig - 1) as f64)
                    } else {
                        0.0
                    };
                    // Outside (front of physical sheet): [sig_size - 2k, 2k + 1]
                    sheets.push(SheetSpec {
                        slots: vec![slot(base + sig_size - 2 * k), slot(base + 2 * k + 1)],
                        cols: 2,
                        rows: 1,
                        creep_px,
                        is_back: false,
                    });
                    // Inside (back of physical sheet): [2k + 2, sig_size - 2k - 1]
                    sheets.push(SheetSpec {
                        slots: vec![slot(base + 2 * k + 2), slot(base + sig_size - 2 * k - 1)],
                        cols: 2,
                        rows: 1,
                        creep_px,
                        is_back: true,
                    });
                }
            }
            sheets
        }
        ImposeMode::Trifold => {
            let sigs = page_count.div_ceil(6);
            let mut sheets = Vec::with_capacity(sigs * 2);
            for s in 0..sigs {
                let base = s * 6;
                // Outside: [5, 6, 1] (tucked flap | back cover | front cover)
                sheets.push(SheetSpec {
                    slots: vec![slot(base + 5), slot(base + 6), slot(base + 1)],
                    cols: 3,
                    rows: 1,
                    creep_px: 0.0,
                    is_back: false,
                });
                // Inside: [2, 3, 4]
                sheets.push(SheetSpec {
                    slots: vec![slot(base + 2), slot(base + 3), slot(base + 4)],
                    cols: 3,
                    rows: 1,
                    creep_px: 0.0,
                    is_back: true,
                });
            }
            sheets
        }
        ImposeMode::Zfold => {
            let sigs = page_count.div_ceil(6);
            let mut sheets = Vec::with_capacity(sigs * 2);
            for s in 0..sigs {
                let base = s * 6;
                // Front: [5, 4, 1] (back of 6 | back of 3 | front cover)
                sheets.push(SheetSpec {
                    slots: vec![slot(base + 5), slot(base + 4), slot(base + 1)],
                    cols: 3,
                    rows: 1,
                    creep_px: 0.0,
                    is_back: false,
                });
                // Back: [2, 3, 6]
                sheets.push(SheetSpec {
                    slots: vec![slot(base + 2), slot(base + 3), slot(base + 6)],
                    cols: 3,
                    rows: 1,
                    creep_px: 0.0,
                    is_back: true,
                });
            }
            sheets
        }
        ImposeMode::Spread => {
            let pairs = page_count.div_ceil(2);
            let mut sheets = Vec::with_capacity(pairs);
            for k in 0..pairs {
                let base = k * 2;
                sheets.push(SheetSpec {
                    slots: vec![slot(base + 1), slot(base + 2)],
                    cols: 2,
                    rows: 1,
                    creep_px: 0.0,
                    is_back: false,
                });
            }
            sheets
        }
        ImposeMode::Grid { cols, rows } => {
            let per_sheet = cols * rows;
            let sheet_count = page_count.div_ceil(per_sheet);
            let mut sheets = Vec::with_capacity(sheet_count);
            for k in 0..sheet_count {
                let base = k * per_sheet;
                let slots = (0..per_sheet).map(|i| slot(base + i + 1)).collect();
                sheets.push(SheetSpec {
                    slots,
                    cols,
                    rows,
                    creep_px: 0.0,
                    is_back: false,
                });
            }
            sheets
        }
        ImposeMode::CutAndStack { cols, rows } => {
            let per_sheet = cols * rows;
            let sheet_count = page_count.div_ceil(per_sheet);
            let mut sheets = Vec::with_capacity(sheet_count);
            for k in 0..sheet_count {
                let slots = (0..per_sheet)
                    .map(|i| slot(i * sheet_count + k + 1))
                    .collect();
                sheets.push(SheetSpec {
                    slots,
                    cols,
                    rows,
                    creep_px: 0.0,
                    is_back: false,
                });
            }
            sheets
        }
    }
}

/// Compose one physical sheet by placing each panel slot on a `cols × rows` grid,
/// preserving outer bleed while meeting flush at internal fold/cut edges.
fn compose_sheet(
    scenes: &[Scene],
    spec: &SheetSpec,
    (bleed_x, bleed_y, trim_w, trim_h): (f64, f64, f64, f64),
    opts: ImposeOptions,
) -> Scene {
    let cols = spec.cols;
    let rows = spec.rows;
    let sheet_trim_w = trim_w * cols as f64;
    let sheet_trim_h = trim_h * rows as f64;
    let margin_x = if opts.marks {
        bleed_x.max(MARKS_MIN_SLUG_PX)
    } else {
        bleed_x
    };
    let margin_y = if opts.marks {
        bleed_y.max(MARKS_MIN_SLUG_PX)
    } else {
        bleed_y
    };
    let sheet_w = sheet_trim_w + 2.0 * margin_x;
    let sheet_h = sheet_trim_h + 2.0 * margin_y;

    let mut sheet = Scene::new(sheet_w, sheet_h);
    if margin_x > 0.0 || margin_y > 0.0 {
        sheet.trim = Some(Rect {
            x: margin_x,
            y: margin_y,
            w: sheet_trim_w,
            h: sheet_trim_h,
        });
    }

    let rotate_back = spec.is_back && opts.duplex == DuplexMode::LongEdge;
    if rotate_back {
        sheet.commands.push(SceneCommand::PushTransform {
            angle_deg: 180.0,
            cx: sheet_w / 2.0,
            cy: sheet_h / 2.0,
        });
    }

    for (idx, page_slot) in spec.slots.iter().enumerate() {
        let Some(page_idx) = *page_slot else {
            continue;
        };
        let Some(page) = scenes.get(page_idx) else {
            continue;
        };
        let row = idx / cols;
        let col = idx % cols;
        let panel_trim_x = margin_x + trim_w * col as f64;
        let panel_trim_y = margin_y + trim_h * row as f64;
        // Outer sheet edges preserve bleed; internal fold/cut edges clip flush at trim.
        let clip_x = if col == 0 {
            margin_x - bleed_x
        } else {
            panel_trim_x
        };
        let clip_right = if col + 1 == cols {
            margin_x + sheet_trim_w + bleed_x
        } else {
            panel_trim_x + trim_w
        };
        let clip_y = if row == 0 {
            margin_y - bleed_y
        } else {
            panel_trim_y
        };
        let clip_bottom = if row + 1 == rows {
            margin_y + sheet_trim_h + bleed_y
        } else {
            panel_trim_y + trim_h
        };

        // Saddle-stitch creep shifts left panel right (+creep) and right panel left (-creep).
        let creep_shift = if spec.creep_px > 0.0 && cols == 2 && rows == 1 {
            if col == 0 {
                spec.creep_px
            } else {
                -spec.creep_px
            }
        } else {
            0.0
        };
        let tx = (panel_trim_x - bleed_x) + creep_shift;
        let ty = panel_trim_y - bleed_y;

        // Strip per-page trailing crop marks (8 StrokeLines emitted by scene compiler when bleed > 0)
        // so only clean sheet-level marks appear when `--marks` is enabled.
        let panel_cmds = strip_panel_crop_marks(page);

        sheet.commands.push(SceneCommand::PushClip {
            x: clip_x,
            y: clip_y,
            w: clip_right - clip_x,
            h: clip_bottom - clip_y,
        });
        if tx != 0.0 || ty != 0.0 {
            sheet.commands.push(SceneCommand::PushScaleTranslate {
                sx: 1.0,
                sy: 1.0,
                tx,
                ty,
            });
        }
        sheet.commands.extend(panel_cmds.iter().cloned());
        if tx != 0.0 || ty != 0.0 {
            sheet.commands.push(SceneCommand::PopTransform);
        }
        sheet.commands.push(SceneCommand::PopClip);
    }

    if opts.marks {
        let dashed_internal = matches!(
            opts.mode,
            ImposeMode::Booklet | ImposeMode::Trifold | ImposeMode::Zfold | ImposeMode::Spread
        );
        emit_sheet_marks(
            &mut sheet.commands,
            (margin_x, margin_y, sheet_trim_w, sheet_trim_h),
            (cols, rows),
            (trim_w, trim_h),
            dashed_internal,
        );
    }

    if rotate_back {
        sheet.commands.push(SceneCommand::PopTransform);
    }

    sheet
}

/// If `page` has a print bleed, strip the 8 trailing per-page corner crop-mark
/// `StrokeLine` commands emitted by the page compiler so imposed sheets never
/// carry stray interior crop marks.
fn strip_panel_crop_marks(page: &Scene) -> &[SceneCommand] {
    if page.trim.is_some()
        && let Some(split_at) = page.commands.len().checked_sub(8)
        && let Some((head, tail)) = page.commands.split_at_checked(split_at)
        && tail
            .iter()
            .all(|cmd| matches!(cmd, SceneCommand::StrokeLine { .. }))
    {
        head
    } else {
        &page.commands
    }
}

/// Emit K-only corner crop marks at the 4 sheet trim corners and internal fold
/// (dashed) or cut (solid) tick marks in the outer margins.
fn emit_sheet_marks(
    commands: &mut Vec<SceneCommand>,
    (left, top, trim_w, trim_h): (f64, f64, f64, f64),
    (cols, rows): (usize, usize),
    (panel_trim_w, panel_trim_h): (f64, f64),
    dashed_internal: bool,
) {
    let right = left + trim_w;
    let bottom = top + trim_h;
    let black = Color::cmyk(0.0, 0.0, 0.0, 100.0, 0, 0, 0);
    let mut line = |x1: f64, y1: f64, x2: f64, y2: f64, dash: Option<f64>| {
        commands.push(SceneCommand::StrokeLine {
            x1,
            y1,
            x2,
            y2,
            color: black,
            stroke_width: MARK_STROKE_PX,
            stroke_dash: dash,
            stroke_gap: dash,
            stroke_linecap: Some(LineCap::Butt),
        });
    };

    // 4 outer trim corners (2 solid segments per corner, strictly outside trim).
    let o = MARK_OFFSET_PX;
    let l = MARK_LEN_PX;
    // Top-left
    line(left - o - l, top, left - o, top, None);
    line(left, top - o - l, left, top - o, None);
    // Top-right
    line(right + o, top, right + o + l, top, None);
    line(right, top - o - l, right, top - o, None);
    // Bottom-left
    line(left - o - l, bottom, left - o, bottom, None);
    line(left, bottom + o, left, bottom + o + l, None);
    // Bottom-right
    line(right + o, bottom, right + o + l, bottom, None);
    line(right, bottom + o, right, bottom + o + l, None);

    // Internal column fold/cut tick marks (strictly outside trim).
    let tick_dash = dashed_internal.then_some(3.0);
    for col in 1..cols {
        let fx = left + panel_trim_w * col as f64;
        line(fx, top - o - l, fx, top - o, tick_dash);
        line(fx, bottom + o, fx, bottom + o + l, tick_dash);
    }
    // Internal row cut tick marks (strictly outside trim).
    for row in 1..rows {
        let fy = top + panel_trim_h * row as f64;
        line(left - o - l, fy, left - o, fy, tick_dash);
        line(right + o, fy, right + o + l, fy, tick_dash);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slots_only(plan: &[SheetSpec]) -> Vec<Vec<Option<usize>>> {
        plan.iter().map(|s| s.slots.clone()).collect()
    }

    #[test]
    fn booklet_plan_for_4_and_8_pages_and_multi_signature() {
        assert_eq!(
            slots_only(&sheet_plan(4, ImposeOptions::new(ImposeMode::Booklet))),
            vec![vec![Some(3), Some(0)], vec![Some(1), Some(2)]]
        );
        assert_eq!(
            slots_only(&sheet_plan(8, ImposeOptions::new(ImposeMode::Booklet))),
            vec![
                vec![Some(7), Some(0)],
                vec![Some(1), Some(6)],
                vec![Some(5), Some(2)],
                vec![Some(3), Some(4)],
            ]
        );
        // 8 pages split into two 4-page signatures: [4|1],[2|3] then [8|5],[6|7].
        let multi = ImposeOptions {
            signature: Some(4),
            ..ImposeOptions::new(ImposeMode::Booklet)
        };
        assert_eq!(
            slots_only(&sheet_plan(8, multi)),
            vec![
                vec![Some(3), Some(0)],
                vec![Some(1), Some(2)],
                vec![Some(7), Some(4)],
                vec![Some(5), Some(6)],
            ]
        );
    }

    #[test]
    fn trifold_zfold_and_spread_plans() {
        assert_eq!(
            slots_only(&sheet_plan(6, ImposeOptions::new(ImposeMode::Trifold))),
            vec![
                vec![Some(4), Some(5), Some(0)],
                vec![Some(1), Some(2), Some(3)],
            ]
        );
        assert_eq!(
            slots_only(&sheet_plan(6, ImposeOptions::new(ImposeMode::Zfold))),
            vec![
                vec![Some(4), Some(3), Some(0)],
                vec![Some(1), Some(2), Some(5)],
            ]
        );
        assert_eq!(
            slots_only(&sheet_plan(3, ImposeOptions::new(ImposeMode::Spread))),
            vec![vec![Some(0), Some(1)], vec![Some(2), None]]
        );
    }

    #[test]
    fn impose_validates_sizes_creep_and_signature() {
        let mismatched = vec![Scene::new(561.28, 793.6), Scene::new(600.0, 793.6)];
        let err = impose_scenes(&mismatched, ImposeOptions::new(ImposeMode::Booklet)).unwrap_err();
        assert!(
            err.message.contains("uniform page sizes"),
            "{}",
            err.message
        );

        let pages = vec![Scene::new(100.0, 140.0); 4];
        let bad_sig = ImposeOptions {
            signature: Some(6),
            ..ImposeOptions::new(ImposeMode::Booklet)
        };
        assert!(impose_scenes(&pages, bad_sig).is_err());

        let bad_creep = ImposeOptions {
            creep_mm: 1.0,
            ..ImposeOptions::new(ImposeMode::Trifold)
        };
        assert!(impose_scenes(&pages, bad_creep).is_err());
    }

    #[test]
    fn impose_preserves_outer_bleed_and_adds_marks_duplex_and_creep() {
        let bleed = 12.0;
        let mut pages = Vec::new();
        for _ in 0..8 {
            let mut s = Scene::new(100.0 + 2.0 * bleed, 140.0 + 2.0 * bleed);
            s.trim = Some(Rect {
                x: bleed,
                y: bleed,
                w: 100.0,
                h: 140.0,
            });
            pages.push(s);
        }
        let opts = ImposeOptions {
            mode: ImposeMode::Booklet,
            duplex: DuplexMode::LongEdge,
            creep_mm: 2.54, // 9.6 px on inner sheet
            signature: None,
            marks: true,
        };
        let sheets = impose_scenes(&pages, opts).unwrap();
        assert_eq!(sheets.len(), 4);
        // Marks force 24 px slug margin on each side around 200x140 trim box.
        assert!((sheets[0].width - 248.0).abs() < 1e-6);
        assert!((sheets[0].height - 188.0).abs() < 1e-6);
        assert_eq!(
            sheets[0].trim,
            Some(Rect {
                x: 24.0,
                y: 24.0,
                w: 200.0,
                h: 140.0,
            })
        );
        // Front sheet has no 180° rotation; back sheet starts with 180° PushTransform.
        assert!(!matches!(
            sheets[0].commands.first(),
            Some(SceneCommand::PushTransform { .. })
        ));
        assert!(matches!(
            sheets[1].commands.first(),
            Some(SceneCommand::PushTransform { angle_deg, .. }) if (*angle_deg - 180.0).abs() < 1e-6
        ));
        // Marks emitted: 8 corner crop lines + 2 fold ticks = 10 StrokeLines.
        let mark_lines = sheets[0]
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::StrokeLine { .. }))
            .count();
        assert_eq!(mark_lines, 10);
    }

    #[test]
    fn grid_and_cut_and_stack_plans_and_2d_marks() {
        // 10 pages on a 2x2 grid (4-up) -> 3 sheets.
        assert_eq!(
            slots_only(&sheet_plan(
                10,
                ImposeOptions::new(ImposeMode::Grid { cols: 2, rows: 2 })
            )),
            vec![
                vec![Some(0), Some(1), Some(2), Some(3)],
                vec![Some(4), Some(5), Some(6), Some(7)],
                vec![Some(8), Some(9), None, None],
            ]
        );
        // Cut-and-stack for 10 pages on 2x2 (S=3 sheets):
        // sheet 0 = [1, 4, 7, 10], sheet 1 = [2, 5, 8, _], sheet 2 = [3, 6, 9, _].
        assert_eq!(
            slots_only(&sheet_plan(
                10,
                ImposeOptions::new(ImposeMode::CutAndStack { cols: 2, rows: 2 })
            )),
            vec![
                vec![Some(0), Some(3), Some(6), Some(9)],
                vec![Some(1), Some(4), Some(7), None],
                vec![Some(2), Some(5), Some(8), None],
            ]
        );

        let pages = vec![Scene::new(100.0, 60.0); 10];
        let opts = ImposeOptions {
            marks: true,
            ..ImposeOptions::new(ImposeMode::CutAndStack { cols: 2, rows: 2 })
        };
        let sheets = impose_scenes(&pages, opts).unwrap();
        assert_eq!(sheets.len(), 3);
        // 2x2 of 100x60 = 200x120 trim + 24px slug on each side = 248x168.
        assert!((sheets[0].width - 248.0).abs() < 1e-6);
        assert!((sheets[0].height - 168.0).abs() < 1e-6);
        // 8 corner lines + 2 column cut ticks + 2 row cut ticks = 12 solid StrokeLines.
        let mark_lines: Vec<_> = sheets[0]
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::StrokeLine { stroke_dash, .. } => Some(*stroke_dash),
                _ => None,
            })
            .collect();
        assert_eq!(mark_lines.len(), 12);
        assert!(
            mark_lines.iter().all(Option::is_none),
            "cut-and-stack cut ticks must be solid"
        );
    }

    #[test]
    fn parse_grid_spec_validates_bounds() {
        assert_eq!(parse_grid_spec("2x4"), Ok((2, 4)));
        assert_eq!(parse_grid_spec("3X2"), Ok((3, 2)));
        assert!(parse_grid_spec("1x1").is_err());
        assert!(parse_grid_spec("0x2").is_err());
        assert!(parse_grid_spec("33x2").is_err());
        assert!(parse_grid_spec("2by4").is_err());
    }
}
