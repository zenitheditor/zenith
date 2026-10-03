//! Auto-layout ops: `set_layout`, the `hug` / `fill` size keywords of
//! `set_geometry`, and the guard that keeps hand-placement ops off in-flow
//! children of `row` / `column` / `grid` frames. Wiring only.
//!
//! - `set` — apply `set_layout` (value checks, then the writes).
//! - `size` — parse and write `set_geometry` `w` / `h` keywords.
//! - `guard` — the structural in-flow test and `tx.layout_managed`.

mod guard;
mod set;
mod size;

pub(super) use guard::{places_in_flow, reject_layout_managed};
pub(super) use set::apply_set_layout;
pub(super) use size::{SizeArg, parse_size_arg, write_size_keywords};
