//! Sheet planning, grid spec parsing, and imposition option validation.

use zenith_core::PX_PER_INCH;
use zenith_scene::Scene;

use super::{ImposeMode, ImposeOptions};
use crate::error::PipelineError;

/// Millimeters per inch (exact ISO definition).
const MM_PER_INCH: f64 = 25.4;

/// Default N-up grid dimensions (`2×2`, 4-up) when `--grid` is omitted for
/// `grid` or `cut-and-stack` imposition.
pub const DEFAULT_GRID: (usize, usize) = (2, 2);

/// Maximum columns or rows allowed in a single N-up imposition grid.
const MAX_GRID_SIDE: usize = 32;

/// Uniform source-page geometry `(bleed_x, bleed_y, trim_w, trim_h)` in px.
#[derive(Clone, Copy)]
pub(super) struct PanelGeometry {
    pub(super) bleed_x: f64,
    pub(super) bleed_y: f64,
    pub(super) trim_w: f64,
    pub(super) trim_h: f64,
}

/// One planned sheet: panel slots (`None` = blank) in row-major order, grid
/// dimensions (`cols × rows`), creep shift (px) inward toward the spine, and
/// whether this is a back side.
pub(super) struct SheetSpec {
    pub(super) slots: Vec<Option<usize>>,
    pub(super) cols: usize,
    pub(super) rows: usize,
    pub(super) creep_px: f64,
    pub(super) is_back: bool,
}

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

/// Validate `scenes` and `opts`, returning the shared panel geometry.
pub(super) fn validate_and_measure(
    scenes: &[Scene],
    opts: ImposeOptions,
) -> Result<PanelGeometry, PipelineError> {
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

    Ok(PanelGeometry {
        bleed_x,
        bleed_y,
        trim_w,
        trim_h,
    })
}

/// Compute the ordered list of press sheets (slots, grid dims, creep, back-side flag).
pub(super) fn sheet_plan(page_count: usize, opts: ImposeOptions) -> Vec<SheetSpec> {
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
