//! Argument types for `zenith tx` and `zenith outline-text`.

use clap::Args;
use std::path::PathBuf;

/// The common `tx` edits: one runnable op per line. `zenith tx --help` and
/// `zenith schema write` both print this text, so they cannot disagree. A
/// macro, because `concat!` takes only literals.
macro_rules! common_edits {
    () => {
        "  change text         {\"op\":\"set_span_text\",\"node\":\"title\",\"span\":0,\"text\":\"Hello\"}\n  \
change a fill       {\"op\":\"set_fill\",\"node\":\"hero\",\"fill\":\"color.brand\"}  (rect, ellipse, frame, text)\n  \
change a token      {\"op\":\"update_token_value\",\"id\":\"color.brand\",\"value\":\"#e11d48\"}  (recolors every user; a size is \"(px)4\")\n  \
add a token         {\"op\":\"create_token\",\"id\":\"color.ok\",\"type\":\"color\",\"value\":\"#16a34a\"}  (then set_fill to use it)\n  \
change a style      {\"op\":\"set_style_property\",\"style_id\":\"heading\",\"property\":\"fill\",\"value\":\"color.ok\"}  (every node with that style)\n  \
move or resize      {\"op\":\"set_geometry\",\"node\":\"r\",\"x\":10,\"w\":200}  (not for a child in a row/column)\n  \
offset one child    {\"op\":\"set_layout\",\"node\":\"button\",\"margin_top\":40}  (child of a row/column frame)\n  \
add a node          {\"op\":\"add_node\",\"parent\":\"page.main\",\"source\":\"rect id=\\\"box\\\" x=(px)10 y=(px)10 w=(px)100 h=(px)80\"}\n  \
remove a node       {\"op\":\"remove_node\",\"node\":\"old-rect\"}\n  \
repeat a row or card {\"op\":\"duplicate_node\",\"node\":\"topics.1\",\"new_id\":\"topics.1-b\"}  (then change the copy's text)\n  \
repeat a page       {\"op\":\"duplicate_page\",\"page\":\"page.1\",\"new_id\":\"page.2\",\"id_suffix\":\".2\"}  (child ids get the suffix: title.2)\n  \
move a page         {\"op\":\"move_page\",\"page\":\"page.5\",\"position\":{\"at\":\"after\",\"id\":\"page.2\"}}  (swap_pages swaps two)\n  \
change one instance {\"op\":\"set_override\",\"instance\":\"card.2\",\"ref\":\"card.title\",\"text\":\"Hello\"}  (also fill, visible)\n  \
show or hide        {\"op\":\"set_visible\",\"node\":\"caption\",\"visible\":false}"
    };
}

/// The common edits as text, for `zenith schema write`.
pub const COMMON_EDITS: &str = common_edits!();

/// Arguments for `zenith tx`.
#[derive(Debug, Args)]
#[command(after_help = concat!("COMMON EDITS (a node needs the op that matches its property):\n", common_edits!(), "\n\n\
A tx file holds {\"ops\":[...]}. Ops apply in order. `--ops` takes the same JSON or a bare array.\n\
Run `zenith inspect <PATH>` for ids, styles, and overrides. Run `zenith schema op <name>` for one op's fields and example.\n\
Run `zenith schema ops` to list every op.\n\n\
EXAMPLES:\n  \
zenith tx deck.zen --ops '[{\"op\":\"set_fill\",\"node\":\"hero\",\"fill\":\"color.brand\"}]'  # dry run: diff and boxes\n  \
zenith tx deck.zen edits.json --apply --png p3.png --page 3  # write, and render page 3"))]
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

    /// With `--apply`, render the edited page to this PNG in the same call. A rejected edit writes neither file.
    #[arg(long, value_name = "OUT", requires = "apply")]
    pub png: Option<PathBuf>,

    /// The 1-based page `--png` renders (default 1).
    #[arg(
        long,
        value_name = "N",
        requires = "png",
        value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..)
    )]
    pub page: Option<usize>,

    /// With `--apply`, print the source diff and moved or resized boxes. A dry run always prints them.
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

    /// With `--apply`, print the source diff and moved or resized boxes. A dry run always prints them.
    #[arg(long)]
    pub diff: bool,

    /// Emit machine-readable JSON instead of a human-readable summary.
    #[arg(long)]
    pub json: bool,
}
