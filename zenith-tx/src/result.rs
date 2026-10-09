//! Transaction result types: [`TxResult`], [`TxStatus`], and [`TxError`].

use std::fmt;

use zenith_core::{Diagnostic, Document};

/// The outcome status of a transaction run.
#[derive(Debug, Clone, PartialEq)]
pub enum TxStatus {
    /// All ops applied and no diagnostics at Error or Warning severity.
    Accepted,
    /// All ops applied but at least one Warning-severity diagnostic was
    /// produced (by op application or post-apply validation).
    AcceptedWithWarnings,
    /// At least one Error-severity diagnostic was produced; the document was
    /// not changed (`source_after == source_before`).
    Rejected,
}

/// The complete result of a transaction run, returned by both dry-run and apply.
///
/// Dry-run vs. apply is the *caller's* concern (whether to persist
/// `source_after` to disk). Both paths through `run_transaction` return this
/// identical shape.
#[derive(Debug, Clone, PartialEq)]
pub struct TxResult {
    /// Overall outcome of the transaction.
    pub status: TxStatus,
    /// All diagnostics: from op-application and from post-apply validation.
    pub diagnostics: Vec<Diagnostic>,
    /// Canonical-formatted source of the *original* document (before any ops).
    pub source_before: String,
    /// Canonical-formatted source of the *result* document.
    ///
    /// Equal to `source_before` when `status == Rejected`.
    pub source_after: String,
    /// The result document. Equal to the input document when
    /// `status == Rejected`.
    ///
    /// With the input document it lets a caller patch the user's own source
    /// text (`zenith_core::patch_source`) instead of writing `source_after`.
    pub document_after: Document,
    /// Node ids touched by successfully-applied ops, in first-seen application
    /// order, de-duplicated (no HashMap — stable insertion-order `Vec` with a
    /// linear membership check keeps the output deterministic).
    pub affected_node_ids: Vec<String>,
}

// ── TxError ───────────────────────────────────────────────────────────────────

/// An error that prevents a transaction result from being produced at all.
///
/// Distinct from a rejected transaction (which does produce a [`TxResult`]).
/// Only returned for envelope-parse failures or formatter failures that make
/// it impossible to produce `source_before` / `source_after`.
#[derive(Debug, Clone, PartialEq)]
pub struct TxError {
    pub message: String,
}

impl fmt::Display for TxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "zenith-tx error: {}", self.message)
    }
}

impl std::error::Error for TxError {}
