//! Container-node compilation: `frame` (paint + clip, no translation) and
//! `group` (translate + opacity cascade), plus `instance` expansion and the bounding-box helpers used
//! to determine a group's rotation pivot.
//!
//! This module-root is wiring only: it declares the concern-grouped submodules
//! and re-exports the entry points consumed by the parent `compile` module. The
//! shared [`NodeCtx`](super::NodeCtx) borrow bundle threaded through every
//! container compiler is defined in the parent `compile::ctx` module.

mod frame;
mod frame_paint;
mod group;
mod instance;
mod wrap;

// Entry points consumed by the parent `compile` module (`compile::mod`). The
// external `use container::{...}` paths resolve through these re-exports.
pub(super) use frame::compile_frame;
pub(super) use group::{compile_group, group_children_bounds};
pub(super) use instance::{
    compile_instance, expand_imported, expand_local, prefix_ids_in_children, synthetic_group,
};
