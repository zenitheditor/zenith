//! Page lint: visual QA over the final geometry of one page compile.
//! Wiring only.
//!
//! - `ledger` — every compiled node in paint order, plus the text glyph ink.
//! - `paint` — opaque regions, effects, and authored placement of a node.
//! - `geom` — rectangle math and coverage outlines.
//! - `overlap` — `text.ink_overlap` and `text.occluded`.
//! - `label_overflow` — `label.overflow`.
//! - `contrast` — label contrast, and text contrast of expanded content.
//! - `run` — the entry point the page compile calls.

mod contrast;
mod geom;
mod label_overflow;
mod ledger;
mod overlap;
mod paint;
mod run;

pub(in crate::compile) use paint::PaintEnv;
pub(in crate::compile) use run::{LintEnv, lint_page};
