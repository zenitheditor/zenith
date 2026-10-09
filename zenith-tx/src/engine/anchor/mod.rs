//! Anchor ops: `set_anchor`, `nudge_anchor_gap`, and `detach_anchor`.
//! Wiring only.
//!
//! - `fields` — mutable access to a node's anchor attributes and `x` / `y`.
//! - `set` — apply `set_anchor` (value checks, then the writes).
//! - `gap` — apply `nudge_anchor_gap`.
//! - `detach` — apply `detach_anchor`.

mod detach;
mod fields;
mod gap;
mod set;

pub(super) use detach::apply_detach_anchor;
pub(in crate::engine) use fields::anchor_fields_mut;
pub(super) use gap::apply_nudge_anchor_gap;
pub(super) use set::apply_set_anchor;
