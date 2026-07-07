use std::path::PathBuf;

use clap::{Args, Subcommand};

/// Arguments for `zenith asset`.
#[derive(Debug, Args)]
pub struct AssetArgs {
    #[command(subcommand)]
    pub command: AssetSub,
}

/// Subcommands of `zenith asset`.
#[derive(Debug, Subcommand)]
pub enum AssetSub {
    /// Import a local file as a frozen asset declaration.
    Import(AssetImportArgs),
    /// Bake a ZPX manifest into a frozen PNG asset declaration.
    #[command(name = "zpx-bake")]
    ZpxBake(AssetZpxBakeArgs),
}

/// Arguments for `zenith asset import`.
#[derive(Debug, Args)]
#[command(after_help = "EXAMPLES:\n  \
zenith asset import logo.svg --into poster.zen --id asset.logo --src assets/logo.svg --kind svg\n  \
zenith asset import photo.png --into poster.zen --id asset.hero --src assets/hero.png --kind image --apply")]
pub struct AssetImportArgs {
    /// Source file to import.
    pub input: PathBuf,

    /// Document that will receive the asset declaration.
    #[arg(long, value_name = "DOC.zen")]
    pub into: PathBuf,

    /// Asset id to add to the document.
    #[arg(long, value_name = "ASSET_ID")]
    pub id: String,

    /// Relative asset path to declare and write under the document directory.
    #[arg(long, value_name = "RELATIVE_ASSET_PATH")]
    pub src: String,

    /// Asset kind.
    #[arg(long, value_name = "image|svg|font")]
    pub kind: String,

    /// Apply the import to disk (dry-run by default).
    #[arg(long)]
    pub apply: bool,

    /// Emit machine-readable JSON instead of a human-readable summary.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `zenith asset zpx-bake`.
#[derive(Debug, Args)]
#[command(after_help = "EXAMPLES:\n  \
zenith asset zpx-bake painting.zpx --into poster.zen --id asset.paint --src assets/paint.png\n  \
zenith asset zpx-bake painting.zpx --into poster.zen --id asset.paint --src assets/paint.png --apply")]
pub struct AssetZpxBakeArgs {
    /// ZPX manifest to bake.
    #[arg(value_name = "ZPX_MANIFEST")]
    pub manifest: PathBuf,

    /// Document that will receive the asset declaration.
    #[arg(long, value_name = "DOC.zen")]
    pub into: PathBuf,

    /// Asset id to add to the document.
    #[arg(long, value_name = "ASSET_ID")]
    pub id: String,

    /// Relative PNG asset path to declare and write under the document directory.
    #[arg(long, value_name = "RELATIVE_PNG_PATH")]
    pub src: String,

    /// Apply the bake to disk (dry-run by default).
    #[arg(long)]
    pub apply: bool,

    /// Emit machine-readable JSON instead of a human-readable summary.
    #[arg(long)]
    pub json: bool,
}
