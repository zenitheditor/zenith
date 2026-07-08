//! The catalog entry type, its policy verbs, and the entry constructor.

use crate::diagnostics::Severity;

/// One catalog entry: a stable diagnostic `code`, the [`Severity`] the engine
/// emits it at, and a one-line human summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticCodeInfo {
    /// The stable dot-separated code, e.g. `"layout.off_canvas"`.
    pub code: &'static str,
    /// The severity the engine emits this diagnostic at.
    pub severity: Severity,
    /// One-line description of what the diagnostic means.
    pub summary: &'static str,
}

impl DiagnosticCodeInfo {
    /// True when a `diagnostics { … }` policy entry can adjust this code — i.e.
    /// its severity is `Warning` or `Advisory`. Error-severity codes are
    /// immutable.
    pub fn is_governable(&self) -> bool {
        match self.severity {
            Severity::Error => false,
            Severity::Warning | Severity::Advisory => true,
        }
    }
}

/// The three policy verbs accepted inside a `diagnostics { … }` block, in
/// canonical order.
pub const DIAGNOSTIC_VERBS: &[&str] = &["allow", "deny", "warn"];

/// `const`-friendly constructor for a [`DiagnosticCodeInfo`] table entry.
pub(super) const fn info(
    code: &'static str,
    severity: Severity,
    summary: &'static str,
) -> DiagnosticCodeInfo {
    DiagnosticCodeInfo {
        code,
        severity,
        summary,
    }
}
