//! `chart` node compilation — axis frame, scale, plot area, bar/line/area/
//! sparkline/pie/donut, and the chart text look.
//!
//! Wiring only: submodule declarations and the public re-exports.
//! No business logic lives here (AGENTS.md: module-root files are wiring only).

mod axis;
mod bar;
mod bar_emit;
mod cartesian;
mod entry;
mod frame;
mod hbar;
mod legend;
mod line;
mod look;
mod palette;
mod pie;
mod role;
mod scale;
mod text;

pub(in crate::compile) use entry::compile_chart;
pub(in crate::compile) use role::{ChartTextRole, parse_chart_source};
