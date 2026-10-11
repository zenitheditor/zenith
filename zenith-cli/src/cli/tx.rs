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
COMMON EDITS (pass the ops with --ops '[...]'; a node needs the op that matches its property):\n  \
change text         {\"op\":\"set_span_text\",\"node\":\"title\",\"span\":0,\"text\":\"Hello\"}\n  \
change a fill       {\"op\":\"set_fill\",\"node\":\"hero\",\"fill\":\"color.brand\"}  (rect, ellipse, frame, text)\n  \
change a token      {\"op\":\"update_token_value\",\"id\":\"color.brand\",\"value\":\"#e11d48\"}  (recolors every user)\n  \
move or resize      {\"op\":\"set_geometry\",\"node\":\"r\",\"x\":10,\"w\":200}  (not for a child in a row/column)\n  \
offset one child    {\"op\":\"set_layout\",\"node\":\"button\",\"margin_top\":40}  (child of a row/column frame)\n  \
add a node          {\"op\":\"add_node\",\"parent\":\"page.main\",\"source\":\"rect id=\\\"box\\\" x=(px)10 y=(px)10 w=(px)100 h=(px)80\"}\n  \
remove a node       {\"op\":\"remove_node\",\"node\":\"old-rect\"}\n  \
show or hide        {\"op\":\"set_visible\",\"node\":\"caption\",\"visible\":false}\n\n\
DISCOVERING OPS:\n  \
zenith schema op set_fill          # fields, types, and a working example\n  \
zenith schema op add_node          # how to insert a new node from .zen source\n  \
zenith schema ops                  # list every available op with a summary\n  \
See examples/*.tx.json for runnable samples.\n\n\
EXAMPLES:\n  \
zenith tx poster.zen edits.json                 # preview the diff and moved boxes (dry-run)\n  \
zenith tx poster.zen edits.json --apply         # write the change to disk\n  \
zenith tx poster.zen edits.json --apply --diff  # write, and print the diff and moved boxes\n  \
zenith tx poster.zen --apply --ops '[{\"op\":\"set_fill\",\"node\":\"hero\",\"fill\":\"color.brand\"}]'  # inline ops, no file")]
pub struct TxArgs {
    /// Path to the `.zen` document.
    pub path: PathBuf,

    /// Path to the transaction JSON file.
    #[arg(required_unless_present = "ops", conflicts_with = "ops")]
    pub tx_file: Option<PathBuf>,

    /// The transaction as inline JSON: a `{"ops":[...]}` object or a bare array of ops.
    #[arg(long, alias = "op")]
    pub ops: Option<String>,

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
