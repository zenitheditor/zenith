//! Container-node compilation: `frame` (clip-only) and `group` (translate +
//! opacity cascade), plus `instance` expansion and the bounding-box helpers used
//! to determine a group's rotation pivot.
//!
//! This module-root is wiring only: it declares the concern-grouped submodules
//! and re-exports the entry points consumed by the parent `compile` module. The
//! shared [`NodeCtx`](super::NodeCtx) borrow bundle threaded through every
//! container compiler is defined in the parent `compile::ctx` module.

mod flow;
mod frame;
mod group;
mod instance;

// Entry points consumed by the parent `compile` module (`compile::mod`). The
// external `use container::{...}` paths resolve through these re-exports.
pub(super) use frame::compile_frame;
pub(super) use group::compile_group;
pub(super) use instance::{compile_instance, prefix_ids_in_children};
