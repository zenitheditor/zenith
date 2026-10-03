//! Geometry op application: `set_geometry`, `align_nodes`,
//! `align_to_edge`, and `distribute_nodes`, plus the bbox accessors they
//! share. Wiring only.
//!
//! - `boxes` — read and write a node box, page-space conversion, page bounds.
//! - `set` — apply `set_geometry`.
//! - `required` — reject a `set_geometry` `null` that removes a needed box field.
//! - `align` — apply `align_nodes` and `align_to_edge`.
//! - `distribute` — apply `distribute_nodes`.

mod align;
mod boxes;
mod distribute;
mod required;
mod set;

pub(in crate::engine) use align::{apply_align_nodes, apply_align_to_edge};
pub(in crate::engine) use boxes::node_geometry_mut;
pub(in crate::engine) use distribute::apply_distribute_nodes;
pub(in crate::engine) use set::{GeometryDelta, apply_set_geometry};
