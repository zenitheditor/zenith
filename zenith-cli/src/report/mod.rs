//! Machine-readable and human failure and diagnostic reporting.
//!
//! - `error` — [`CliError`] and the `zenith-error-v1` envelope for failures
//!   outside a command's own JSON shape.
//! - `human` — grouped human diagnostic lines.
//! - `location` — 1-based line/column from a byte span.

mod error;
mod human;
mod location;

pub(crate) use error::CliError;
pub(crate) use human::human_diagnostic_lines;
pub(crate) use location::line_col;
