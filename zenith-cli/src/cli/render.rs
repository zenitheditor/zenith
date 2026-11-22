//! Argument types for `zenith render`.

use clap::{Args, ValueEnum};
use std::path::PathBuf;

/// Color space override for `--pdf` output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PdfColorspaceArg {
    /// Emit DeviceRGB for screen viewing and digital sharing.
    Srgb,
    /// Emit DeviceCMYK for commercial print production.
    Cmyk,
}

/// Sheet imposition layout for `--pdf` output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ImposeArg {
    /// 2-up saddle-stitch / bi-fold (`[4|1]`, `[2|3]`, …; pads to multiple of 4).
    #[value(alias = "bifold", alias = "saddle")]
    Booklet,
    /// 3-up roll-fold / letter-fold (`[5|6|1]`, `[2|3|4]`; pads to multiple of 6).
    #[value(alias = "letterfold")]
    Trifold,
    /// 3-up Z-fold / accordion (`[5|4|1]`, `[2|3|6]`; pads to multiple of 6).
    #[value(alias = "accordion")]
    Zfold,
    /// 2-up side-by-side spreads (`[1|2]`, `[3|4]`, …; pads to multiple of 2).
    #[value(alias = "2up")]
    Spread,
    /// Sequential N-up gang-up grid (`--grid CxR`, default `2x2`).
    #[value(alias = "nup")]
    Grid,
    /// Stack-ordered N-up cut-and-stack (`--grid CxR`, default `2x2`) so guillotine cuts stay in order.
    #[value(alias = "stack")]
    CutAndStack,
}

/// Duplex flip orientation for back sheets when `--impose` is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DuplexArg {
    /// Standard short-edge flip (landscape booklets): both sides upright.
    ShortEdge,
    /// Long-edge flip: back sheets rotate 180° around sheet center.
    LongEdge,
}

/// Shared press-sheet imposition CLI flags used by both `zenith render` and
/// `zenith merge --combine`.
#[derive(Debug, Args)]
pub struct ImposeCliArgs {
    /// Arrange reading-order pages onto press sheets for PDF output:
    /// `booklet` (`bifold`), `trifold`, `zfold`, `spread` (`2up`), `grid` (`nup`),
    /// or `cut-and-stack` (`stack`).
    #[arg(long, value_enum, value_name = "MODE")]
    pub impose: Option<ImposeArg>,

    /// Grid dimensions `<cols>x<rows>` (e.g. `2x4`, default `2x2`) for `--impose grid`
    /// or `--impose cut-and-stack`.
    #[arg(long, value_name = "CxR")]
    pub grid: Option<String>,

    /// Duplex flip orientation for imposed back sheets: `short-edge` (default) or `long-edge` (180° back rotation).
    /// Requires `--impose`.
    #[arg(long, value_enum, value_name = "EDGE")]
    pub duplex: Option<DuplexArg>,

    /// Total saddle-stitch creep compensation in millimeters (distributed from 0 on outer sheet to MM on centerfold).
    /// Requires `--impose booklet`.
    #[arg(long, value_name = "MM")]
    pub creep: Option<f64>,

    /// Group pages into multi-signature folded sections of N pages (positive multiple of 4, e.g. 16).
    /// Requires `--impose booklet`.
    #[arg(long, value_name = "N")]
    pub signature: Option<usize>,

    /// Draw sheet-level K-only corner crop marks and fold/cut tick marks in the outer margin.
    /// Requires `--impose`.
    #[arg(long)]
    pub marks: bool,
}

impl ImposeCliArgs {
    /// Whether any imposition-related flag was supplied.
    pub(crate) fn has_any(&self) -> bool {
        self.impose.is_some()
            || self.grid.is_some()
            || self.duplex.is_some()
            || self.creep.is_some()
            || self.signature.is_some()
            || self.marks
    }

    /// Validate and convert these CLI flags into [`zenith_pipeline::render::ImposeOptions`].
    pub(crate) fn to_impose_options(
        &self,
    ) -> Result<Option<zenith_pipeline::render::ImposeOptions>, String> {
        use zenith_pipeline::render::{
            DEFAULT_GRID, DuplexMode, ImposeMode, ImposeOptions, parse_grid_spec,
        };
        let Some(mode_arg) = self.impose else {
            if self.has_any() {
                return Err(
                    "error: --duplex, --creep, --signature, --grid, and --marks require --impose <MODE>"
                        .to_owned(),
                );
            }
            return Ok(None);
        };
        let parse_grid = || match self.grid.as_deref() {
            Some(raw) => parse_grid_spec(raw).map_err(|msg| format!("error: {msg}")),
            None => Ok(DEFAULT_GRID),
        };
        let mode = match mode_arg {
            ImposeArg::Booklet => ImposeMode::Booklet,
            ImposeArg::Trifold => ImposeMode::Trifold,
            ImposeArg::Zfold => ImposeMode::Zfold,
            ImposeArg::Spread => ImposeMode::Spread,
            ImposeArg::Grid => {
                let (cols, rows) = parse_grid()?;
                ImposeMode::Grid { cols, rows }
            }
            ImposeArg::CutAndStack => {
                let (cols, rows) = parse_grid()?;
                ImposeMode::CutAndStack { cols, rows }
            }
        };
        if self.grid.is_some()
            && !matches!(
                mode,
                ImposeMode::Grid { .. } | ImposeMode::CutAndStack { .. }
            )
        {
            return Err(
                "error: --grid requires --impose grid or --impose cut-and-stack".to_owned(),
            );
        }
        let creep_mm = self.creep.unwrap_or(0.0);
        if !creep_mm.is_finite() || creep_mm < 0.0 {
            return Err("error: --creep must be a finite number >= 0".to_owned());
        }
        if creep_mm > 0.0 && mode != ImposeMode::Booklet {
            return Err("error: --creep requires --impose booklet".to_owned());
        }
        if let Some(sig) = self.signature {
            if mode != ImposeMode::Booklet {
                return Err("error: --signature requires --impose booklet".to_owned());
            }
            if sig == 0 || sig % 4 != 0 {
                return Err(
                    "error: --signature must be a positive multiple of 4 (e.g. 8, 16)".to_owned(),
                );
            }
        }
        let duplex = match self.duplex {
            Some(DuplexArg::LongEdge) => DuplexMode::LongEdge,
            Some(DuplexArg::ShortEdge) | None => DuplexMode::ShortEdge,
        };
        Ok(Some(ImposeOptions {
            mode,
            duplex,
            creep_mm,
            signature: self.signature,
            marks: self.marks,
        }))
    }
}

/// Arguments for `zenith render`.
#[derive(Debug, Args)]
#[command(
    after_help = "At least one of --scene, --png, --svg, --pdf, --all-pages, --all-pages-svg, or --contact-sheet is required.\n\n\
--scale F rasterizes PNG outputs at F x page pixels (0 < F <= 4). Each side is\n\
max(1, round(page_px x F)), rounding half away from zero. Effects scale too.\n\
--contact-sheet tiles pages in ceil(sqrt(n)) columns with a 16 px gutter and a\n\
page-number label under each page. Without --scale it fits the sheet to 2048 px wide.\n\n\
EXAMPLES:\n  \
zenith render poster.zen --png out.png\n  \
zenith render poster.zen --svg out.svg                     # page 1, outlined text\n  \
zenith render deck.zen --contact-sheet sheet.png             # all pages, one image\n  \
zenith render deck.zen --png slide.png --page 3 --scale 0.5  # cheap preview\n  \
zenith render book.zen --all-pages pages/ --scale 0.25       # small PNG per page\n  \
zenith render book.zen --pdf share.pdf --colorspace srgb     # RGB PDF for digital sharing\n  \
zenith render book.zen --pdf print.pdf --print --impose booklet # CMYK 300 DPI bi-fold/booklet"
)]
pub struct RenderArgs {
    /// Path to the `.zen` document.
    pub path: PathBuf,

    /// Write the compiled scene display-list JSON to this path.
    #[arg(long, value_name = "OUT")]
    pub scene: Option<PathBuf>,

    /// Write the rendered PNG to this path.
    #[arg(long, value_name = "OUT")]
    pub png: Option<PathBuf>,

    /// Write a self-contained RGB SVG with outlined text (default page 1).
    #[arg(long, value_name = "OUT")]
    pub svg: Option<PathBuf>,

    /// Write a vector PDF (with print boxes + DeviceRGB/DeviceCMYK) to this path.
    #[arg(long, value_name = "OUT")]
    pub pdf: Option<PathBuf>,

    /// Override PDF output color space (`srgb` for digital sharing, `cmyk` for print).
    ///
    /// When omitted, `--pdf` follows the document's `colorspace` attribute (default: `srgb`).
    /// Requires `--pdf <OUT>`.
    #[arg(long, value_enum, value_name = "SPACE")]
    pub colorspace: Option<PdfColorspaceArg>,

    /// Print-ready PDF preset: emits DeviceCMYK and sets 300 DPI raster fallback
    /// (`--raster-scale 3.125`, unless `--raster-scale` is explicitly given).
    /// Requires `--pdf <OUT>`.
    #[arg(long)]
    pub print: bool,

    /// Embed a 4-channel CMYK ICC profile file as `/DestOutputProfile` in the
    /// PDF/X `/OutputIntents` dictionary. Implies CMYK output; requires `--pdf <OUT>`.
    #[arg(long, value_name = "PATH")]
    pub icc_profile: Option<PathBuf>,

    /// Press-sheet imposition options for multi-page `--pdf` output (cannot be combined with `--page`).
    #[command(flatten)]
    pub impose: ImposeCliArgs,

    /// Embed whole font programs in the PDF instead of subsetting to used glyphs.
    ///
    /// Native PDF text remains selectable and searchable with either mode.
    /// Rasterized text loses selection and search (default: subset fonts).
    #[arg(long)]
    pub embed_full_fonts: bool,

    /// 1-based page number to render; for `--pdf`, the default renders all pages.
    ///
    /// Without `--page`, single-output flags (`--scene`/`--png`/`--svg`) render page 1,
    /// while `--pdf` renders every page into one multi-page PDF. Passing
    /// `--page N` selects exactly that page for all outputs.
    #[arg(long, value_name = "N")]
    pub page: Option<usize>,

    /// Render every page to `<DIR>/page-<N>.png` (1-based) instead of a single page.
    #[arg(long, value_name = "DIR")]
    pub all_pages: Option<PathBuf>,

    /// Write every page to `<DIR>/page-<N>.svg` in document order.
    #[arg(long, value_name = "DIR")]
    pub all_pages_svg: Option<PathBuf>,

    /// Write one PNG with every page (or the `--page` page) tiled in a grid.
    ///
    /// Columns = ceil(sqrt(pages)). Each cell is the largest scaled page, with
    /// a 16 px gutter and the page number under each page. Without `--scale`
    /// the sheet fits 2048 px wide (never above scale 1).
    #[arg(long, value_name = "OUT")]
    pub contact_sheet: Option<PathBuf>,

    /// Raster scale for PNG outputs: 0 < F <= 4 (default 1).
    ///
    /// Applies to `--png`, `--all-pages`, `--spread`, and `--contact-sheet`.
    /// Each side is `max(1, round(page_px × F))`. The page is drawn at this
    /// scale (no resample), and blur, shadow, mask feather, and stroke widths
    /// scale with it. Use 0.25–0.5 for cheap previews.
    #[arg(long, value_name = "F", allow_hyphen_values = true)]
    pub scale: Option<String>,

    /// Raster fallback resolution for PDF and SVG: 0 < F <= 4 (default 1).
    #[arg(long, value_name = "F", allow_hyphen_values = true)]
    pub raster_scale: Option<String>,

    /// Render two facing pages side by side as a single PNG, e.g. `--spread 10-11`
    /// (1-based page numbers; A on the left, B on the right). Requires `--png`.
    #[arg(long, value_name = "A-B")]
    pub spread: Option<String>,

    /// Override the spread gutter in pixels (default: the document's spread-gutter, or 0).
    /// Only used when `--spread` is set.
    #[arg(long, value_name = "PX")]
    pub gutter: Option<u32>,

    /// Overlay non-printing page construction guides on the rendered output.
    #[arg(long)]
    pub construction_overlay: bool,

    /// Verify each image asset's bytes against its declared `sha256` and fail on mismatch.
    #[arg(long)]
    pub locked: bool,

    /// Emit machine-readable JSON (diagnostics + output path) to stdout.
    #[arg(long)]
    pub json: bool,

    /// Suppress a diagnostic code (downgrade Warning/Advisory to nothing).
    ///
    /// Repeatable. Overrides the document's in-file `diagnostics` block and any
    /// global/local config policy for this code.
    #[arg(long = "allow", value_name = "CODE", action = clap::ArgAction::Append)]
    pub allow: Vec<String>,

    /// Force a diagnostic code to Warning severity.
    ///
    /// Repeatable. Overrides the document's in-file `diagnostics` block and any
    /// global/local config policy for this code.
    #[arg(long = "warn", value_name = "CODE", action = clap::ArgAction::Append)]
    pub warn: Vec<String>,

    /// Elevate a diagnostic code to a blocking Error (CI gate).
    ///
    /// Repeatable. Overrides the document's in-file `diagnostics` block and any
    /// global/local config policy for this code.
    #[arg(long = "deny", value_name = "CODE", action = clap::ArgAction::Append)]
    pub deny: Vec<String>,

    /// Path to a JSON object/array or CSV file supplying values for
    /// `(data)"field.path"` references. JSON nested keys flatten to dot-paths
    /// (`{"a":{"b":1}}` → `"a.b"`); a JSON array uses the first element. CSV
    /// header row gives field names; the first data row supplies values.
    /// Produces a SINGLE render bound to the first object/row; for BATCH output
    /// (PNG or SVG pages per CSV row with a provenance manifest) use `zenith merge` instead.
    #[arg(long, value_name = "FILE")]
    pub data: Option<PathBuf>,
}
