//! Argument types for the batch generators: `zenith merge` and `zenith variant`.

use clap::Args;
use std::path::PathBuf;

/// Arguments for `zenith merge`.
#[derive(Debug, Args)]
#[command(after_help = "EXAMPLES:\n  \
zenith merge card.zen people.csv --out-dir out/ --name-by name --manifest run.json\n  \
zenith merge badge.zen people.csv --out-dir out/ --format pdf --combine out/all.pdf\n\n\
FILES: PNG and SVG write one file per page: `<stem>.<ext>` for a one-page template, \
`<stem>-page-N.<ext>` for more pages. PDF writes ONE `<stem>.pdf` per row with all its pages. \
The PDF keeps the page TrimBox and BleedBox from `page bleed`.\n\n\
A row is atomic: if any page fails, the row writes no file.\n\n\
--combine FILE also writes one PDF of every successful row's pages, in CSV order. \
The per-row PDFs are still written. Failed rows are left out and listed in the report.")]
pub struct MergeArgs {
    /// Template `.zen` document with `role="data.<column>"` text nodes.
    pub doc: PathBuf,

    /// CSV data file; header row names the columns.
    pub data: PathBuf,

    /// Directory for rendered row files.
    #[arg(long, value_name = "DIR")]
    pub out_dir: PathBuf,

    /// CSV column for output names (default: row-NNNN with the chosen format extension).
    #[arg(long, value_name = "COL")]
    pub name_by: Option<String>,

    /// Emit a machine-readable JSON batch report (per-row provenance).
    #[arg(long)]
    pub json: bool,

    /// Write a deterministic generation manifest (JSON) to this path for CI
    /// reproducibility. Independent of --json.
    #[arg(long, value_name = "PATH")]
    pub manifest: Option<PathBuf>,
    /// Output format. PDF writes one file per row.
    #[arg(long, value_enum, default_value = "png")]
    pub format: crate::commands::render::BatchFormat,
    /// Raster fallback resolution for SVG or PDF: 0 < F <= 4 (default 1).
    #[arg(long, value_name = "F", allow_hyphen_values = true)]
    pub raster_scale: Option<String>,
    /// Also write one PDF of every successful row's pages here. Requires --format pdf.
    #[arg(long, value_name = "FILE")]
    pub combine: Option<PathBuf>,
}

/// Arguments for `zenith variant`.
#[derive(Debug, Args)]
#[command(after_help = "EXAMPLE:\n  \
zenith variant poster.zen --out-dir out/ --manifest run.json\n  \
zenith variant poster.zen --out-dir out/ --format pdf\n\n\
The document must contain a `variants { variant id=\"square\" source=\"page.main\" w=(px)1080 \
h=(px)1080 { … } }` block.\n\n\
Each variant writes `<stem>-<id>.zen` and one `<stem>-<id>.<ext>` file. \
A PDF variant is one page with the TrimBox and BleedBox from `page bleed`.")]
pub struct VariantArgs {
    /// Input `.zen` document containing a `variants` block.
    pub doc: PathBuf,

    /// Directory for each generated `.zen` companion and image.
    #[arg(long, value_name = "DIR")]
    pub out_dir: PathBuf,

    /// Emit a machine-readable JSON batch report (per-variant provenance).
    #[arg(long)]
    pub json: bool,

    /// Write a deterministic generation manifest (JSON) to this path for CI
    /// reproducibility.  Independent of --json.
    #[arg(long, value_name = "PATH")]
    pub manifest: Option<PathBuf>,
    /// Output format.
    #[arg(long, value_enum, default_value = "png")]
    pub format: crate::commands::render::BatchFormat,
    /// Raster fallback resolution for SVG or PDF: 0 < F <= 4 (default 1).
    #[arg(long, value_name = "F", allow_hyphen_values = true)]
    pub raster_scale: Option<String>,
}
