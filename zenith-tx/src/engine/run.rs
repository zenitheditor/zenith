//! Public entry point: [`run_transaction`] and result finalization.
//!
//! This module is pure: it performs no file I/O and does not mutate the input
//! document (it works on a clone). Dry-run vs. apply is the caller's concern.

use zenith_core::{Diagnostic, Document, KdlAdapter, KdlSource, Severity, validate};

use super::dispatch::apply_op;
use super::lock::{node_is_locked, op_lock_targets};
use crate::op::Transaction;
use crate::result::{TxError, TxResult, TxStatus};

/// Apply `tx` to `doc` and return a structured [`TxResult`].
///
/// The function is **pure**: `doc` is never mutated (a clone is used for the
/// candidate), and no I/O is performed. Both dry-run and apply callers receive
/// the same result shape; the caller decides whether to persist `source_after`.
pub fn run_transaction(doc: &Document, tx: &Transaction) -> Result<TxResult, TxError> {
    // 1. Format the original document → source_before.
    let source_before = format_source(doc, "source_before")?;

    // 2. Clone the document into a mutable candidate.
    let mut candidate = doc.clone();

    // 3. Apply each op in order, collecting diagnostics and affected ids.
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut affected: Vec<String> = Vec::new(); // insertion-order, de-duplicated

    for op in &tx.ops {
        // Lock pre-check: a guarded op against a locked node is rejected unless
        // the transaction carries `permissions.allow_locked`. The check reads the
        // *candidate* state, so a `set_locked` earlier in the same transaction
        // locks the node for later ops (and `set_locked` itself is exempt, so a
        // node can always be unlocked). Targets are visited in order for
        // determinism; if any target is locked the whole op is skipped, leaving
        // the emitted `node.locked` error to reject the transaction in step 5.
        if !tx.permissions.allow_locked {
            let mut locked_hit = false;
            for target in op_lock_targets(op) {
                if node_is_locked(&candidate, target) {
                    locked_hit = true;
                    diagnostics.push(Diagnostic::error(
                        "node.locked",
                        format!(
                            "node '{}' is locked; unlock it or set \
                             permissions.allow_locked to edit it",
                            target
                        ),
                        None,
                        Some(target.to_owned()),
                    ));
                }
            }
            if locked_hit {
                continue;
            }
        }

        apply_op(op, &mut candidate, &mut diagnostics, &mut affected);
    }

    // 4. Post-apply validation and result finalization.
    finish_candidate(source_before, candidate, diagnostics, affected)
}

pub(super) fn format_source(doc: &Document, label: &str) -> Result<String, TxError> {
    let bytes = KdlAdapter.format(doc).map_err(|e| TxError {
        message: format!("failed to format {label} document: {e}"),
    })?;
    String::from_utf8(bytes).map_err(|e| TxError {
        message: format!("{label} is not valid UTF-8: {e}"),
    })
}

pub(super) fn finish_candidate(
    source_before: String,
    candidate: Document,
    mut diagnostics: Vec<Diagnostic>,
    affected: Vec<String>,
) -> Result<TxResult, TxError> {
    let report = validate(&candidate);
    diagnostics.extend(report.diagnostics);

    let has_errors = Diagnostic::has_errors(&diagnostics);
    let has_warnings = diagnostics.iter().any(|d| d.severity == Severity::Warning);

    let (status, source_after) = if has_errors {
        (TxStatus::Rejected, source_before.clone())
    } else {
        let after = format_source(&candidate, "source_after")?;
        let status = if has_warnings {
            TxStatus::AcceptedWithWarnings
        } else {
            TxStatus::Accepted
        };
        (status, after)
    };

    Ok(TxResult {
        status,
        diagnostics,
        source_before,
        source_after,
        affected_node_ids: affected,
    })
}
