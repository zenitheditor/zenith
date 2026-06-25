//! `chart` node compilation — axis frame, scale, plot area.
//!
//! Wiring only: submodule declarations and the single public re-export.
//! No business logic lives here (AGENTS.md: module-root files are wiring only).

mod axis;
mod entry;
mod frame;
mod scale;

pub(in crate::compile) use entry::compile_chart;
