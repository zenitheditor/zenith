//! The `text` leaf compile path.
//!
//! Wiring only: the submodules carry the logic.
//! - `autofit`: the public `compile_text` entry and the `overflow="autofit"`
//!   shrink-to-fit search.
//! - `sized`: the sized layout engine (`compile_text_core`) with its fast
//!   single-line path, tab-leader/chain/markdown branches, and
//!   effect/mask/blend/rotation brackets. The multi-sub-path WRAP body lives in
//!   [`super::wrap`].
//! - `overflow`: the post-emit overflow measurement and the per-mode
//!   `text.overflow` / `text.fit_failed` diagnostic text.
//! - `fit`: the shared downward fit search (autofit + diagnostics) and
//!   `compile_text_sized`, the sized compile with its overflow diagnostic.

mod autofit;
mod fit;
mod overflow;
mod sized;

pub(in crate::compile) use autofit::compile_text;
pub(in crate::compile) use fit::{LabelHost, compile_label_text, compile_text_sized};
