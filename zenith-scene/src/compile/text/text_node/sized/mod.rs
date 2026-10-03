//! The sized `text` layout engine (`compile_text_core`).
//!
//! Wiring only: the submodules carry the logic.
//! - `engine`: the orchestrator (branches, brackets, overflow).
//! - `style`: the `Copy` node style and box layout bundles.
//! - `origin`: resolve the box origin from authored geometry or anchors.
//! - `valign`: the `v-align` pre-offset.
//! - `spans`: footnote expansion, span styling, and span shaping.
//! - `line`: the fast single-line emit (backgrounds, decorations, glyphs).
//! - `wrapped`: the hand-off to the multi-line wrap path.
//! - `tab_leader`: the table-of-contents branch.
//! - `effects`: the blur / shadow / filter pick.
//!
//! The multi-sub-path WRAP body lives in [`crate::compile::text::wrap`];
//! overflow measurement lives in [`super::overflow`] and the overflow
//! diagnostics in [`super::fit`].

mod effects;
mod engine;
mod line;
mod origin;
mod spans;
mod style;
mod tab_leader;
mod valign;
mod wrapped;

pub(super) use engine::compile_text_core;
