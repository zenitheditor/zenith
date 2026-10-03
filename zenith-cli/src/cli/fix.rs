//! Argument types for `zenith fix`.

use clap::Args;
use std::path::PathBuf;

/// Arguments for `zenith fix`.
#[derive(Debug, Args)]
#[command(after_help = "FIXES:\n  \
raw visual literal      → the token with the same value, else a minted token\n                          \
(color.custom.<hex>, size.<n>, radius.<n>, stroke.<n>, space.<n>,\n                          \
weight.<n>, font.<slug>) placed in the tokens block\n  \
unknown property        → its did-you-mean, when unique and not already set\n  \
unknown token reference → a declared prefix (color.primary.500 → color.primary)\n                          \
or a unique id within 2 edits of the same type\n  \
invalid enum value      → its did-you-mean, when unique\n\
Everything else stays in `remaining`. The result is canonical (`zenith fmt`).\n\n\
EXAMPLES:\n  \
zenith fix draft.zen            # preview the fixes and the source diff (dry-run)\n  \
zenith fix draft.zen --apply    # write the fixed source (recorded in history)\n  \
zenith fix draft.zen --json     # zenith-fix-v1: applied + remaining diagnostics")]
pub struct FixArgs {
    /// Path to the `.zen` document.
    pub path: PathBuf,

    /// Write the fixed source back to disk (dry-run by default).
    #[arg(long)]
    pub apply: bool,

    /// Emit machine-readable JSON (`zenith-fix-v1`) instead of a summary and diff.
    #[arg(long)]
    pub json: bool,
}
