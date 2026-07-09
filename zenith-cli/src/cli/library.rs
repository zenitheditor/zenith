//! Argument types for `zenith library` and its subcommands.

use clap::{Args, Subcommand};
use std::path::PathBuf;

/// Arguments for `zenith library`.
///
/// The library subsystem is a set of reusable **packs** — collections of design
/// assets that you materialize into a `.zen` document.  A pack is identified by
/// a package id such as `@zenith/filters`, and each pack exports one or more
/// named **items** addressed as `<package>#<item>` (e.g.
/// `@zenith/flowchart#decision`).
///
/// Three item kinds exist:
///
/// * **token** — a filter or mask token (e.g. `@zenith/filters#sepia`).  Added
///   to the document's `tokens` block; apply with `filter=(token)"sepia"` or
///   `mask=(token)"vignette"` on any node.
/// * **component** — a reusable node group (e.g. a flowchart shape) that is
///   materialized as an `instance` on a named page.  Requires `--page <id>`.
/// * **action** — a canned transaction op sequence (e.g.
///   `@zenith/brand-kit#apply-2026`) that mutates the target document's tokens
///   or layout.  No page required.
///
/// A pack comes in one of two FORMATS:
///
/// * **`.zen` file** — the feature-rich format: tokens, components, and actions,
///   authored directly. Identified by a `library` self-entry in its own
///   `libraries` block.
/// * **directory of `*.svg`** — the plug-and-install format: an icon set with
///   nothing to author. Each `*.svg` is one icon component, its id is the file
///   stem, and its geometry is converted to native `path` nodes on demand. An
///   optional `library.kdl` beside the icons declares `id`, `version`,
///   `license`, and per-icon `aliases`, `tags`, and `categories`; without one,
///   the pack id defaults to `@local/<dirname>`.
///
/// Embedded `@zenith/*` packs are bundled in the binary — including
/// `@zenith/icons-lucide`, the full Lucide icon set. Project-local packs live in
/// `<project-dir>/libraries/` (a `*.zen` file, or a subdirectory of `*.svg`) and
/// shadow embedded packs of the same id.
///
/// WORKFLOW:
///   zenith library list                          # discover packs + items
///   zenith library search device                 # ranked search over names/aliases/tags
///   zenith library show @zenith/filters#sepia    # inspect one item
///   zenith library add @zenith/filters#sepia --into poster.zen
#[derive(Debug, Args)]
#[command(
    long_about = "Manage reusable library packs (embedded @zenith/* presets + project-local packs).\n\n\
A pack exports items addressed as <package>#<item>, e.g. `@zenith/flowchart#decision`.\n\
Item kinds:\n  \
token     — filter or mask token; copy into tokens block, apply with filter=(token)\"id\"\n  \
component — reusable node group; materialized as an instance on a page (requires --page)\n  \
action    — canned tx op sequence; runs a transaction against the target document\n\n\
Pack formats:\n  \
.zen file          — tokens, components, actions; declares its own `library` self-entry\n  \
directory of *.svg — an icon set; one icon per file, id = file stem, converted to native\n                       \
paths on demand. Optional `library.kdl` adds id/version/license and per-icon\n                       \
aliases/tags/categories. Without it the id is @local/<dirname>.\n\n\
Embedded @zenith/* packs are built in (including @zenith/icons-lucide, the full Lucide set).\n\
Project packs live in libraries/ — a *.zen file, or a subdirectory of *.svg — and shadow them.\n\n\
WORKFLOW:\n  \
zenith library list                          # discover packs and items\n  \
zenith library search device                 # ranked search over names, aliases, and tags\n  \
zenith library show @zenith/filters#sepia    # inspect item content before adding\n  \
zenith library add @zenith/filters#sepia --into poster.zen"
)]
pub struct LibraryArgs {
    #[command(subcommand)]
    pub command: LibrarySub,
}

/// Subcommands of `zenith library`.
#[derive(Debug, Subcommand)]
pub enum LibrarySub {
    /// List all resolved library packs (project + embedded presets) and items.
    ///
    /// Lists every available pack and its exported items.  Run `zenith library
    /// show <package>#<item>` to inspect any item in detail before adding it.
    /// Human output lists at most the first few items per pack — an icon library
    /// exports ~1745 — so use `zenith library search` to find one; `--json`
    /// carries every item.
    /// A pack's header line shows `(tokens: N)` when it carries a token set
    /// beyond its exported items; merge that whole set into a document with
    /// `zenith theme apply <pack-id> <doc>`.
    List(LibraryListArgs),

    /// Inspect a library item in detail before adding it.
    ///
    /// Shows the package, item id, and kind-specific content: filter/mask token
    /// types and ops, component node structure, or action op sequence.  Prints
    /// the exact `zenith library add` invocation to materialize the item.
    Show(LibraryShowArgs),

    /// Ranked search over item id, aliases, tags, package id, kind, and license.
    ///
    /// Searches resolved project and embedded packs. Results are ranked, best
    /// first: an item NAMED for the query beats one merely aliased to it, which
    /// beats one merely tagged with it. Every query term must match. Narrow with
    /// `--category`, `--kind`, and `--pack`; `--limit 0` lifts the result cap.
    Search(LibrarySearchArgs),

    /// Materialize a library item into a target `.zen` document.
    ///
    /// Adds ONE named item (a component, filter/mask token, or action). To merge
    /// a pack's WHOLE token set into a document instead, use `zenith theme apply
    /// <pack-id> <doc>` — it works with any pack id that carries tokens, not
    /// just `@zenith/theme.*` presets.
    Add(LibraryAddArgs),
}

/// Arguments for `zenith library add`.
#[derive(Debug, Args)]
pub struct LibraryAddArgs {
    /// The item to add, as `<package>#<item>`, e.g. `@zenith/flowchart#decision`.
    pub spec: String,

    /// Target `.zen` document to materialize the item into (written in-place,
    /// unless `--dry-run`). Its parent directory is the project dir whose
    /// `libraries/` packs are resolved alongside the embedded presets.
    #[arg(long, value_name = "FILE")]
    pub into: PathBuf,

    /// Id of the page in the target document to place the instance on.
    ///
    /// Required only for COMPONENT items; TOKEN items (filter tokens) ignore it.
    #[arg(long, value_name = "ID")]
    pub page: Option<String>,

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
    /// Project `libraries/` packs — `*.zen` files and `<name>/` SVG icon
    /// directories — are scanned alongside embedded presets.
    /// Defaults to the current working directory.
    pub path: Option<PathBuf>,

    /// Emit machine-readable JSON instead of a human-readable listing.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `zenith library show`.
#[derive(Debug, Args)]
#[command(after_help = "EXAMPLES:\n  \
zenith library show @zenith/filters#sepia       # inspect a filter token\n  \
zenith library show @zenith/flowchart#decision  # inspect a component\n  \
zenith library show @zenith/brand-kit#apply-2026 --json")]
pub struct LibraryShowArgs {
    /// The item to inspect, as `<package>#<item>`, e.g. `@zenith/filters#sepia`.
    pub spec: String,

    /// Project directory, or a `.zen` file whose parent is the project directory.
    /// Project `libraries/` packs — `*.zen` files and `<name>/` SVG icon
    /// directories — are resolved alongside embedded presets.
    /// Defaults to the current working directory.
    pub path: Option<PathBuf>,

    /// Emit machine-readable JSON instead of a human-readable summary.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `zenith library search`.
#[derive(Debug, Args)]
#[command(after_help = "EXAMPLES:\n  \
zenith library search device                     # rank device-like icon components\n  \
zenith library search arrow --category navigation\n  \
zenith library search cloud --json               # machine-readable results with tags\n  \
zenith library search token --kind token --limit 0\n  \
zenith library search noir")]
pub struct LibrarySearchArgs {
    /// Query text, ranked against item id, aliases, tags, package id, kind, and license.
    pub query: String,

    /// Project directory, or a `.zen` file whose parent is the project directory.
    /// Project packs — `libraries/*.zen` files and `libraries/<name>/` SVG icon
    /// directories — are resolved alongside embedded presets.
    /// Defaults to the current working directory.
    pub path: Option<PathBuf>,

    /// Keep only items in this category (e.g. `navigation`, `shapes`).
    /// Categories filter; they are never matched as free text.
    #[arg(long)]
    pub category: Option<String>,

    /// Keep only items of this kind.
    #[arg(long, value_parser = ["component", "token", "action"])]
    pub kind: Option<String>,

    /// Keep only items from this exact package id.
    #[arg(long)]
    pub pack: Option<String>,

    /// Maximum results to show; `0` shows every match.
    #[arg(long, default_value_t = crate::commands::library::DEFAULT_SEARCH_LIMIT)]
    pub limit: usize,

    /// Emit machine-readable JSON instead of a human-readable summary.
    #[arg(long)]
    pub json: bool,
}
