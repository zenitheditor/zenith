//! Auto-layout engine: lowers `row` / `column` / `grid` frames to absolute
//! geometry before compile.
//!
//! The engine rewrites a clone of the document: every layout frame gets its
//! resolved box, and every flow child gets px `x` / `y` / `w` / `h`, counted
//! from its frame's top-left like any frame child. Anchors,
//! node boxes, connectors, text chains, compile, and inspect then read final
//! geometry and need no layout logic. A document without a layout frame is
//! never cloned. Wiring only.
//!
//! - `model` — boxes, per-axis sizing, resolved frame settings, child roles.
//! - `flex` — pure fill / line-break / justify / align math.
//! - `measure` — hug sizes of children ([`crate::compile::IntrinsicEnv`]).
//! - `memo` — the per-lowering memo of measured sizes.
//! - `solve` / `stack` — solve one frame (grid / row and column).
//! - `lower` — the walk that writes the solved geometry into the nodes,
//!   anchored roots included.
//! - `settle` — repeat passes until position-dependent heights settle.
//! - `write` — node geometry setters.
//! - `diag` — layout diagnostics.

mod diag;
mod flex;
mod lower;
mod measure;
mod memo;
mod model;
mod settle;
mod solve;
mod stack;
mod write;

pub(crate) use lower::{Lowered, lower_nodes};
pub use model::LayoutBox;
pub(crate) use settle::{NodeBoxes, may_depend_on_position, settle};
