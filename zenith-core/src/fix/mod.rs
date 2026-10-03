//! `zenith fix`: apply machine-fixable diagnostics to `.zen` source.
//!
//! Pure and deterministic. The CLI reads the file, calls [`fix_source`], and
//! writes the result.
//!
//! - `locate` — find nodes and property entries by node id.
//! - `plan` — map diagnostics to edits.
//! - `mint` — semantic ids and placement for new tokens.
//! - `apply` — write edits into the KDL document.
//! - `run` — the validate → plan → apply → format loop.
//! - `diff` — unified source diff for dry-run output.

mod apply;
mod diff;
mod locate;
mod mint;
mod plan;
mod run;

pub use diff::unified_diff;
pub use mint::MintedToken;
pub use plan::AppliedFix;
pub use run::{FixOutcome, fix_source, fix_source_with};
