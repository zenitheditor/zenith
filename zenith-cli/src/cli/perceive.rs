//! Argument types for `zenith perceive` and its subcommands.

use clap::{Args, Subcommand};
use std::path::PathBuf;

/// Arguments for `zenith perceive`.
#[derive(Debug, Args)]
#[command(
    after_help = "EXAMPLES:\n  zenith perceive vector logo.zen\n  zenith perceive vector logo.zen --json"
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
    },
}
