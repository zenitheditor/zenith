//! Transaction engine: [`run_transaction`] and all per-op application logic.
//!
//! This module is pure: it performs no file I/O and does not mutate the input
//! document (it works on a clone). Dry-run vs. apply is the caller's concern.

mod asset;
mod dispatch;
mod fill_rule;
mod flags;
mod geometry;
mod lock;
mod path;
mod pattern;
mod recipe;
mod run;
pub(crate) mod structure;
mod style;
mod text_outline;
mod token;
mod tree;

pub use run::run_transaction;
pub use text_outline::{
    SCENE_TEXT_OUTLINE_FAILED, TextOutlineRequest, apply_text_outline_paths,
    check_text_outline_source, reject_text_outline,
};

// Internal re-exports so sibling submodules can reach shared helpers via
// `super::`, exactly as before the split.
use run::{finish_candidate, format_source};
use tree::{
    find_node_any_mut, find_node_any_shared, find_node_shared, px, record_affected,
    subtree_contains,
};
