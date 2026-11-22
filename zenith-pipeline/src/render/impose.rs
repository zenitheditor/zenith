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

/// One planned sheet: panel slots (`None` = blank), creep shift (px) inward
/// toward the spine for left/right panels, and whether this is a back side.
struct SheetSpec {
    slots: Vec<Option<usize>>,
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
                        creep_px,
                        is_back: false,
                    });
                    // Inside (back of physical sheet): [2k + 2, sig_size - 2k - 1]
                    sheets.push(SheetSpec {
                        slots: vec![slot(base + 2 * k + 2), slot(base + sig_size - 2 * k - 1)],
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
                    creep_px: 0.0,
                    is_back: false,
                });
                // Inside: [2, 3, 4]
                sheets.push(SheetSpec {
                    slots: vec![slot(base + 2), slot(base + 3), slot(base + 4)],
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
                    creep_px: 0.0,
                    is_back: false,
                });
                // Back: [2, 3, 6]
                sheets.push(SheetSpec {
                    slots: vec![slot(base + 2), slot(base + 3), slot(base + 6)],
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
                    creep_px: 0.0,
                    is_back: false,
                });
            }
            sheets
        }
    }
}

/// Compose one physical sheet by placing each panel slot side-by-side left-to-right,
/// preserving outer bleed while meeting flush at internal fold/spine edges.
fn compose_sheet(
    scenes: &[Scene],
    spec: &SheetSpec,
    (bleed_x, bleed_y, trim_w, trim_h): (f64, f64, f64, f64),
    opts: ImposeOptions,
) -> Scene {
    let cols = spec.slots.len();
    let sheet_trim_w = trim_w * cols as f64;
    let sheet_trim_h = trim_h;
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

    for (col, page_slot) in spec.slots.iter().enumerate() {
        let Some(page_idx) = *page_slot else {
            continue;
        };
        let Some(page) = scenes.get(page_idx) else {
            continue;
        };
        let panel_trim_x = margin_x + trim_w * col as f64;
        // Outer sheet edges preserve bleed; inner fold/spine edges clip flush at trim.
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
        let clip_y = margin_y - bleed_y;
        let clip_h = sheet_trim_h + 2.0 * bleed_y;

        // Saddle-stitch creep shifts left panel right (+creep) and right panel left (-creep).
        let creep_shift = if spec.creep_px > 0.0 && cols == 2 {
            if col == 0 {
                spec.creep_px
            } else {
                -spec.creep_px
            }
        } else {
            0.0
        };
        let tx = (panel_trim_x - bleed_x) + creep_shift;
        let ty = margin_y - bleed_y;

        // Strip per-page trailing crop marks (8 StrokeLines emitted by scene compiler when bleed > 0)
        // so only clean sheet-level marks appear when `--marks` is enabled.
        let panel_cmds = strip_panel_crop_marks(page);

        sheet.commands.push(SceneCommand::PushClip {
            x: clip_x,
            y: clip_y,
            w: clip_right - clip_x,
            h: clip_h,
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
        emit_sheet_marks(
            &mut sheet.commands,
            (margin_x, margin_y, sheet_trim_w, sheet_trim_h),
            cols,
            trim_w,
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
        && page.commands.len() >= 8
        && page.commands[page.commands.len() - 8..]
            .iter()
            .all(|cmd| matches!(cmd, SceneCommand::StrokeLine { .. }))
    {
        &page.commands[..page.commands.len() - 8]
    } else {
        &page.commands
    }
}

/// Emit K-only corner crop marks at the 4 sheet trim corners and dashed fold
/// ticks in the top/bottom outer margins at every internal fold crease.
fn emit_sheet_marks(
    commands: &mut Vec<SceneCommand>,
    (left, top, trim_w, trim_h): (f64, f64, f64, f64),
    cols: usize,
    panel_trim_w: f64,
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

    // Internal fold-crease tick marks (dashed, strictly outside trim).
    for col in 1..cols {
        let fx = left + panel_trim_w * col as f64;
        line(fx, top - o - l, fx, top - o, Some(3.0));
        line(fx, bottom + o, fx, bottom + o + l, Some(3.0));
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
}
