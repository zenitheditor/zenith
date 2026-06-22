//! Container-node compilation: `frame` (clip-only) and `group` (translate +
//! opacity cascade), plus `instance` expansion and the bounding-box helpers used
//! to determine a group's rotation pivot.
//!
//! This module-root is wiring only: it declares the concern-grouped submodules,
//! defines the shared [`ContainerCtx`] borrow bundle threaded through every
//! container compiler, and re-exports the entry points consumed by the parent
//! `compile` module.

use std::collections::BTreeMap;

use zenith_core::{FontProvider, ResolvedToken, Style};
use zenith_layout::RustybuzzEngine;

use super::ComponentMap;
use super::anchor::AnchorMap;
use super::chain::ChainAssignments;
use super::field::FieldCtx;
use super::table_flow::TableFlowAssignments;

mod flow;
mod frame;
mod group;
mod instance;

// Entry points consumed by the parent `compile` module (`compile::mod`). The
// external `use container::{...}` paths resolve through these re-exports.
pub(super) use frame::compile_frame;
pub(super) use group::compile_group;
pub(super) use instance::{compile_instance, prefix_ids_in_children};

/// Immutable, shared borrows threaded through every container compiler.
///
/// The container compilers (`compile_frame`, `compile_group`, `compile_instance`
/// and their flow/grid helpers) all forward the same nine read-only lookups down
/// the recursion. Bundling them into one `Copy` struct keeps every signature
/// short — the mutable `commands`/`diagnostics` sinks and the per-subtree
/// [`RenderCtx`](super::RenderCtx) cascade stay as explicit parameters because
/// they are not shared-immutable.
///
/// Every field is a reference or a `Copy` value, so the struct is itself `Copy`
/// and can be forwarded freely without clones.
#[derive(Clone, Copy)]
pub(super) struct ContainerCtx<'a> {
    /// Resolved token table for dimension / color property resolution.
    pub(super) resolved: &'a BTreeMap<String, ResolvedToken>,
    /// Style id → style lookup for the style cascade.
    pub(super) style_map: &'a BTreeMap<&'a str, &'a Style>,
    /// Component definitions, for `instance` expansion.
    pub(super) components: &'a ComponentMap<'a>,
    /// Font provider used to shape text descendants.
    pub(super) fonts: &'a dyn FontProvider,
    /// Shaping engine used to shape text descendants.
    pub(super) engine: &'a RustybuzzEngine,
    /// Text-chain assignments cascaded into text descendants.
    pub(super) chains: &'a ChainAssignments,
    /// Table-flow assignments cascaded into table descendants.
    pub(super) flows: &'a TableFlowAssignments,
    /// Anchor map for `anchor`/`anchor_zone` derived placement.
    pub(super) anchors: &'a AnchorMap,
    /// Per-page field context (page index, live area, footnote markers, …).
    pub(super) field_ctx: &'a FieldCtx<'a>,
}
