//! Variant generation: pure engine and CLI command wiring.
//!
//! - `engine` — the pure in-memory variant expansion engine.
//! - `run`    — the CLI entry point: file I/O, rendering, manifest, output.
//!
//! Module root is wiring only: re-exports + submodule declarations.

mod engine;
mod run;

pub use engine::{VariantExpansion, VariantOutcome, VariantResult, expand_variants};
pub use run::{
    VariantCmdErr, VariantOutputs, VariantReport, VariantResultRecord, build_manifest, run_variant,
    run_variant_with_format, run_variant_with_options, run_variant_with_output_constraints,
    to_json_output,
};
