//! Pure logic for `zenith merge`.
//!
//! The public entry point [`run`] operates entirely on in-memory source text
//! plus filesystem paths for outputs.  The source `.zen` file is NEVER
//! mutated; each row's document is produced in-memory via the transaction
//! engine and re-parsed before compilation.

use std::collections::BTreeSet;
use std::path::Path;

use zenith_core::{KdlAdapter, KdlSource, Severity};
use zenith_render::render_png;
use zenith_scene::compile_page;
use zenith_tx::{Op, OpSpan, Transaction, TxStatus, run_transaction};

use crate::commands::render::{build_asset_provider, build_font_provider};

// ── Error type ────────────────────────────────────────────────────────────────

/// A fatal error that prevents the merge from starting.
///
/// Exit code 2 for all setup/template errors (consistent with the other
/// commands whose `RenderCmdErr`/`FmtErr`/`TxCmdErr` all use 2 for this class
/// of failure).
#[derive(Debug)]
pub struct MergeError {
    /// Human-readable message.
    pub message: String,
    /// Recommended exit code (always 2 for template/setup errors).
    pub exit_code: u8,
}

impl MergeError {
    fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            exit_code: 2,
        }
    }
}

// ── Report types ──────────────────────────────────────────────────────────────

/// One per-row failure.
#[derive(Debug)]
pub struct RowFailure {
    /// 0-based row index in the CSV (header row not counted).
    pub row: usize,
    /// Human-readable reason.
    pub reason: String,
}

/// Summary of a completed merge run.
#[derive(Debug)]
pub struct MergeReport {
    /// Filenames (not full paths) of PNGs successfully written, in row order.
    pub written: Vec<String>,
    /// Rows that were skipped due to per-row errors.
    pub failed: Vec<RowFailure>,
}

// ── Internal binding type ─────────────────────────────────────────────────────

/// Maps a node id to the CSV column that supplies its replacement text.
struct DataBinding {
    node_id: String,
    column: String,
}

// ── collect_data_nodes ────────────────────────────────────────────────────────

/// Return an error if `role` starts with `"data."` on a non-text node.
///
/// The error message and format live here exactly once.
fn reject_data_role_on_non_text(role: Option<&str>, id: &str) -> Result<(), MergeError> {
    if let Some(role) = role
        && role.starts_with("data.")
    {
        return Err(MergeError::new(format!(
            "role=\"{}\" on non-text node {}: replace_text supports text nodes only",
            role, id
        )));
    }
    Ok(())
}

/// Walk `nodes` recursively and collect every node that carries a
/// `role="data.<column>"` attribute.
///
/// Only `Node::Text` is allowed to carry a `data.*` role.  Any other variant
/// with such a role is a hard [`MergeError`].
///
/// Recurses into `Node::Frame` and `Node::Group` children.
fn collect_data_nodes(
    nodes: &[zenith_core::Node],
    out: &mut Vec<DataBinding>,
) -> Result<(), MergeError> {
    for node in nodes {
        match node {
            zenith_core::Node::Text(n) => {
                if let Some(role) = n.role.as_deref()
                    && let Some(col) = role.strip_prefix("data.")
                {
                    out.push(DataBinding {
                        node_id: n.id.clone(),
                        column: col.to_owned(),
                    });
                }
            }
            zenith_core::Node::Rect(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Ellipse(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Line(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Code(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Frame(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
                collect_data_nodes(&n.children, out)?;
            }
            zenith_core::Node::Group(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
                collect_data_nodes(&n.children, out)?;
            }
            zenith_core::Node::Image(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Polygon(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Polyline(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Instance(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Field(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Footnote(n) => {
                reject_data_role_on_non_text(n.role.as_deref(), &n.id)?;
            }
            zenith_core::Node::Unknown(_n) => {
                // UnknownNode has no id or role field; data.* roles cannot be
                // placed on unknown nodes (the parser would not parse them).
            }
        }
    }
    Ok(())
}

// ── sanitize_filename ─────────────────────────────────────────────────────────

/// Map filesystem-unsafe characters and NUL to `_`, trim leading/trailing
/// dots and whitespace, and return `"_"` for the empty result.
pub fn sanitize_filename(s: &str) -> String {
    let mapped: String = s
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            other => other,
        })
        .collect();
    let trimmed = mapped.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if trimmed.is_empty() {
        "_".to_owned()
    } else {
        trimmed.to_owned()
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
    // ── 1. Parse the template document (once) ─────────────────────────────
    let doc = KdlAdapter
        .parse(doc_src.as_bytes())
        .map_err(|e| MergeError::new(format!("error[parse.error]: {}", e.message)))?;

    // ── 2. Collect data bindings ──────────────────────────────────────────
    let mut bindings: Vec<DataBinding> = Vec::new();
    for page in &doc.body.pages {
        collect_data_nodes(&page.children, &mut bindings)?;
    }
    if bindings.is_empty() {
        return Err(MergeError::new("no role=\"data.*\" template nodes found"));
    }

    // ── 3. Parse CSV headers and validate bindings ────────────────────────
    let mut reader = csv::Reader::from_reader(csv_src.as_bytes());
    let headers = reader
        .headers()
        .map_err(|e| MergeError::new(format!("CSV header error: {}", e)))?
        .clone();

    // Build a header→index map.
    let header_index: std::collections::HashMap<String, usize> = headers
        .iter()
        .enumerate()
        .map(|(i, h)| (h.to_owned(), i))
        .collect();

    // Verify all binding columns exist.
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

    // Verify name_by column exists.
    if let Some(col) = name_by
        && !header_index.contains_key(col)
    {
        return Err(MergeError::new(format!(
            "--name-by column {:?} not found in CSV header",
            col
        )));
    }

    // Pre-resolve column indices (avoids per-cell HashMap lookups).
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

    let name_by_index: Option<usize> = match name_by {
        None => None,
        Some(col) => Some(
            header_index
                .get(col)
                .copied()
                .ok_or_else(|| MergeError::new(format!("--name-by column {:?} not found", col)))?,
        ),
    };

    // ── 4. Build font + asset providers ONCE from the original doc ────────
    let fonts =
        build_font_provider(&doc, project_dir, false).map_err(|e| MergeError::new(e.message))?;
    let assets = match project_dir {
        Some(dir) => {
            build_asset_provider(&doc, dir, false).map_err(|e| MergeError::new(e.message))?
        }
        None => zenith_core::BytesAssetProvider::new(),
    };

    // ── 5. Ensure output directory exists ─────────────────────────────────
    std::fs::create_dir_all(out_dir).map_err(|e| {
        MergeError::new(format!(
            "could not create output directory '{}': {}",
            out_dir.display(),
            e
        ))
    })?;

    // ── 6. Iterate CSV rows ───────────────────────────────────────────────
    let mut written: Vec<String> = Vec::new();
    let mut failed: Vec<RowFailure> = Vec::new();
    let mut used_names: BTreeSet<String> = BTreeSet::new();

    for (row_idx, record_result) in reader.records().enumerate() {
        let record = match record_result {
            Ok(r) => r,
            Err(e) => {
                failed.push(RowFailure {
                    row: row_idx,
                    reason: format!("CSV read error: {}", e),
                });
                continue;
            }
        };

        // Build Transaction: one ReplaceText op per binding.
        let ops: Vec<Op> = bindings
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

        let tx = Transaction {
            ops,
            permissions: Default::default(),
        };

        // Run transaction.
        let tx_result = match run_transaction(&doc, &tx) {
            Ok(r) => r,
            Err(e) => {
                failed.push(RowFailure {
                    row: row_idx,
                    reason: format!("transaction engine error: {}", e.message),
                });
                continue;
            }
        };

        // A Rejected transaction is a per-row failure.
        if tx_result.status == TxStatus::Rejected {
            let msgs: Vec<String> = tx_result
                .diagnostics
                .iter()
                .map(|d| format!("{}[{}]: {}", severity_label(&d.severity), d.code, d.message))
                .collect();
            failed.push(RowFailure {
                row: row_idx,
                reason: format!("transaction rejected: {}", msgs.join("; ")),
            });
            continue;
        }

        // Re-parse source_after → row document.
        let row_doc = match KdlAdapter.parse(tx_result.source_after.as_bytes()) {
            Ok(d) => d,
            Err(e) => {
                failed.push(RowFailure {
                    row: row_idx,
                    reason: format!("post-transaction parse error: {}", e.message),
                });
                continue;
            }
        };

        // Compile page 0.
        let compile_result = compile_page(&row_doc, &fonts, 0);

        // Block on Error-severity compile diagnostics (e.g. text.fit_failed).
        let hard_diags: Vec<String> = compile_result
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| format!("error[{}]: {}", d.code, d.message))
            .collect();
        if !hard_diags.is_empty() {
            failed.push(RowFailure {
                row: row_idx,
                reason: format!("compile error(s): {}", hard_diags.join("; ")),
            });
            continue;
        }

        // Determine output filename.
        let filename = match name_by_index {
            Some(col_idx) => {
                let cell = record.get(col_idx).unwrap_or("");
                format!("{}.png", sanitize_filename(cell))
            }
            None => format!("row-{:04}.png", row_idx + 1),
        };

        // Collision check.
        if used_names.contains(&filename) {
            failed.push(RowFailure {
                row: row_idx,
                reason: format!("output filename collision: {}", filename),
            });
            continue;
        }
        used_names.insert(filename.clone());

        // Render to PNG bytes.
        let png_bytes = match render_png(&compile_result.scene, &fonts, &assets) {
            Ok(b) => b,
            Err(e) => {
                failed.push(RowFailure {
                    row: row_idx,
                    reason: format!("render error: {}", e),
                });
                continue;
            }
        };

        // Write immediately (stream — never accumulate all PNGs in memory).
        let out_path = out_dir.join(&filename);
        if let Err(e) = std::fs::write(&out_path, &png_bytes) {
            failed.push(RowFailure {
                row: row_idx,
                reason: format!("write error '{}': {}", out_path.display(), e),
            });
            continue;
        }

        written.push(filename);
    }

    Ok(MergeReport { written, failed })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn severity_label(sev: &Severity) -> &'static str {
    match sev {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Advisory => "advisory",
    }
}
