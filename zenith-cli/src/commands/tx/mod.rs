//! `zenith tx` and `zenith outline-text`: module wiring.
//!
//! - `run`      — entry points, error and outcome types, the shared tail.
//! - `boxes`    — compiled page boxes, the box delta, `tx.page_box_changed`.
//! - `collapse` — fold rigid subtrees of the box delta into one entry.
//! - `tree`     — the id tree: parent, siblings, descendants, ancestors.
//! - `render`   — human and JSON output.

mod boxes;
mod collapse;
mod render;
mod run;
mod tree;

pub use boxes::BoxDelta;
pub use render::{TxView, render_human, status_json, status_label};
pub use run::{TxCmdErr, TxCtx, TxOutcome, run, run_outline_text, run_with, status_exit_code};
