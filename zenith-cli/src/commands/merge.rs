//! Variable-data merge command wiring.

mod output;
mod run;

pub use output::{build_manifest, to_json_output};
pub use run::{MergeError, MergeReport, RowResult, run, run_with_format, sanitize_filename};
