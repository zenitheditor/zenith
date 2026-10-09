//! Geometry op application: `set_geometry`, `nudge_geometry`,
//! `nudge_line_points`, `align_nodes`, `align_to_edge`, and
//! `distribute_nodes`, plus the bbox accessors they share. Wiring only.
//!
//! - `boxes` — read and write a node box, page-space conversion, page bounds.
//! - `set` — apply `set_geometry`.
//! - `axis` — offset one authored value by a px delta, keeping its unit.
//! - `nudge` — apply `nudge_geometry`.
//! - `line` — apply `nudge_line_points`.
//! - `required` — reject an edit that leaves a node without a needed box
//!   field, and read whether an anchor places a node.
//! - `align` — apply `align_nodes` and `align_to_edge`.
//! - `distribute` — apply `distribute_nodes`.

mod align;
mod axis;
mod boxes;
mod distribute;
mod line;
mod nudge;
mod required;
mod set;

pub(in crate::engine) use align::{apply_align_nodes, apply_align_to_edge};
pub(in crate::engine) use axis::{AxisCtx, dimension_px};
pub(in crate::engine) use boxes::node_geometry_mut;
pub(in crate::engine) use distribute::apply_distribute_nodes;
pub(in crate::engine) use line::{LineDelta, apply_nudge_line_points};
pub(in crate::engine) use nudge::{NudgeDelta, apply_nudge_geometry};
pub(in crate::engine) use required::{anchored, reject_unplaced};
pub(in crate::engine) use set::{GeometryDelta, apply_set_geometry};
