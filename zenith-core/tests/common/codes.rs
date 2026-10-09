//! `codes`, shared by the test binaries that list diagnostic codes.

use zenith_core::ValidationReport;

pub fn codes(report: &ValidationReport) -> Vec<&str> {
    report.diagnostics.iter().map(|d| d.code.as_str()).collect()
}
