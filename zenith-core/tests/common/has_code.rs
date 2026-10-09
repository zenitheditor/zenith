//! `has_code`, shared by the test binaries that assert on diagnostic codes.

use zenith_core::ValidationReport;

pub fn has_code(report: &ValidationReport, code: &str) -> bool {
    report.diagnostics.iter().any(|d| d.code == code)
}
