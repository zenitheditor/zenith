//! `perceive page`: deterministic signals measured from a page render.
//!
//! [`perceive_pages`] renders pages as `zenith render --png` does at scale 1,
//! cuts the trim box, and runs the page-level metrics of
//! `zenith-perception`. The result is the schema-1 [`PerceiveReport`]
//! envelope. The signals describe the pixels and declared geometry. They are
//! not a judgement of design quality.

mod analyze;
mod inputs;
mod output;
mod raster;
mod run;

pub use output::{
    BoundsOutput, ClearspaceOutput, DensityCellOutput, DensityOutput, DensityRatiosOutput,
    DiagnosticOutput, EdgeOutput, Fixed, GridConformanceOutput, HistogramOutput, InkOutput, Metric,
    OpticalBalanceOutput, PERCEIVE_DECIMALS, PERCEIVE_NOTE, PERCEIVE_SCHEMA, PageMetrics,
    PagePerception, PerceiveReport, ValueZonesOutput, ZoneOutput, report_json,
};
pub use run::{PerceiveArtifact, perceive_pages};
