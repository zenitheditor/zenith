//! Local document-history recording (best-effort attach hook).
//!
//! Wraps the `zenith-session` subsystem with the real OS adapters and the
//! resolved data directory. Every successful write-through edit (`tx --apply`,
//! `library add`) calls [`record_edit`]: it reconciles the document's identity
//! (minting and stamping a `doc-id` on first edit) and records the new state as
//! a Tier-1 session snapshot and a Tier-2 version. Recording is best-effort:
//! failures become a warning message on `Recorded::warning` — they never block
//! the edit.

use std::path::Path;

use zenith_core::{KdlAdapter, KdlSource as _};
use zenith_session::adapter::{OsClock, OsFs, OsRng};
use zenith_session::{
    Outcome, StorePaths, reconcile, record_state, record_version, resolve_data_dir,
};

// ── Public result type ────────────────────────────────────────────────────────

/// The result of a best-effort history recording.
///
/// `bytes` are ALWAYS the bytes that should be written to disk — identical to
/// the input unless a fresh `doc-id` was minted or forked, in which case they
/// carry the stamped id. `warning` carries a human-readable description of any
/// non-fatal failure that occurred during recording.
pub struct Recorded {
    /// Bytes to write to the `.zen` file (may have a stamped `doc-id`).
    pub bytes: Vec<u8>,
    /// Non-fatal warning produced during history recording, if any.
    pub warning: Option<String>,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Resolve the data directory and record `content` (the about-to-be-written
/// `.zen` bytes) as history for the document at `doc_path`.
///
/// Returns a [`Recorded`] value: `bytes` are always the correct bytes to write
/// (possibly with a freshly minted `doc-id` stamped in), and `warning` carries
/// any non-fatal error message. This function never panics and never returns an
/// error — all failures are surfaced as `warning: Some(...)`.
pub fn record_edit(content: &[u8], doc_path: &Path, op_kind: &str) -> Recorded {
    let paths = match resolve_data_dir() {
        Ok(data_dir) => StorePaths::new(data_dir),
        Err(e) => {
            return Recorded {
                bytes: content.to_vec(),
                warning: Some(format!("history: resolve_data_dir failed: {}", e.message)),
            };
        }
    };
    record_edit_in(&paths, content, doc_path, op_kind)
}

/// Same as [`record_edit`] but with an explicit store root (used by tests).
///
/// Returns a [`Recorded`] value: `bytes` are always the correct bytes to write,
/// and `warning` carries any non-fatal error message. Never panics.
pub fn record_edit_in(
    paths: &StorePaths,
    content: &[u8],
    doc_path: &Path,
    op_kind: &str,
) -> Recorded {
    let fs = OsFs;
    let clock = OsClock;
    let rng = OsRng;

    // Parse to read the embedded doc-id (if any).
    let mut doc = match KdlAdapter.parse(content) {
        Ok(d) => d,
        Err(e) => {
            return Recorded {
                bytes: content.to_vec(),
                warning: Some(format!("history: parse failed: {}", e.message)),
            };
        }
    };

    let reconciled = match reconcile(&fs, paths, &clock, &rng, doc.doc_id.as_deref(), doc_path) {
        Ok(r) => r,
        Err(e) => {
            return Recorded {
                bytes: content.to_vec(),
                warning: Some(format!("history: reconcile failed: {}", e.message)),
            };
        }
    };

    // If a new id was minted or forked, stamp it into the document and re-format
    // so the written file carries the identity.
    let final_bytes: Vec<u8> = match reconciled.outcome {
        Outcome::Minted | Outcome::Copied { .. } => {
            doc.doc_id = Some(reconciled.doc_id.clone());
            match KdlAdapter.format(&doc) {
                Ok(b) => b,
                Err(e) => {
                    return Recorded {
                        bytes: content.to_vec(),
                        warning: Some(format!("history: format failed: {}", e.message)),
                    };
                }
            }
        }
        Outcome::Matched | Outcome::Moved { .. } | Outcome::Adopted => content.to_vec(),
    };

    // Record Tier-1 session snapshot. Best-effort: on failure, return the
    // (possibly stamped) bytes with a warning so the write still proceeds.
    if let Err(e) = record_state(
        &fs,
        paths,
        &clock,
        &rng,
        &reconciled.doc_id,
        &final_bytes,
        Some(op_kind),
    ) {
        return Recorded {
            bytes: final_bytes,
            warning: Some(format!(
                "history: record_state failed: {} (file will still be written)",
                e.message
            )),
        };
    }

    // Record Tier-2 durable version. Best-effort for the same reason.
    if let Err(e) = record_version(
        &fs,
        paths,
        &clock,
        &reconciled.doc_id,
        &final_bytes,
        None,
        Some(op_kind),
    ) {
        return Recorded {
            bytes: final_bytes,
            warning: Some(format!(
                "history: record_version failed: {} (file will still be written)",
                e.message
            )),
        };
    }

    Recorded {
        bytes: final_bytes,
        warning: None,
    }
}
