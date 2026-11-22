//! Argument types for `zenith reflow`.

use std::path::PathBuf;

use clap::Args;

/// Arguments for `zenith reflow`.
#[derive(Debug, Args)]
#[command(after_help = "EXAMPLES:\n  \
zenith reflow book.zen              # dry-run: preview continuation pages & diff\n  \
zenith reflow book.zen --apply      # write spawned continuation pages into book.zen\n  \
zenith reflow book.zen --json       # machine-readable pagination report")]
pub struct ReflowArgs {
    /// Path to the `.zen` document with chained text (`chain="..."`).
    pub path: PathBuf,

    /// Write the paginated document back to disk (dry-run by default).
    #[arg(long)]
    pub apply: bool,

    /// Maximum number of new continuation pages to spawn (default: 200, max: 1000).
    #[arg(long, default_value_t = 200, value_name = "N")]
    pub max_pages: usize,

    /// Emit machine-readable JSON instead of a human-readable summary.
    #[arg(long)]
    pub json: bool,
}
