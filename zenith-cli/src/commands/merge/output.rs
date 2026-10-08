//! Merge manifest and JSON report encoding.

use super::MergeReport;
use crate::json_types::{DiagnosticJson, MergeOutput, MergeRowResult};

/// Build a deterministic generation manifest from the merge inputs and report.
/// Inputs are hashed as bytes; NO timestamps, absolute paths, or crate version
/// are embedded, so identical inputs yield a byte-identical manifest.
/// Rows with committed files are included. Partial rows carry failed status without error text.
pub fn build_manifest(
    doc_src: &str,
    csv_src: &str,
    name_by: Option<&str>,
    report: &MergeReport,
) -> crate::json_types::MergeManifest {
    use sha2::{Digest, Sha256};
    // Format version of the manifest schema itself. Bump ONLY when the manifest
    // structure changes — never on a routine crate release (that would break
    // CI byte-identical comparison).
    const MANIFEST_FORMAT_VERSION: &str = "1";

    let source_sha256 = format!("{:x}", Sha256::digest(doc_src.as_bytes()));
    let data_sha256 = format!("{:x}", Sha256::digest(csv_src.as_bytes()));
    let rows = report
        .rows
        .iter()
        .filter(|r| !r.outputs.is_empty())
        .map(|r| crate::json_types::ManifestRow {
            row: r.row,
            key: r.key.clone(),
            outputs: r.outputs.clone(),
            status: r.failure.as_ref().map(|_| "failed"),
        })
        .collect();
    crate::json_types::MergeManifest {
        schema: "zenith-merge-manifest-v1",
        generator: MANIFEST_FORMAT_VERSION,
        source_sha256,
        data_sha256,
        name_by: name_by.map(str::to_owned),
        rows,
    }
}

/// Convert a completed [`MergeReport`] into the JSON-serialisable envelope.
pub fn to_json_output(report: &MergeReport) -> MergeOutput {
    let n_written = report.rows.iter().filter(|r| r.failure.is_none()).count();
    let n_failed = report.rows.iter().filter(|r| r.failure.is_some()).count();
    MergeOutput {
        schema: "zenith-merge-v1",
        total_rows: report.rows.len(),
        written: n_written,
        failed: n_failed,
        diagnostics: Vec::new(),
        rows: report
            .rows
            .iter()
            .map(|r| MergeRowResult {
                row: r.row,
                key: r.key.clone(),
                status: if r.failure.is_none() { "ok" } else { "failed" },
                outputs: r.outputs.clone(),
                diagnostics: {
                    let mut diagnostics = DiagnosticJson::located_all(&r.diagnostics, "");
                    if let Some(reason) = &r.failure
                        && !zenith_core::Diagnostic::has_errors(&r.diagnostics)
                    {
                        diagnostics.push(DiagnosticJson::error("merge.row_failed", reason.clone()));
                    }
                    diagnostics
                },
            })
            .collect(),
    }
}
