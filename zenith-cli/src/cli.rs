//! Clap argument types for the Zenith CLI.
//!
//! This module defines the top-level [`Cli`] struct and the [`Command`]
//! subcommand enum. No business logic lives here — just argument shapes.

use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// Zenith — design-document toolchain.
#[derive(Debug, Parser)]
#[command(name = "zenith", about = "Zenith design-document toolchain")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// Top-level subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Validate a `.zen` document and report diagnostics.
    Validate(ValidateArgs),

    /// Format a `.zen` document in-place (idempotent).
    Fmt(FmtArgs),

    /// List all design tokens and their resolved values.
    Tokens(TokensArgs),

    /// Compile and render a `.zen` document.
    Render(RenderArgs),

    /// Apply a transaction to a `.zen` document (dry-run by default).
    Tx(TxArgs),

    /// Print the node tree of a `.zen` document (read-only).
    Inspect(InspectArgs),

    /// Mail-merge a `.zen` template with a CSV data file, writing one PNG per row.
    Merge(MergeArgs),

    /// Inspect the library subsystem (preset + project packs).
    Library(LibraryArgs),
}

/// Arguments for `zenith library`.
#[derive(Debug, Args)]
pub struct LibraryArgs {
    #[command(subcommand)]
    pub command: LibrarySub,
}

/// Subcommands of `zenith library`.
#[derive(Debug, Subcommand)]
pub enum LibrarySub {
    /// List all resolved library packs (project + embedded presets) and items.
    List(LibraryListArgs),

    /// Materialize a library item into a target `.zen` document.
    Add(LibraryAddArgs),
}

/// Arguments for `zenith library add`.
#[derive(Debug, Args)]
pub struct LibraryAddArgs {
    /// The item to add, as `<package>#<item>`, e.g. `@zenith/flowchart#decision`.
    pub spec: String,

    /// Target `.zen` document to materialize the item into (written in-place,
    /// unless `--dry-run`). Its parent directory is the project dir whose
    /// `libraries/*.zen` packs are resolved alongside the embedded presets.
    #[arg(long, value_name = "FILE")]
    pub into: PathBuf,

    /// Id of the page in the target document to place the instance on.
    #[arg(long, value_name = "ID")]
    pub page: String,

    /// Instance origin as `X,Y` in pixels (default `0,0`).
    #[arg(long, value_name = "X,Y")]
    pub at: Option<String>,

    /// Override the generated instance id base (default: the item name).
    #[arg(long, value_name = "ID")]
    pub id: Option<String>,

    /// Print the resulting source to stdout WITHOUT writing the file.
    #[arg(long)]
    pub dry_run: bool,
}

/// Arguments for `zenith library list`.
#[derive(Debug, Args)]
pub struct LibraryListArgs {
    /// Project directory, or a `.zen` file whose parent is the project directory.
    /// Project `libraries/*.zen` packs are scanned alongside embedded presets.
    /// Defaults to the current working directory.
    pub path: Option<PathBuf>,

    /// Emit machine-readable JSON instead of a human-readable listing.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `zenith validate`.
#[derive(Debug, Args)]
pub struct ValidateArgs {
    /// Path to the `.zen` document.
    pub path: PathBuf,

    /// Emit machine-readable JSON instead of a human-readable table.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `zenith fmt`.
#[derive(Debug, Args)]
pub struct FmtArgs {
    /// Path to the `.zen` document (written in-place).
    pub path: PathBuf,

    /// Emit machine-readable JSON reporting `changed` and `hash`.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `zenith tokens`.
#[derive(Debug, Args)]
pub struct TokensArgs {
    /// Path to the `.zen` document.
    pub path: PathBuf,

    /// Emit machine-readable JSON instead of a human-readable table.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `zenith tx`.
#[derive(Debug, Args)]
pub struct TxArgs {
    /// Path to the `.zen` document.
    pub path: PathBuf,

    /// Path to the transaction JSON file.
    pub tx_file: PathBuf,

    /// Apply the result back to disk (dry-run by default).
    #[arg(long)]
    pub apply: bool,

    /// Emit machine-readable JSON instead of a human-readable summary.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `zenith inspect`.
#[derive(Debug, Args)]
pub struct InspectArgs {
    /// Path to the `.zen` document.
    pub path: PathBuf,

    /// Inspect only the subtree rooted at this node id.
    #[arg(long, value_name = "ID")]
    pub node: Option<String>,

    /// Emit machine-readable JSON instead of a human-readable tree.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `zenith merge`.
#[derive(Debug, Args)]
pub struct MergeArgs {
    /// Template `.zen` document with role="data.<column>" text nodes.
    pub doc: PathBuf,

    /// CSV data file; header row names the columns.
    pub data: PathBuf,

    /// Directory to write one PNG per row into.
    #[arg(long, value_name = "DIR")]
    pub out_dir: PathBuf,

    /// CSV column to name each output file by (default: row-NNNN.png).
    #[arg(long, value_name = "COL")]
    pub name_by: Option<String>,
}

/// Arguments for `zenith render`.
#[derive(Debug, Args)]
#[command(after_help = "At least one of --scene or --png is required.")]
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

    /// 1-based page number to render (default: 1).
    #[arg(long, value_name = "N", default_value_t = 1)]
    pub page: usize,

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
}
