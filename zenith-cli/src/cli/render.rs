//! Argument types for `zenith render`.

use clap::Args;
use std::path::PathBuf;

/// Arguments for `zenith render`.
#[derive(Debug, Args)]
#[command(
    after_help = "At least one of --scene, --png, --pdf, or --all-pages is required.\n\n\
EXAMPLES:\n  \
zenith render poster.zen --png out.png\n  \
zenith render book.zen --all-pages sheet/      # one PNG per page\n  \
zenith render book.zen --pdf book.pdf          # print-ready vector PDF"
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

    /// Write a vector PDF (with print boxes + DeviceCMYK) to this path.
    #[arg(long, value_name = "OUT")]
    pub pdf: Option<PathBuf>,

    /// Embed whole font programs in the PDF instead of subsetting to used glyphs.
    ///
    /// PDF text is always selectable and searchable; this only trades a larger
    /// file for embedding the complete face (default: subset for small files).
    #[arg(long)]
    pub embed_full_fonts: bool,

    /// 1-based page number to render; for `--pdf`, the default renders all pages.
    ///
    /// Without `--page`, single-output flags (`--scene`/`--png`) render page 1,
    /// while `--pdf` renders every page into one multi-page PDF. Passing
    /// `--page N` selects exactly that page for all outputs.
    #[arg(long, value_name = "N")]
    pub page: Option<usize>,

    /// Render every page to `<DIR>/page-<N>.png` (1-based) instead of a single page.
    #[arg(long, value_name = "DIR")]
    pub all_pages: Option<PathBuf>,

    /// Render two facing pages side by side as a single PNG, e.g. `--spread 10-11`
    /// (1-based page numbers; A on the left, B on the right). Requires `--png`.
    #[arg(long, value_name = "A-B")]
    pub spread: Option<String>,

    /// Override the spread gutter in pixels (default: the document's spread-gutter, or 0).
    /// Only used when `--spread` is set.
    #[arg(long, value_name = "PX")]
    pub gutter: Option<u32>,

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
    /// (one PNG per CSV row with a provenance manifest) use `zenith merge` instead.
    #[arg(long, value_name = "FILE")]
    pub data: Option<PathBuf>,
}
