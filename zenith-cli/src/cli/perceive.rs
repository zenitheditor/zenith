//! Argument types for `zenith perceive` and its subcommands.

use clap::{Args, Subcommand};
use std::path::PathBuf;

/// Arguments for `zenith perceive`.
#[derive(Debug, Args)]
#[command(
    after_help = "EXAMPLES:\n  zenith perceive vector logo.zen\n  zenith perceive vector logo.zen --json\n  zenith perceive page poster.zen --json\n  zenith perceive page deck.zen --all-pages"
)]
pub struct PerceiveArgs {
    #[command(subcommand)]
    pub command: PerceiveSub,

    /// Emit machine-readable JSON instead of human-readable text.
    #[arg(long, global = true)]
    pub json: bool,
}

/// Subcommands of `zenith perceive`.
#[derive(Debug, Subcommand)]
pub enum PerceiveSub {
    /// Run deterministic vector/path perception metrics for every path node.
    Vector {
        /// `.zen` document to analyze.
        path: PathBuf,
        /// Analyze only the named path id. Repeat to analyze a multi-path mark subset.
        #[arg(long = "node")]
        nodes: Vec<String>,
    },
    /// Render pages as `zenith render --png` does, then measure deterministic page signals.
    ///
    /// Signals: luminance histogram, value zones, density map, edge map,
    /// optical balance, clearspace against declared margins, and grid
    /// conformance against declared construction guides. They are signals,
    /// not a judgement of design quality. JSON uses schema 1.
    Page {
        /// `.zen` document to render and measure.
        path: PathBuf,
        /// 1-based page to measure (default 1).
        #[arg(long, conflicts_with = "all_pages")]
        page: Option<usize>,
        /// Measure every page, in page order.
        #[arg(long)]
        all_pages: bool,
    },
}
