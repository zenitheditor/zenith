//! Argument types for `zenith import`.

use std::path::PathBuf;

use clap::{Args, Subcommand};

/// Arguments for `zenith import`.
#[derive(Debug, Args)]
pub struct ImportArgs {
    #[command(subcommand)]
    pub command: ImportSub,
}

/// Subcommands of `zenith import`.
#[derive(Debug, Subcommand)]
pub enum ImportSub {
    /// Convert a vector SVG file into an editable, tokenized `.zen` document.
    Svg(ImportSvgArgs),
}

/// Arguments for `zenith import svg`.
#[derive(Debug, Args)]
#[command(after_help = "EXAMPLES:\n  \
zenith import svg logo.svg --out logo.zen\n  \
zenith import svg icon.svg -o icon.zen --name \"App Icon\" --json\n  \
zenith import svg diagram.svg -o diagram.zen --force")]
pub struct ImportSvgArgs {
    /// Input `.svg` file to convert.
    pub input: PathBuf,

    /// Destination `.zen` file path.
    #[arg(short, long, value_name = "OUT.zen")]
    pub out: PathBuf,

    /// Optional project/document display name (defaults to input file stem).
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,

    /// Overwrite `--out` if it already exists.
    #[arg(long)]
    pub force: bool,

    /// Emit machine-readable JSON instead of a human-readable summary.
    #[arg(long)]
    pub json: bool,
}
