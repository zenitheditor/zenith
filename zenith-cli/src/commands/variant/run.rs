//! CLI entry point for `zenith variant`.
//!
//! [`run_variant`] is the single public entry point.  It loads and parses the
//! input `.zen`, calls [`expand_variants`], renders each generated variant to
//! PNG, writes a side-by-side `.zen` for review, and optionally writes a
//! deterministic generation manifest.
//!
//! No clap parsing lives here — argument types are in `cli.rs`.  No FS I/O
//! lives in `lib.rs` — it is all here or in the callee chain.

use std::collections::BTreeSet;
use std::path::Path;

use zenith_core::{BytesAssetProvider, KdlAdapter, KdlSource, Severity};
use zenith_render::render_png;
use zenith_scene::compile_page;

use crate::commands::render::{
    build_asset_provider, build_font_provider, collect_missing_asset_diagnostics,
};
use crate::json_types::{
    DiagnosticJson, VariantManifest, VariantManifestTarget, VariantOutput, VariantResultJson,
};

use super::engine::{VariantOutcome, expand_variants};

// ── Error type ────────────────────────────────────────────────────────────────

/// A fatal error that prevents variant generation from starting.
///
/// Exit code 2 for all setup errors (consistent with `MergeError`).
#[derive(Debug)]
pub struct VariantCmdErr {
    /// Human-readable message.
    pub message: String,
    /// Recommended exit code (always 2 for setup errors).
    pub exit_code: u8,
}

impl VariantCmdErr {
    fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            exit_code: 2,
        }
    }
}

// ── Report types ──────────────────────────────────────────────────────────────

/// Output paths for a single generated variant.
#[derive(Debug)]
pub struct VariantOutputs {
    /// The `.zen` source written to disk (relative filename within `out_dir`).
    pub zen: String,
    /// The rendered `.png` written to disk (relative filename within `out_dir`).
    pub png: String,
}

/// Result record for one variant (generated or failed).
#[derive(Debug)]
pub struct VariantResultRecord {
    /// The variant's stable id.
    pub id: String,
    /// The source page id this variant derives from.
    pub source: String,
    /// Output files written — `None` when `failure` is set.
    pub outputs: Option<VariantOutputs>,
    /// `None` = generated successfully; `Some(reason)` = failed.
    pub failure: Option<String>,
}

/// Summary of a completed variant-generation run.
#[derive(Debug)]
pub struct VariantReport {
    /// All per-variant records, in ascending variant-id order.
    pub variants: Vec<VariantResultRecord>,
}

impl VariantReport {
    /// Number of variants that were generated successfully.
    pub fn generated(&self) -> usize {
        self.variants.iter().filter(|r| r.failure.is_none()).count()
    }

    /// References to the records that failed, in id order.
    pub fn failed(&self) -> Vec<&VariantResultRecord> {
        self.variants
            .iter()
            .filter(|r| r.failure.is_some())
            .collect()
    }
}

// ── run_variant ───────────────────────────────────────────────────────────────

/// Run variant generation for all `variants` blocks in `doc_src`.
///
/// # Parameters
///
/// - `doc_src`     — UTF-8 source of the input `.zen` document.
/// - `project_dir` — directory of the `.zen` file (for asset/font resolution).
/// - `out_dir`     — directory to write `<stem>-<id>.zen` and `<stem>-<id>.png`.
/// - `stem`        — output file stem (typically the `.zen` filename without extension).
///
/// # Errors
///
/// Returns [`VariantCmdErr`] (exit code 2) for setup failures that prevent any
/// variant from being processed.  Per-variant failures are recorded in
/// [`VariantReport::failed`] and do not cause an `Err` return.
pub fn run_variant(
    doc_src: &str,
    project_dir: Option<&Path>,
    out_dir: &Path,
    stem: &str,
) -> Result<VariantReport, VariantCmdErr> {
    // ── 1. Parse the input document ───────────────────────────────────────
    let doc = KdlAdapter
        .parse(doc_src.as_bytes())
        .map_err(|e| VariantCmdErr::new(format!("error[parse.error]: {}", e.message)))?;

    // ── 2. Expand variants (pure engine — no I/O) ─────────────────────────
    let expansion = expand_variants(&doc);

    // An empty expansion (no variants block) is not an error; we return an
    // empty report so the caller can produce a "0 generated" summary.

    // ── 3. Build font + asset providers ONCE from the original doc ────────
    let fonts =
        build_font_provider(&doc, project_dir, false).map_err(|e| VariantCmdErr::new(e.message))?;
    let template_assets = match project_dir {
        Some(dir) => {
            build_asset_provider(&doc, dir, false).map_err(|e| VariantCmdErr::new(e.message))?
        }
        None => BytesAssetProvider::new(),
    };

    // ── 4. Ensure output directory exists ─────────────────────────────────
    std::fs::create_dir_all(out_dir).map_err(|e| {
        VariantCmdErr::new(format!(
            "could not create output directory '{}': {}",
            out_dir.display(),
            e
        ))
    })?;

    // ── 5. Pre-flight collision check ─────────────────────────────────────
    // Distinct variant ids always produce distinct stems, so collisions are
    // not expected; the check mirrors merge's safety pattern.
    let mut used_names: BTreeSet<String> = BTreeSet::new();
    let mut collision_err: Option<String> = None;
    for result in &expansion.results {
        if !matches!(result.outcome, VariantOutcome::Generated(_)) {
            continue;
        }
        let zen_name = format!("{}-{}.zen", stem, result.id);
        let png_name = format!("{}-{}.png", stem, result.id);
        for name in [&zen_name, &png_name] {
            if used_names.contains(name.as_str()) {
                collision_err = Some(format!("output filename collision: {name}"));
                break;
            }
            used_names.insert(name.clone());
        }
        if collision_err.is_some() {
            break;
        }
    }
    if let Some(msg) = collision_err {
        return Err(VariantCmdErr::new(msg));
    }

    // ── 6. Process each variant result ────────────────────────────────────
    let mut records: Vec<VariantResultRecord> = Vec::with_capacity(expansion.results.len());

    for result in expansion.results {
        match result.outcome {
            VariantOutcome::Failed(reason) => {
                records.push(VariantResultRecord {
                    id: result.id,
                    source: result.source,
                    outputs: None,
                    failure: Some(reason),
                });
            }
            VariantOutcome::Generated(materialized) => {
                let zen_name = format!("{}-{}.zen", stem, result.id);
                let png_name = format!("{}-{}.png", stem, result.id);

                // ── 6a. Write the materialized `.zen` ─────────────────────
                let zen_bytes = match KdlAdapter.format(&materialized) {
                    Ok(b) => b,
                    Err(e) => {
                        records.push(VariantResultRecord {
                            id: result.id,
                            source: result.source,
                            outputs: None,
                            failure: Some(format!("format error: {}", e)),
                        });
                        continue;
                    }
                };
                let zen_path = out_dir.join(&zen_name);
                if let Err(e) = std::fs::write(&zen_path, &zen_bytes) {
                    records.push(VariantResultRecord {
                        id: result.id,
                        source: result.source,
                        outputs: None,
                        failure: Some(format!("write error '{}': {}", zen_path.display(), e)),
                    });
                    continue;
                }

                // ── 6b. Find the source page index in the materialized doc ─
                let page_index = match materialized
                    .body
                    .pages
                    .iter()
                    .position(|p| p.id == result.source)
                {
                    Some(idx) => idx,
                    None => {
                        // Source page missing in materialized doc — clean up the
                        // .zen we already wrote and record failure.
                        let _ = std::fs::remove_file(&zen_path);
                        let failure = format!(
                            "source page '{}' not found in materialized document",
                            result.source
                        );
                        records.push(VariantResultRecord {
                            id: result.id,
                            source: result.source,
                            outputs: None,
                            failure: Some(failure),
                        });
                        continue;
                    }
                };

                // ── 6c. Gate on hard asset diagnostics ────────────────────
                if let Some(dir) = project_dir {
                    let missing_diags = collect_missing_asset_diagnostics(&materialized, dir);
                    let hard: Vec<String> = missing_diags
                        .iter()
                        .filter(|d| d.severity == Severity::Error)
                        .map(crate::commands::format_error_diag)
                        .collect();
                    if !hard.is_empty() {
                        let _ = std::fs::remove_file(&zen_path);
                        records.push(VariantResultRecord {
                            id: result.id,
                            source: result.source,
                            outputs: None,
                            failure: Some(format!("asset error(s): {}", hard.join("; "))),
                        });
                        continue;
                    }
                }

                // ── 6d. Compile the source page ───────────────────────────
                let compile_result = compile_page(&materialized, &fonts, page_index, None);

                let hard_diags: Vec<String> = compile_result
                    .diagnostics
                    .iter()
                    .filter(|d| d.severity == Severity::Error)
                    .map(crate::commands::format_error_diag)
                    .collect();
                if !hard_diags.is_empty() {
                    let _ = std::fs::remove_file(&zen_path);
                    records.push(VariantResultRecord {
                        id: result.id,
                        source: result.source,
                        outputs: None,
                        failure: Some(format!("compile error(s): {}", hard_diags.join("; "))),
                    });
                    continue;
                }

                // ── 6e. Render to PNG ─────────────────────────────────────
                let png_bytes = match render_png(&compile_result.scene, &fonts, &template_assets) {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = std::fs::remove_file(&zen_path);
                        records.push(VariantResultRecord {
                            id: result.id,
                            source: result.source,
                            outputs: None,
                            failure: Some(format!("render error: {}", e)),
                        });
                        continue;
                    }
                };

                // ── 6f. Write PNG ─────────────────────────────────────────
                let png_path = out_dir.join(&png_name);
                if let Err(e) = std::fs::write(&png_path, &png_bytes) {
                    let _ = std::fs::remove_file(&zen_path);
                    records.push(VariantResultRecord {
                        id: result.id,
                        source: result.source,
                        outputs: None,
                        failure: Some(format!("write error '{}': {}", png_path.display(), e)),
                    });
                    continue;
                }

                records.push(VariantResultRecord {
                    id: result.id,
                    source: result.source,
                    outputs: Some(VariantOutputs {
                        zen: zen_name,
                        png: png_name,
                    }),
                    failure: None,
                });
            }
        }
    }

    Ok(VariantReport { variants: records })
}

// ── build_manifest ────────────────────────────────────────────────────────────

/// Build a deterministic generation manifest from the variant inputs and report.
///
/// `source_sha256` is the SHA-256 of the input `.zen` bytes.  No timestamps,
/// absolute paths, or crate versions are embedded — identical inputs yield a
/// byte-identical manifest.  Only successfully-generated variants are included.
pub fn build_manifest(doc_src: &str, report: &VariantReport) -> VariantManifest {
    use sha2::{Digest, Sha256};

    // Bump only when the manifest structure changes — never on a routine
    // crate release (that would break CI byte-identical comparison).
    const MANIFEST_FORMAT_VERSION: &str = "1";

    let source_sha256 = format!("{:x}", Sha256::digest(doc_src.as_bytes()));

    let targets = report
        .variants
        .iter()
        .filter(|r| r.failure.is_none())
        .filter_map(|r| {
            let outputs = r.outputs.as_ref()?;
            Some(VariantManifestTarget {
                id: r.id.clone(),
                source: r.source.clone(),
                outputs_zen: outputs.zen.clone(),
                outputs_png: outputs.png.clone(),
            })
        })
        .collect();

    VariantManifest {
        schema: "zenith-variant-manifest-v1",
        generator: MANIFEST_FORMAT_VERSION,
        source_sha256,
        targets,
    }
}

// ── to_json_output ────────────────────────────────────────────────────────────

/// Convert a completed [`VariantReport`] into the JSON-serialisable envelope.
pub fn to_json_output(report: &VariantReport) -> VariantOutput {
    let n_generated = report.generated();
    let n_failed = report.failed().len();
    VariantOutput {
        schema: "zenith-variant-v1",
        total_variants: report.variants.len(),
        generated: n_generated,
        failed: n_failed,
        variants: report
            .variants
            .iter()
            .map(|r| VariantResultJson {
                id: r.id.clone(),
                source: r.source.clone(),
                status: if r.failure.is_none() { "ok" } else { "failed" },
                outputs_zen: r.outputs.as_ref().map(|o| o.zen.clone()),
                outputs_png: r.outputs.as_ref().map(|o| o.png.clone()),
                diagnostics: match &r.failure {
                    None => Vec::new(),
                    Some(reason) => vec![DiagnosticJson {
                        code: "variant.failed".to_owned(),
                        severity: "error".to_owned(),
                        message: reason.clone(),
                        subject_id: Some(r.id.clone()),
                    }],
                },
            })
            .collect(),
    }
}
