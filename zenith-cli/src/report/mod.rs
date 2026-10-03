//! Machine-readable and human failure and diagnostic reporting.
//!
//! - `error` — [`CliError`] and the `zenith-error-v1` envelope for failures
//!   outside a command's own JSON shape.
//! - `human` — grouped human diagnostic lines.
//! - `locator` — span to file, line, and column; the parse-error line.
//! - `source` — the file behind each diagnostic span.

mod error;
mod human;
mod locator;
mod source;

pub(crate) use error::CliError;
pub(crate) use human::human_diagnostic_lines;
pub(crate) use locator::{Locator, parse_error_line};
pub(crate) use source::{ImportFiles, attributed_loader_diagnostics};
