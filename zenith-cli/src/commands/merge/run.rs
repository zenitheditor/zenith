//! Pure logic for `zenith merge`.
//!
//! The public entry point [`run`] operates entirely on in-memory source text
//! plus filesystem paths for outputs.  The source `.zen` file is NEVER
//! mutated; each row's document is produced in-memory via the transaction
//! engine and re-parsed before compilation.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use zenith_core::Diagnostic;
use zenith_core::{AssetKind, BytesAssetProvider, KdlAdapter, KdlSource};
use zenith_scene::{DocumentPrep, PageCompiler};
use zenith_tx::{AddAssetMetadata, Op, OpSpan, Transaction, TxStatus, run_transaction};

use zenith_pipeline::PolicyFlags;
use zenith_pipeline::assets::{
    build_asset_provider, build_font_provider, collect_missing_asset_diagnostics, image_sizes,
    resolve_text_sources,
};
use zenith_pipeline::policy::ConfigLayers;

use super::bindings::{AssetBinding, DataBinding, collect_data_nodes};
use super::blocking::block_row_defects;
use super::combine::{CombinedResult, Combiner, check_combine};
use super::names::{row_filenames, sanitize_filename};
use crate::commands::render::{
    BatchExportOptions, BatchFormat, BatchLayout, encode_batch_pdf, encode_batch_scene,
    load_batch_policy,
};
use crate::native::{self, NativeConfig, NativeFs};

// ── Error type ────────────────────────────────────────────────────────────────

/// A fatal error that prevents the merge from starting.
///
/// Exit code 2 for all setup/template errors (consistent with the other
/// commands whose `PipelineError`/`FmtErr`/`TxCmdErr` all use 2 for this class
/// of failure).
#[derive(Debug)]
pub struct MergeError {
    /// Human-readable message.
    pub message: String,
    /// Recommended exit code (always 2 for template/setup errors).
    pub exit_code: u8,
}

impl MergeError {
    pub(super) fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            exit_code: 2,
        }
    }
}

// ── Report types ──────────────────────────────────────────────────────────────

/// Result for one CSV data row.
#[derive(Debug)]
pub struct RowResult {
    /// 0-based CSV data row index.
    pub row: usize,
    /// The --name-by cell value, or None.
    pub key: Option<String>,
    /// Committed filenames in page order, including partially written failed rows.
    pub outputs: Vec<String>,
    /// None = ok; Some(reason) = failed.
    pub failure: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Summary of a completed merge run. `rows` is the single source of truth,
/// in CSV order.
#[derive(Debug)]
pub struct MergeReport {
    pub rows: Vec<RowResult>,
    /// The `--combine` outcome. `None` when no combined file is asked for.
    pub combined: Option<CombinedResult>,
}

/// Paths a merge writes or protects beyond the row files in `out_dir`.
#[derive(Debug, Clone, Copy, Default)]
pub struct MergeTargets<'a> {
    /// Paths no output can replace: the inputs and the manifest.
    pub reserved: &'a [&'a Path],
    /// Also write one PDF of every successful row's pages here, in CSV order.
    /// It requires PDF output.
    pub combine: Option<&'a Path>,
}

impl MergeReport {
    /// All successfully-written filenames, in row→page order.
    pub fn written(&self) -> Vec<String> {
        self.rows
            .iter()
            .flat_map(|r| r.outputs.iter().cloned())
            .collect()
    }
    /// References to the rows that failed, in CSV order.
    pub fn failed(&self) -> Vec<&RowResult> {
        self.rows.iter().filter(|r| r.failure.is_some()).collect()
    }
    /// Whether any row or the combined file failed.
    pub fn has_failures(&self) -> bool {
        self.rows.iter().any(|r| r.failure.is_some())
            || self
                .combined
                .as_ref()
                .is_some_and(|combined| combined.failure.is_some())
    }
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Run a mail-merge: for each CSV row, build a per-row document (in-memory),
/// compile it, render it to PNG, and stream the file to `out_dir`.
///
/// # Parameters
///
/// - `doc_src`     — UTF-8 source of the template `.zen` document.
/// - `csv_src`     — UTF-8 CSV with a header row.
/// - `project_dir` — directory of the `.zen` file (for asset resolution).
/// - `out_dir`     — directory to write one PNG per row into.
/// - `name_by`     — CSV column to derive filenames from; default `row-NNNN.png`.
///
/// # Errors
///
/// Returns [`MergeError`] (exit code 2) for template/setup failures that
/// prevent any row from being processed.  Per-row failures are collected into
/// [`MergeReport::failed`] and do not cause an `Err` return.
pub fn run(
    doc_src: &str,
    csv_src: &str,
    project_dir: Option<&Path>,
    out_dir: &Path,
    name_by: Option<&str>,
) -> Result<MergeReport, MergeError> {
    run_with_format(
        doc_src,
        csv_src,
        project_dir,
        out_dir,
        name_by,
        BatchFormat::Png,
    )
}

pub fn run_with_format(
    doc_src: &str,
    csv_src: &str,
    project_dir: Option<&Path>,
    out_dir: &Path,
    name_by: Option<&str>,
    format: BatchFormat,
) -> Result<MergeReport, MergeError> {
    run_with_options(
        doc_src,
        csv_src,
        project_dir,
        out_dir,
        name_by,
        BatchExportOptions::from(format),
    )
}

pub fn run_with_options(
    doc_src: &str,
    csv_src: &str,
    project_dir: Option<&Path>,
    out_dir: &Path,
    name_by: Option<&str>,
    options: BatchExportOptions,
) -> Result<MergeReport, MergeError> {
    run_with_targets(
        doc_src,
        csv_src,
        project_dir,
        out_dir,
        name_by,
        options,
        MergeTargets::default(),
    )
}

/// Run a merge that protects `targets.reserved` and can write a combined PDF.
///
/// # Errors
///
/// As [`run`], plus `--combine` with a format other than PDF, and a reserved
/// path that a planned output aliases.
pub fn run_with_targets(
    doc_src: &str,
    csv_src: &str,
    project_dir: Option<&Path>,
    out_dir: &Path,
    name_by: Option<&str>,
    options: BatchExportOptions,
    targets: MergeTargets<'_>,
) -> Result<MergeReport, MergeError> {
    options.check().map_err(MergeError::new)?;
    let format = options.format;
    check_combine(format, targets.combine).map_err(MergeError::new)?;
    // ── 1. Parse the template document (once) ─────────────────────────────
    let doc = KdlAdapter
        .parse(doc_src.as_bytes())
        .map_err(|e| MergeError::new(crate::report::parse_error_line(doc_src, &e)))?;
    let options = options.resolved_for_doc(&doc);

    let policy = load_batch_policy(format, &doc, project_dir).map_err(MergeError::new)?;
    // Every row is validated and compiled under the merged config policy and brand,
    // the same governance `zenith validate` applies, whatever the output format.
    let layers = ConfigLayers::load(&NativeConfig, project_dir).map_err(MergeError::new)?;
    let row_policy = layers.policy(&doc.diagnostic_policy, &PolicyFlags::default());
    let row_brand = layers.brand(&doc.brand_contract);

    // ── 2. Collect data bindings ──────────────────────────────────────────
    let mut bindings: Vec<DataBinding> = Vec::new();
    let mut asset_bindings: Vec<AssetBinding> = Vec::new();
    for page in &doc.body.pages {
        collect_data_nodes(&page.children, &mut bindings, &mut asset_bindings)?;
    }
    if bindings.is_empty() && asset_bindings.is_empty() {
        return Err(MergeError::new(
            "error[merge.no_data_nodes]: no role=\"data.*\" template nodes found; tag a text or \
             image node with role=\"data.<column>\"",
        ));
    }

    // ── 3. Validate: asset bindings require a project_dir ────────────────
    if !asset_bindings.is_empty() && project_dir.is_none() {
        return Err(MergeError::new(
            "image data bindings require a project directory (the .zen file must be on disk)",
        ));
    }

    // ── 4. Parse CSV headers and validate bindings ────────────────────────
    let mut reader = csv::Reader::from_reader(csv_src.as_bytes());
    let headers = reader
        .headers()
        .map_err(|e| MergeError::new(format!("CSV header error: {}", e)))?
        .clone();

    // Build a header→index map (BTreeMap for deterministic ordering).
    let header_index: BTreeMap<String, usize> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| (h.to_owned(), i))
        .collect();

    // Verify all text-binding columns exist.
    let unknown: Vec<String> = bindings
        .iter()
        .filter(|b| !header_index.contains_key(&b.column))
        .map(|b| b.column.clone())
        .collect();
    if !unknown.is_empty() {
        return Err(MergeError::new(format!(
            "CSV column(s) not found in header: {}",
            unknown.join(", ")
        )));
    }

    // Verify all asset-binding columns exist.
    let unknown_asset: Vec<String> = asset_bindings
        .iter()
        .filter(|b| !header_index.contains_key(&b.column))
        .map(|b| b.column.clone())
        .collect();
    if !unknown_asset.is_empty() {
        return Err(MergeError::new(format!(
            "CSV column(s) not found in header: {}",
            unknown_asset.join(", ")
        )));
    }

    // Verify name_by column exists.
    if let Some(col) = name_by
        && !header_index.contains_key(col)
    {
        return Err(MergeError::new(format!(
            "--name-by column {:?} not found in CSV header",
            col
        )));
    }

    // Pre-resolve column indices (avoids per-cell BTreeMap lookups).
    // All columns were verified to exist above so `get` never returns None.
    let binding_indices: Vec<usize> = bindings
        .iter()
        .map(|b| -> Result<usize, MergeError> {
            header_index
                .get(&b.column)
                .copied()
                .ok_or_else(|| MergeError::new(format!("column {:?} not found", b.column)))
        })
        .collect::<Result<Vec<usize>, MergeError>>()?;

    let asset_binding_indices: Vec<usize> = asset_bindings
        .iter()
        .map(|b| -> Result<usize, MergeError> {
            header_index
                .get(&b.column)
                .copied()
                .ok_or_else(|| MergeError::new(format!("column {:?} not found", b.column)))
        })
        .collect::<Result<Vec<usize>, MergeError>>()?;

    let name_by_index: Option<usize> = match name_by {
        None => None,
        Some(col) => Some(
            header_index
                .get(col)
                .copied()
                .ok_or_else(|| MergeError::new(format!("--name-by column {:?} not found", col)))?,
        ),
    };

    // Row files must not replace the inputs, the manifest, or the combined file.
    let mut protected = targets.reserved.to_vec();
    protected.extend(targets.combine);
    let mut output_guard = crate::output_file::OutputGuard::new(&protected)
        .map_err(|error| MergeError::new(error.to_string()))?;
    // Check every planned page against reserved inputs and manifests before writes.
    let mut planned_reader = csv::Reader::from_reader(csv_src.as_bytes());
    for (row, record) in planned_reader.records().enumerate() {
        let Ok(record) = record else { continue };
        let stem = match name_by_index {
            Some(index) => sanitize_filename(record.get(index).unwrap_or("")),
            None => format!("row-{:04}", row + 1),
        };
        for name in row_filenames(&stem, doc.body.pages.len(), format) {
            output_guard
                .check(&out_dir.join(name))
                .map_err(|error| MergeError::new(error.to_string()))?;
        }
    }

    // ── 5. Build font + asset providers ONCE from the original doc ────────
    let fonts = build_font_provider(native::host(), &doc, project_dir, false)
        .map_err(|e| MergeError::new(e.message))?;
    // Template assets are loaded once; per-row image bytes are layered on top.
    let template_assets = match project_dir {
        Some(dir) => build_asset_provider(&NativeFs, &doc, dir, false)
            .map_err(|e| MergeError::new(e.message))?,
        None => BytesAssetProvider::new(),
    };
    let mut combiner = match (targets.combine, project_dir) {
        (None, _) => None,
        (Some(path), Some(dir)) => Some(Combiner::new(
            path,
            build_asset_provider(&NativeFs, &doc, dir, false)
                .map_err(|e| MergeError::new(e.message))?,
        )),
        (Some(path), None) => Some(Combiner::new(path, BytesAssetProvider::new())),
    };

    // ── 6. Ensure output directory exists ─────────────────────────────────
    std::fs::create_dir_all(out_dir).map_err(|e| {
        MergeError::new(format!(
            "could not create output directory '{}': {}",
            out_dir.display(),
            e
        ))
    })?;

    // ── 7. Iterate CSV rows ───────────────────────────────────────────────
    let mut rows: Vec<RowResult> = Vec::new();
    let mut used_names: BTreeSet<String> = BTreeSet::new();

    for (row_idx, record_result) in reader.records().enumerate() {
        let record = match record_result {
            Ok(r) => r,
            Err(e) => {
                push_failure(&mut rows, row_idx, None, format!("CSV read error: {}", e));
                continue;
            }
        };

        // Extract the row key once, as early as possible after the record is
        // obtained, so it is available to every failure path that follows.
        let row_key: Option<String> =
            name_by_index.map(|col_idx| record.get(col_idx).unwrap_or("").to_owned());

        // Build Transaction ops: ReplaceText ops first, then asset ops.
        let mut ops: Vec<Op> = bindings
            .iter()
            .zip(binding_indices.iter())
            .map(|(binding, &col_idx)| {
                let cell = record.get(col_idx).unwrap_or("");
                Op::ReplaceText {
                    node: binding.node_id.clone(),
                    spans: vec![OpSpan {
                        text: cell.to_owned(),
                        fill: None,
                        font_weight: None,
                        italic: None,
                        underline: None,
                        strikethrough: None,
                        vertical_align: None,
                        footnote_ref: None,
                    }],
                }
            })
            .collect();

        // Append AddAsset + SetAsset ops for non-empty image cells.
        for (binding, &col_idx) in asset_bindings.iter().zip(asset_binding_indices.iter()) {
            let cell = record.get(col_idx).unwrap_or("").trim();
            if cell.is_empty() {
                // Empty cell → leave template image in place; no op needed.
                continue;
            }
            let (asset_id, add_asset) = row_add_asset_op(row_idx, &binding.column, cell);
            ops.push(add_asset);
            ops.push(Op::SetAsset {
                node_id: binding.node_id.clone(),
                asset_id,
            });
        }

        let tx = Transaction {
            ops,
            permissions: Default::default(),
        };

        // Run transaction.
        let tx_result = match run_transaction(&doc, &tx) {
            Ok(r) => r,
            Err(e) => {
                push_failure(
                    &mut rows,
                    row_idx,
                    row_key,
                    format!("transaction engine error: {}", e.message),
                );
                continue;
            }
        };

        // A Rejected transaction is a per-row failure.
        if tx_result.status == TxStatus::Rejected {
            let msgs: Vec<String> = tx_result
                .diagnostics
                .iter()
                .map(|d| {
                    format!(
                        "{}[{}]: {}",
                        crate::json_types::severity_str(&d.severity),
                        d.code,
                        d.message
                    )
                })
                .collect();
            push_failure(
                &mut rows,
                row_idx,
                row_key,
                format!("transaction rejected: {}", msgs.join("; ")),
            );
            continue;
        }

        // Re-parse source_after → row document.
        let mut row_doc = match KdlAdapter.parse(tx_result.source_after.as_bytes()) {
            Ok(d) => d,
            Err(e) => {
                push_failure(
                    &mut rows,
                    row_idx,
                    row_key,
                    format!("post-transaction parse error: {}", e.message),
                );
                continue;
            }
        };

        // Resolve external text-file sources on the per-row document.
        // Any text.src_missing Error is a per-row failure (same gate as asset.missing).
        {
            let mut text_src_diags: Vec<zenith_core::Diagnostic> = Vec::new();
            resolve_text_sources(&NativeFs, &mut row_doc, project_dir, &mut text_src_diags);
            let hard: Vec<String> = text_src_diags
                .iter()
                .filter(|d| d.is_error())
                .map(zenith_pipeline::format_error_diag)
                .collect();
            if !hard.is_empty() {
                push_failure(
                    &mut rows,
                    row_idx,
                    row_key,
                    format!("text source error(s): {}", hard.join("; ")),
                );
                continue;
            }
        }

        // Validate the row document: contrast, safe zone, brand, and the rest.
        // An Error fails the row. Warnings and advice go to the row report.
        let mut row_validation =
            zenith_core::validate_with_policy(&row_doc, &row_policy, &row_brand).diagnostics;
        let validation_errors: Vec<String> = row_validation
            .iter()
            .filter(|d| d.is_error())
            .map(zenith_pipeline::format_error_diag)
            .collect();
        if !validation_errors.is_empty() {
            push_failure(
                &mut rows,
                row_idx,
                row_key,
                format!("validation error(s): {}", validation_errors.join("; ")),
            );
            if let Some(row) = rows.last_mut() {
                row.diagnostics = row_validation;
            }
            continue;
        }

        // Build per-row asset provider: template assets + row-specific images.
        // BytesAssetProvider is not Clone, so we rebuild from template doc and
        // then layer the row image(s) on top.
        let mut row_images: Vec<(String, Arc<[u8]>)> = Vec::new();
        let row_assets = if asset_bindings.is_empty() {
            // No image bindings: the pre-built `template_assets` provider is used
            // directly by the render call below — no per-row provider is needed.
            None
        } else {
            let Some(dir) = project_dir else {
                push_failure(
                    &mut rows,
                    row_idx,
                    row_key,
                    "internal: project directory unexpectedly missing".to_owned(),
                );
                continue;
            };
            // Start with template assets.
            let mut row_provider = build_asset_provider(&NativeFs, &doc, dir, false)
                .map_err(|e| MergeError::new(e.message))?;
            // Layer in per-row images. The combined file reuses their bytes.
            let mut row_asset_missing = false;
            for (binding, &col_idx) in asset_bindings.iter().zip(asset_binding_indices.iter()) {
                let cell = record.get(col_idx).unwrap_or("").trim();
                if cell.is_empty() {
                    continue;
                }
                let asset_id = row_asset_id(row_idx, &binding.column);
                let img_path = dir.join(cell);
                match std::fs::read(&img_path) {
                    Ok(bytes) => {
                        let bytes: Arc<[u8]> = bytes.into();
                        row_provider.register(&asset_id, AssetKind::Image, Arc::clone(&bytes));
                        row_images.push((asset_id, bytes));
                    }
                    Err(e) => {
                        push_failure(
                            &mut rows,
                            row_idx,
                            row_key.clone(),
                            format!(
                                "error[asset.missing]: asset '{}' file not found: '{}': {}",
                                asset_id,
                                img_path.display(),
                                e
                            ),
                        );
                        row_asset_missing = true;
                        break;
                    }
                }
            }
            if row_asset_missing {
                continue;
            }
            Some(row_provider)
        };

        // Also gate on collect_missing_asset_diagnostics for any declared-but-missing
        // template assets now embedded in row_doc (includes the AddAsset entries).
        if let Some(dir) = project_dir {
            let missing_diags = collect_missing_asset_diagnostics(&NativeFs, &row_doc, dir);
            let hard: Vec<String> = missing_diags
                .iter()
                .filter(|d| d.is_error())
                .map(zenith_pipeline::format_error_diag)
                .collect();
            if !hard.is_empty() {
                push_failure(
                    &mut rows,
                    row_idx,
                    row_key,
                    format!("asset error(s): {}", hard.join("; ")),
                );
                continue;
            }
        }

        // Determine page count; a row document with no pages is a hard failure.
        let page_count = row_doc.body.pages.len();
        if page_count == 0 {
            push_failure(
                &mut rows,
                row_idx,
                row_key,
                "row document has no pages".to_owned(),
            );
            continue;
        }

        // Derive the row stem once (hoist name logic before per-page work).
        let row_stem = match name_by_index {
            Some(col_idx) => sanitize_filename(record.get(col_idx).unwrap_or("")),
            None => format!("row-{:04}", row_idx + 1),
        };

        // Build all output filenames for this row upfront.
        let page_filenames = row_filenames(&row_stem, page_count, format);

        let planned_paths: Vec<_> = page_filenames
            .iter()
            .map(|name| out_dir.join(name))
            .collect();
        if let Err(error) =
            crate::output_file::check_distinct(planned_paths.iter().map(|path| path.as_path()))
        {
            push_failure(&mut rows, row_idx, row_key, error.to_string());
            continue;
        }

        // Pre-flight collision check: if ANY filename is already taken, fail
        // the whole row without writing anything.
        let mut collided = false;
        for fname in &page_filenames {
            if used_names.contains(fname) {
                push_failure(
                    &mut rows,
                    row_idx,
                    row_key.clone(),
                    format!("output filename collision: {fname}"),
                );
                collided = true;
                break;
            }
        }
        if collided {
            continue;
        }

        // Compile and render every page; collect failures.
        let mut page_failures: Vec<String> = Vec::new();
        let mut diagnostics = std::mem::take(&mut row_validation);
        let mut page_images: Vec<(String, Vec<u8>)> = Vec::new();
        // A per-row format encodes every page of the row in one file.
        let mut row_scenes = Vec::new();

        let row_provider = row_assets.as_ref().unwrap_or(&template_assets);
        let prep = DocumentPrep::new(&row_doc, None, None).with_image_sizes(image_sizes(
            &row_doc,
            None,
            row_provider,
        ));
        let compiler = PageCompiler::new(&prep, &fonts);
        for page_index in 0..page_count {
            let mut compile_result = compiler.compile_page(page_index);
            compile_result.diagnostics = block_row_defects(
                zenith_core::apply_policy(compile_result.diagnostics, &row_policy),
                &row_policy,
            );

            // Block on Error-severity compile diagnostics.
            let hard_diags: Vec<String> = compile_result
                .diagnostics
                .iter()
                .filter(|d| d.is_error())
                .map(zenith_pipeline::format_error_diag)
                .collect();
            diagnostics.extend(std::mem::take(&mut compile_result.diagnostics));
            if !hard_diags.is_empty() {
                page_failures.push(format!(
                    "page {}: compile error(s): {}",
                    page_index + 1,
                    hard_diags.join("; ")
                ));
                continue;
            }

            let page_fname = match format.layout() {
                BatchLayout::PerPage => page_filenames.get(page_index),
                BatchLayout::PerRow => {
                    row_scenes.push(compile_result.scene);
                    continue;
                }
            };
            let Some(page_fname) = page_fname else {
                page_failures.push(format!("page {}: no output name", page_index + 1));
                continue;
            };
            // Encode the scene with row-scoped assets.
            let image_result = encode_batch_scene(
                options,
                &compile_result.scene,
                &fonts,
                row_provider,
                page_index + 1,
                policy.as_ref(),
            );
            match image_result {
                Ok((bytes, page_diagnostics)) => {
                    diagnostics.extend(page_diagnostics);
                    page_images.push((page_fname.clone(), bytes));
                }
                Err(e) => {
                    page_failures.push(format!(
                        "page {}: {}",
                        page_index + 1,
                        e.iter()
                            .map(|d| d.message.as_str())
                            .collect::<Vec<_>>()
                            .join("; ")
                    ));
                    if format.is_vector() {
                        diagnostics.extend(e);
                    }
                }
            }
        }
        if page_failures.is_empty()
            && let (BatchLayout::PerRow, Some(row_fname)) =
                (format.layout(), page_filenames.first())
        {
            let pages: Vec<usize> = (1..=page_count).collect();
            match encode_batch_pdf(
                options,
                &row_scenes,
                &pages,
                &fonts,
                row_provider,
                policy.as_ref(),
            ) {
                Ok((bytes, row_diagnostics)) => {
                    diagnostics.extend(row_diagnostics);
                    page_images.push((row_fname.clone(), bytes));
                }
                Err(e) => {
                    page_failures.push(
                        e.iter()
                            .map(|d| d.message.as_str())
                            .collect::<Vec<_>>()
                            .join("; "),
                    );
                    diagnostics.extend(e);
                }
            }
        }

        // If any page failed, the whole row fails atomically — write nothing.
        if !page_failures.is_empty() {
            push_failure(&mut rows, row_idx, row_key, page_failures.join("; "));
            if let Some(row) = rows.last_mut() {
                row.diagnostics = diagnostics;
            }
            continue;
        }

        // Commit pages in order and reserve each written name immediately.
        let mut write_failed = false;
        let mut newly_written: Vec<String> = Vec::new();
        for (fname, bytes) in page_images {
            let out_path = out_dir.join(&fname);
            if let Err(e) = output_guard.write(&out_path, &bytes) {
                push_failure(
                    &mut rows,
                    row_idx,
                    row_key.clone(),
                    format!("write error '{}': {}", out_path.display(), e),
                );
                if let Some(row) = rows.last_mut() {
                    row.diagnostics = std::mem::take(&mut diagnostics);
                    row.outputs = std::mem::take(&mut newly_written);
                }
                write_failed = true;
                break;
            }
            used_names.insert(fname.clone());
            newly_written.push(fname);
        }
        if write_failed {
            continue;
        }
        if let Some(combiner) = &mut combiner {
            combiner.add(row_idx, row_scenes, row_images);
        }
        rows.push(RowResult {
            row: row_idx,
            key: row_key,
            outputs: newly_written,
            diagnostics: Diagnostic::dedup(diagnostics),
            failure: None,
        });
    }

    let combined =
        combiner.map(|combiner| combiner.finish(options, &fonts, targets.reserved, &rows));
    Ok(MergeReport { rows, combined })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Push a failure [`RowResult`] and prepare for `continue`.
///
/// Extracted to remove the 8-way repetition of the identical struct literal
/// inside the per-row loop body.
fn push_failure(rows: &mut Vec<RowResult>, row: usize, key: Option<String>, reason: String) {
    rows.push(RowResult {
        row,
        key,
        outputs: Vec::new(),
        failure: Some(reason),
        diagnostics: Vec::new(),
    });
}

/// Canonical asset-id used for per-row image bindings.
///
/// Must match between the op-building pass (AddAsset/SetAsset) and the
/// asset-loading pass (register into row_provider) — keeping it here
/// ensures they can never diverge.
fn row_asset_id(row_idx: usize, column: &str) -> String {
    format!("merge.row.{}.asset.{}", row_idx, column)
}

fn row_add_asset_op(row_idx: usize, column: &str, src: &str) -> (String, Op) {
    let asset_id = row_asset_id(row_idx, column);
    let op = Op::AddAsset {
        id: asset_id.clone(),
        kind: "image".to_owned(),
        src: src.to_owned(),
        sha256: None,
        metadata: Box::new(AddAssetMetadata::default()),
    };
    (asset_id, op)
}
