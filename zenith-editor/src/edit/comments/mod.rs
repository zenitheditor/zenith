//! The comments an edit dropped. Wiring only.
//!
//! - `lex` — the comments of a KDL text.
//! - `report` — the comments an edit dropped, with their lines.

pub(crate) mod lex;
mod report;

pub(crate) use report::removed_comments;
