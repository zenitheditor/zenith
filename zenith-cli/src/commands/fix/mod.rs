//! `zenith fix`: apply machine-fixable diagnostics. Wiring only.

mod run;

pub use run::{AppliedFixJson, FixCmdErr, FixCmdOutcome, FixOutputJson, run};
