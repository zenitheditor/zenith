//! WCAG 3 (APCA) contrast advisory check.
//!
//! Compares text-node fills against the colour they visually sit on: the
//! topmost preceding painted backdrop that geometrically covers the text in
//! page coordinates, falling back to the page background. The metric is APCA
//! lightness contrast (`Lc`) with the current WCAG 3 draft thresholds.
//!
//! - `entry` — the validation text pass and the compile-stage label pass.
//! - `walk` — the paint-order walk that collects backdrop candidates.
//! - `text` — backdrop sampling and the APCA judgement of text nodes.
//! - `table` — text inside table cells, judged per cell backdrop.
//! - `label` — measured `shape` / `connector` label ink.
//! - `paint` — `fill` to backdrop paint resolution.
//! - `props` / `geometry` — property and coverage-geometry helpers.
//! - `types` — shared value types.

mod entry;
mod geometry;
mod label;
mod paint;
mod props;
mod table;
mod text;
mod types;
mod walk;

pub(super) use entry::check_page_text_contrast;
pub use entry::label_contrast_checks;
pub use label::LabelInk;
