//! Argument types for `zenith tx` and `zenith outline-text`.

use clap::Args;
use std::path::PathBuf;

/// Arguments for `zenith tx`.
#[derive(Debug, Args)]
#[command(after_help = "TRANSACTION FILE FORMAT:\n  \
A tx file is a JSON object with a single \"ops\" array; ops are applied in order:\n\n  \
    {\"ops\":[\n      \
{\"op\":\"set_text_align\",\"node\":\"text.hello\",\"align\":\"center\"},\n      \
{\"op\":\"set_fill\",\"node\":\"hero\",\"fill\":\"color.brand\"}\n    \
]}\n\n\
DISCOVERING OPS:\n  \
zenith schema op set_fill          # fields, types, and a working example\n  \
zenith schema op add_node          # how to insert a new node from .zen source\n  \
zenith schema ops                  # list every available op with a summary\n  \
See examples/*.tx.json for runnable samples.\n\n\
EXAMPLES:\n  \
zenith tx poster.zen edits.json                 # preview the diff and moved boxes (dry-run)\n  \
zenith tx poster.zen edits.json --apply         # write the change to disk\n  \
zenith tx poster.zen edits.json --apply --diff  # write, and print the diff and moved boxes")]
pub struct TxArgs {
    /// Path to the `.zen` document.
    pub path: PathBuf,

    /// Path to the transaction JSON file.
    pub tx_file: PathBuf,

    /// Apply the result back to disk (dry-run by default).
    #[arg(long)]
    pub apply: bool,

    /// Print the source diff and moved/resized boxes with `--apply` (a dry-run always prints them).
    #[arg(long)]
    pub diff: bool,

    /// Emit machine-readable JSON instead of a human-readable summary.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `zenith outline-text`.
#[derive(Debug, Args)]
#[command(
    after_help = "EXAMPLES:\n  zenith outline-text poster.zen headline --id-prefix headline.outline\n  zenith outline-text poster.zen headline --id-prefix headline.outline --apply"
)]
pub struct OutlineTextArgs {
    /// Path to the `.zen` document.
    pub path: PathBuf,

    /// Text or code node id to materialize.
    pub node: String,

    /// Prefix for generated path ids.
    #[arg(long)]
    pub id_prefix: String,

    /// Verify project font asset hashes while building the font provider.
    #[arg(long)]
    pub locked: bool,

    /// Apply the result back to disk (dry-run by default).
    #[arg(long)]
    pub apply: bool,

    /// Print the source diff and moved/resized boxes with `--apply` (a dry-run always prints them).
    #[arg(long)]
    pub diff: bool,

    /// Emit machine-readable JSON instead of a human-readable summary.
    #[arg(long)]
    pub json: bool,
}
