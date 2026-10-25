//! Variable-data merge command wiring.

mod bindings;
mod blocking;
mod combine;
mod names;
mod output;
mod run;

pub use combine::CombinedResult;
pub use names::sanitize_filename;
pub use output::{build_manifest, to_json_output};
pub use run::{
    MergeError, MergeReport, MergeTargets, RowResult, run, run_with_format, run_with_options,
    run_with_targets,
};
