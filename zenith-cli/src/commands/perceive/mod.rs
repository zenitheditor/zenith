//! `zenith perceive`: deterministic perception signals.
//!
//! - [`vector`] — path metrics read from the document source.
//! - [`page`] — page metrics measured from the render, shared with MCP.

mod outcome;
mod page;
mod vector;

pub use outcome::{PerceiveCmdErr, PerceiveOutcome};
pub use page::{page, page_report};
pub use vector::vector;
