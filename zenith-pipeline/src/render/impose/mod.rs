//! Print sheet imposition: arrange reading-order pages onto press-ready sheets
//! (booklet / bi-fold, tri-fold, Z-fold, 2-up spread, N-up grid, cut-and-stack)
//! with outer bleed preservation, crop/fold/cut marks, CMYK color control bars,
//! registration targets, duplex rotation, creep compensation, and multi-signature
//! grouping.

mod compose;
mod marks;
mod plan;

#[cfg(test)]
mod tests;

use zenith_scene::Scene;

use crate::error::PipelineError;

pub use plan::{DEFAULT_GRID, parse_grid_spec};

/// How reading-order pages are arranged onto physical press sheets for PDF output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImposeMode {
    /// 1-up single-page press sheet: preserves each page 1-to-1 with outer bleed
    /// and optional slug margin for sheet-level `--marks` and `--color-bars`.
    Single,
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
    /// Sheet arrangement mode (`booklet`, `trifold`, `zfold`, `spread`, `grid`, `cut-and-stack`).
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
    /// Draw sheet-level K-only corner crop marks and fold/cut tick marks in
    /// the outer margin.
    pub marks: bool,
    /// Draw CMYK density control bar and 4-plate registration targets in the
    /// outer slug margin.
    pub color_bars: bool,
}

impl ImposeOptions {
    /// Default imposition options for `mode`: short-edge duplex, no creep,
    /// single signature, no crop/fold marks or color bars.
    #[must_use]
    pub fn new(mode: ImposeMode) -> Self {
        Self {
            mode,
            duplex: DuplexMode::ShortEdge,
            creep_mm: 0.0,
            signature: None,
            marks: false,
            color_bars: false,
        }
    }
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
/// `render.impose_invalid` if `creep_mm`, `signature`, or grid dimensions are invalid.
pub fn impose_scenes(scenes: &[Scene], opts: ImposeOptions) -> Result<Vec<Scene>, PipelineError> {
    let geom = plan::validate_and_measure(scenes, opts)?;
    let sheets = plan::sheet_plan(scenes.len(), opts);
    Ok(sheets
        .iter()
        .map(|spec| {
            let sheet_geom = if opts.mode == ImposeMode::Single {
                spec.slots
                    .first()
                    .and_then(|s| *s)
                    .and_then(|idx| scenes.get(idx))
                    .map_or(geom, plan::panel_geometry)
            } else {
                geom
            };
            compose::compose_sheet(scenes, spec, sheet_geom, opts)
        })
        .collect())
}
