//! Table-node compilation: single-page tables with EXPLICIT and CONTENT-BASED
//! column widths, CONTENT-BASED row heights, and SEPARATE or COLLAPSE borders.
//!
//! This unit lays a `table` out as a grid of cells inside its declared
//! `[x, y, w, h]` box, honoring `colspan`/`rowspan` (HTML-table cell flow).
//! AUTO columns (a `column` with no `width`) size to their widest cell's
//! measured natural content; rows size to their tallest cell's wrapped content
//! height at the assigned column width. Both passes reuse the production
//! text-shaping pipeline (`shape_words`/`pack_lines`) via the measurer helpers
//! in [`super::text`]. `border-collapse="separate"` (the default) draws each
//! cell's four edges independently. `border-collapse="collapse"` deduplicates
//! shared edges so adjacent cells never double-draw their shared border.
//!
//! Each cell emits, in order: an optional background `FillRect` (cell.fill or
//! table.fill), then an optional border (four independent `StrokeLine`s in
//! separate mode; accumulated and deduplicated in collapse mode), then its
//! compiled child content clipped to and translated into the cell content box
//! (cell padding inset). The CELL provides each child's geometry: a `text`
//! child auto-wraps to the content width (unless it sets `w`), is horizontally
//! aligned by the cell/table `h-align` (via the text's own `align`), and is
//! offset vertically by `v-align` against its measured wrapped height — so
//! authors never hand-size cell text. Author-specified `w`/`x`/`y`/`align` win.
//! Non-text children keep the prior declared-box align-slack placement. Opacity
//! cascades (table.opacity × ctx.opacity).
//!
//! ## Module layout
//!
//! This is a wiring-only module root. The implementation is split by concern:
//! - [`place`] — the shared HTML-table occupancy walk ([`place_cells`] /
//!   [`PlacedCell`]), the [`CellRect`] geometry, and the per-child declared-box
//!   helper.
//! - [`layout`] — the SINGLE content-based sizing math ([`compute_table_layout`]
//!   / [`TableLayout`]) plus the per-cell width/height measurers.
//! - [`measure`] — the table's natural size for the auto-layout engine.
//! - [`collapse`] — `border-collapse="collapse"` edge deduplication.
//! - [`emit`] — the [`compile_table`] entry point and the per-cell emission.

mod collapse;
mod emit;
mod layout;
mod measure;
mod place;

pub(in crate::compile) use emit::{TableEmitCtx, compile_table};
pub(in crate::compile) use layout::{GridDims, compute_table_layout};
pub(in crate::compile) use measure::table_natural_size;
pub(in crate::compile) use place::place_cells;
