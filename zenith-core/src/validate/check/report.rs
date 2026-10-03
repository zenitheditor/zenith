//! The validation outcome type.
//!
//! [`ValidationReport`] is the public result of a full document validation
//! pass; it is re-exported from the check module root as part of the crate's
//! public validate API.

use crate::diagnostics::Diagnostic;

/// The outcome of a full document validation pass.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationReport {
    /// All diagnostics collected during validation (token resolution +
    /// document-level checks). Never causes a hard panic; always complete.
    pub diagnostics: Vec<Diagnostic>,
}

impl ValidationReport {
    /// Returns `true` if any diagnostic has error severity.
    pub fn has_errors(&self) -> bool {
        Diagnostic::has_errors(&self.diagnostics)
    }
}
