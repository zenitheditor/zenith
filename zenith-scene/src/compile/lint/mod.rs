//! Page lint: visual QA over the final geometry of one page compile.
//! Wiring only.
//!
//! - `ledger` — every compiled node in paint order, plus the text glyph ink.
//! - `paint` — opaque regions, effects, and authored placement of a node.
//! - `geom` — rectangle math and coverage outlines.
//! - `overlap` — `text.ink_overlap` and `text.occluded`.
//! - `label_overflow` — `label.overflow`.
//! - `contrast` — text and label contrast, judged on drawn glyph ink.
//! - `legibility` — `text.edge_crowding`, `text.too_small`, and
//!   `type.near_duplicate_size` (`edges`, `small_text`, `type_scale`, over the
//!   authored facts of `text_facts`).
//! - `arrange` — `align.near_miss`, `spacing.uneven_gap`, and
//!   `connector.crosses_node` (`align`, `spacing`, `connector`).
//! - `blocks` — `layout.block_overlap`.
//! - `chart_overflow` — `chart.overflow`.
//! - `run` — the entry point the page compile calls.

mod align;
mod arrange;
mod blocks;
mod chart_overflow;
mod connector;
mod contrast;
mod edges;
mod geom;
mod label_overflow;
mod ledger;
mod legibility;
mod overlap;
mod paint;
mod run;
mod small_text;
mod spacing;
mod text_facts;
mod type_scale;

pub(in crate::compile) use paint::PaintEnv;
pub(in crate::compile) use run::{LintEnv, lint_page};
pub use small_text::text_size_floor_px;
pub use type_scale::sizes_read_as_one;
