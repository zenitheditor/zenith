//! Threaded text flow ("text chain") pre-pass.
//!
//! A *chain* is the set of `text` nodes that share the same `chain` id. A long
//! article placed in the FIRST member (source order) flows across every
//! member's box in order: each box consumes as much text as fits, and the
//! remainder continues in the next box. This enables tri-fold leaflet panels
//! where one article spans three text boxes.
//!
//! This module runs ONCE per document (across ALL pages), BEFORE the main
//! compile walk, producing a single [`ChainAssignments`] map keyed by global
//! node id. A chain may span boxes on DIFFERENT pages: members are collected in
//! (page-order, then source-order) and the source content is poured greedily
//! across every member — box 1 fills, the remainder flows to box 2, … across
//! page boundaries. [`super::text::compile_text`] consults that map: a chain
//! member renders its ASSIGNED lines (via the shared [`super::text::emit_lines`])
//! instead of wrapping its own spans; a non-chain node is wholly unaffected
//! (byte-identical). The same document-wide map is threaded into every
//! `compile_page` call so a node on page 3 renders the slice it was assigned.
//!
//! ## v0 design choices (documented)
//!
//! - **Content source.** The chain's content is the spans of the FIRST member
//!   (source order) that has non-empty spans. Later members are continuation
//!   slots and declare `chain=id` with empty spans. If more than one member
//!   carries spans, only the first member's spans are used — spans are NOT
//!   concatenated (kept simple for v0).
//! - **Shared style.** All members are assumed to share font family/size/
//!   weight/fill. The whole chain is shaped ONCE with the first member's
//!   resolved style (+ per-span overrides). Each box re-wraps to its OWN width,
//!   so line height is uniform across the chain even when boxes differ in width.
//! - **Geometry source.** A chain member must carry `x`/`y`/`w`/`h` geometry
//!   resolvable to pixels. The pre-pass runs after auto-layout lowering, so a
//!   member placed by a row/column/grid frame has its laid-out box. A chain
//!   member has no content size of its own: in a layout frame it needs a fixed
//!   or `fill` size on each axis it does not stretch on (else
//!   `layout.unsized_child`).
//! - **Opacity cascade.** The pre-pass shapes colors at opacity 1.0 (no group/
//!   frame opacity cascade), so placing chain members under an opacity-cascading
//!   group is a documented follow-up.
//!
//! ## Determinism
//!
//! Members are collected in document source order (a depth-first walk, frames
//! and groups included). The result is a [`BTreeMap`] keyed by node id, and the
//! shaping reuses the deterministic engine. No `HashMap`/time/random reaches
//! output.
//!
//! Wiring only: the submodules carry the logic.
//! - `types`: the [`ChainAssignment`]/[`ChainAssignments`] result types.
//! - `collect`: the depth-first member collection.
//! - `style`: the chain source's shared render-style resolution.
//! - `distribute`: the inline + markdown-block distributors.
//! - `driver`: the document-wide `resolve_chains_document` entry.

mod collect;
mod distribute;
mod driver;
mod style;
mod types;

pub(super) use driver::resolve_chains_document;
pub(crate) use types::{ChainAssignment, ChainAssignments};
