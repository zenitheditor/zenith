/// Severity for a local perception metric diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerceptionSeverity {
    Info,
    Warning,
}

/// A deterministic read-only metric diagnostic local to `zenith-perception`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerceptionDiagnostic {
    pub code: &'static str,
    pub severity: PerceptionSeverity,
    pub message: &'static str,
}

impl PerceptionDiagnostic {
    pub const fn new(
        code: &'static str,
        severity: PerceptionSeverity,
        message: &'static str,
    ) -> Self {
        Self {
            code,
            severity,
            message,
        }
    }
}
