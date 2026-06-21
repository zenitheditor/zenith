//! Tier-1 ephemeral session: a content-addressed snapshot DAG with a HEAD
//! pointer and a redo stack, persisted under `session_dir(doc_id)`.
//!
//! Records are STATE snapshots (full content), never operation logs — so an
//! external change (git checkout, hand-edit) is captured as a normal snapshot
//! and is therefore undoable. Undo/redo (next unit) are pure pointer moves over
//! this DAG; this unit provides the state file plus state-recording and
//! current-content readback.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use crate::adapter::{Clock, Fs, Rng};
use crate::error::SessionError;
use crate::layout::StorePaths;
use crate::manifest::{HistoryRecord, append_record, read_records};
use crate::store::{get_object, object_hash, put_object_with_hash};

// ── Persisted pointer state ────────────────────────────────────────────────────

/// The mutable pointer state for a session: HEAD and the redo stack.
/// Persisted to `state.json` inside `session_dir(doc_id)`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SessionState {
    /// Current record id (HEAD). None = empty session (no states yet).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    /// Record ids available for redo, most-recently-undone LAST (stack top = end).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub redo: Vec<String>,
}

// ── Outcome ────────────────────────────────────────────────────────────────────

/// The outcome of a [`record_state`] call.
#[derive(Debug, Clone, PartialEq)]
pub enum RecordOutcome {
    /// Content was byte-identical to HEAD's snapshot; no new record was created.
    Unchanged,
    /// A new state was recorded; HEAD advanced to this record id.
    Recorded { id: String },
}

// ── Private path helpers ───────────────────────────────────────────────────────

fn state_path(paths: &StorePaths, doc_id: &str) -> PathBuf {
    paths.session_dir(doc_id).join("state.json")
}

pub(crate) fn journal_path(paths: &StorePaths, doc_id: &str) -> PathBuf {
    paths.session_dir(doc_id).join("journal.jsonl")
}

// ── State load / save ──────────────────────────────────────────────────────────

pub(crate) fn load_state(
    fs: &impl Fs,
    paths: &StorePaths,
    doc_id: &str,
) -> Result<SessionState, SessionError> {
    let p = state_path(paths, doc_id);
    if !fs.exists(&p) {
        return Ok(SessionState::default());
    }
    let bytes = fs.read(&p)?;
    serde_json::from_slice(&bytes)
        .map_err(|e| SessionError::new(format!("parse session state: {e}")))
}

pub(crate) fn save_state(
    fs: &impl Fs,
    paths: &StorePaths,
    doc_id: &str,
    state: &SessionState,
) -> Result<(), SessionError> {
    let p = state_path(paths, doc_id);
    fs.create_dir_all(&paths.session_dir(doc_id))?;
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|e| SessionError::new(format!("serialize session state: {e}")))?;
    fs.write(&p, &bytes)
}

// ── Record lookup helper ───────────────────────────────────────────────────────

/// Linear search for a record by id. Returns `None` if not found.
pub(crate) fn find_record<'a>(records: &'a [HistoryRecord], id: &str) -> Option<&'a HistoryRecord> {
    records.iter().find(|r| r.id == id)
}

// ── Public API ─────────────────────────────────────────────────────────────────

/// Record the current `content` as a new state snapshot for `doc_id`.
///
/// `op_kind` is an optional label (e.g. "edit", "external") stored on the
/// record; it does NOT affect undo/redo. Returns `Unchanged` if `content` is
/// byte-identical to the current HEAD snapshot (dedup), else `Recorded { id }`
/// with HEAD advanced and the redo stack cleared.
pub fn record_state(
    fs: &impl Fs,
    paths: &StorePaths,
    clock: &impl Clock,
    _rng: &impl Rng,
    doc_id: &str,
    content: &[u8],
    op_kind: Option<&str>,
) -> Result<RecordOutcome, SessionError> {
    let mut state = load_state(fs, paths, doc_id)?;
    let jpath = journal_path(paths, doc_id);
    let records = read_records(fs, &jpath)?;

    // Dedup against the CURRENT head snapshot.
    let new_hash = object_hash(content);
    if let Some(head_id) = &state.head
        && let Some(head_rec) = find_record(&records, head_id)
        && head_rec.snapshot == new_hash
    {
        return Ok(RecordOutcome::Unchanged);
    }

    // Store the object (dedups bytes) and append a new record. The address is
    // the hash we already computed for the dedup check above.
    put_object_with_hash(fs, paths, doc_id, content, &new_hash)?;
    let seq = u64::try_from(records.len())
        .map_err(|_| SessionError::new("session record count exceeds u64"))?;
    let id = format!("r{seq}");
    let mut rec = HistoryRecord::new(id.clone(), seq, state.head.clone(), new_hash);
    rec.op_kind = op_kind.map(str::to_owned);
    rec.timestamp_ms = clock
        .now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis());
    append_record(fs, &jpath, &rec)?;

    state.head = Some(id.clone());
    state.redo.clear();
    save_state(fs, paths, doc_id, &state)?;
    Ok(RecordOutcome::Recorded { id })
}

/// The full content of the current HEAD snapshot, or `None` if the session is
/// empty (no states recorded yet).
pub fn current_content(
    fs: &impl Fs,
    paths: &StorePaths,
    doc_id: &str,
) -> Result<Option<Vec<u8>>, SessionError> {
    let state = load_state(fs, paths, doc_id)?;
    let head_id = match state.head {
        Some(h) => h,
        None => return Ok(None),
    };
    let records = read_records(fs, &journal_path(paths, doc_id))?;
    let rec = find_record(&records, &head_id).ok_or_else(|| {
        SessionError::new(format!("session HEAD points to unknown record: {head_id}"))
    })?;
    let content = get_object(fs, paths, doc_id, &rec.snapshot)?;
    Ok(Some(content))
}

/// Undo: move HEAD to its parent snapshot. Returns the content now at HEAD, or
/// `None` if the session is empty or already at the root (nothing to undo). The
/// record we left is pushed onto the redo stack so [`redo`] can return to it.
pub fn undo(
    fs: &impl Fs,
    paths: &StorePaths,
    doc_id: &str,
) -> Result<Option<Vec<u8>>, SessionError> {
    let mut state = load_state(fs, paths, doc_id)?;
    let head_id = match state.head.as_deref() {
        Some(h) => h,
        None => return Ok(None),
    };
    let records = read_records(fs, &journal_path(paths, doc_id))?;
    let rec = find_record(&records, head_id).ok_or_else(|| {
        SessionError::new(format!("session HEAD points to unknown record: {head_id}"))
    })?;
    let parent = match rec.parent.clone() {
        Some(p) => p,
        None => return Ok(None), // at root; nothing to undo, HEAD unchanged
    };
    state.redo.push(head_id.to_owned());
    state.head = Some(parent);
    save_state(fs, paths, doc_id, &state)?;
    current_content(fs, paths, doc_id)
}

/// Redo: move HEAD forward to the most-recently-undone snapshot. Returns the
/// content now at HEAD, or `None` if the redo stack is empty.
pub fn redo(
    fs: &impl Fs,
    paths: &StorePaths,
    doc_id: &str,
) -> Result<Option<Vec<u8>>, SessionError> {
    let mut state = load_state(fs, paths, doc_id)?;
    let target = match state.redo.pop() {
        Some(t) => t,
        None => return Ok(None),
    };
    state.head = Some(target);
    save_state(fs, paths, doc_id, &state)?;
    current_content(fs, paths, doc_id)
}

/// Discard all Tier-1 session state for `doc_id` (called on workspace close).
/// Idempotent: a no-op if no session exists. Durable Tier-2 history is unaffected.
pub fn clear_session(fs: &impl Fs, paths: &StorePaths, doc_id: &str) -> Result<(), SessionError> {
    let dir = paths.session_dir(doc_id);
    if fs.exists(&dir) {
        fs.remove(&dir)?;
    }
    Ok(())
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{FakeClock, FakeRng, MemFs};

    fn setup() -> (MemFs, StorePaths, FakeClock, FakeRng) {
        (
            MemFs::new(),
            StorePaths::new("/data"),
            FakeClock(std::time::SystemTime::UNIX_EPOCH),
            FakeRng(0),
        )
    }

    #[test]
    fn first_record_sets_head() {
        let (fs, paths, clock, rng) = setup();
        let outcome = record_state(&fs, &paths, &clock, &rng, "doc1", b"v1", None).unwrap();
        assert_eq!(
            outcome,
            RecordOutcome::Recorded {
                id: "r0".to_string()
            }
        );
        let content = current_content(&fs, &paths, "doc1").unwrap();
        assert_eq!(content, Some(b"v1".to_vec()));
        let state = load_state(&fs, &paths, "doc1").unwrap();
        assert_eq!(state.head, Some("r0".to_string()));
        assert!(state.redo.is_empty());
    }

    #[test]
    fn dedup_identical_head() {
        let (fs, paths, clock, rng) = setup();
        let first = record_state(&fs, &paths, &clock, &rng, "doc1", b"v1", None).unwrap();
        assert_eq!(
            first,
            RecordOutcome::Recorded {
                id: "r0".to_string()
            }
        );
        let second = record_state(&fs, &paths, &clock, &rng, "doc1", b"v1", None).unwrap();
        assert_eq!(second, RecordOutcome::Unchanged);
        let jpath = journal_path(&paths, "doc1");
        let records = read_records(&fs, &jpath).unwrap();
        assert_eq!(records.len(), 1);
    }

    #[test]
    fn second_distinct_record_advances_head_and_chains_parent() {
        let (fs, paths, clock, rng) = setup();
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v1", None).unwrap();
        let outcome = record_state(&fs, &paths, &clock, &rng, "doc1", b"v2", None).unwrap();
        assert_eq!(
            outcome,
            RecordOutcome::Recorded {
                id: "r1".to_string()
            }
        );
        let content = current_content(&fs, &paths, "doc1").unwrap();
        assert_eq!(content, Some(b"v2".to_vec()));
        let jpath = journal_path(&paths, "doc1");
        let records = read_records(&fs, &jpath).unwrap();
        assert_eq!(records.len(), 2);
        let r1 = find_record(&records, "r1").unwrap();
        assert_eq!(r1.parent, Some("r0".to_string()));
    }

    #[test]
    fn op_kind_is_stored() {
        let (fs, paths, clock, rng) = setup();
        record_state(&fs, &paths, &clock, &rng, "doc1", b"data", Some("external")).unwrap();
        let jpath = journal_path(&paths, "doc1");
        let records = read_records(&fs, &jpath).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].op_kind, Some("external".to_string()));
    }

    #[test]
    fn current_content_empty_session() {
        let (fs, paths, _clock, _rng) = setup();
        let result = current_content(&fs, &paths, "doc1").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn new_record_clears_redo() {
        let (fs, paths, clock, rng) = setup();
        // First record so HEAD is set (required for redo to mean anything).
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v1", None).unwrap();
        // Hand-craft a state with a non-empty redo stack.
        let mut state = load_state(&fs, &paths, "doc1").unwrap();
        state.redo = vec!["rX".to_string()];
        save_state(&fs, &paths, "doc1", &state).unwrap();
        // Record a new distinct state — must clear redo.
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v2", None).unwrap();
        let reloaded = load_state(&fs, &paths, "doc1").unwrap();
        assert!(reloaded.redo.is_empty());
    }

    #[test]
    fn recording_returns_same_object_for_identical_content_across_branches() {
        let (fs, paths, clock, rng) = setup();
        // r0: content A
        record_state(&fs, &paths, &clock, &rng, "doc1", b"A", None).unwrap();
        // r1: content B
        record_state(&fs, &paths, &clock, &rng, "doc1", b"B", None).unwrap();
        // r2: content A again — distinct from HEAD (B), so a new record is created.
        let outcome = record_state(&fs, &paths, &clock, &rng, "doc1", b"A", None).unwrap();
        assert_eq!(
            outcome,
            RecordOutcome::Recorded {
                id: "r2".to_string()
            }
        );
        let jpath = journal_path(&paths, "doc1");
        let records = read_records(&fs, &jpath).unwrap();
        assert_eq!(records.len(), 3);
        // r0 and r2 both point to the same object hash (content dedup).
        let r0 = find_record(&records, "r0").unwrap();
        let r2 = find_record(&records, "r2").unwrap();
        assert_eq!(r0.snapshot, r2.snapshot);
        // But they are distinct records with distinct ids.
        assert_ne!(r0.id, r2.id);
    }

    #[test]
    fn undo_moves_to_parent() {
        let (fs, paths, clock, rng) = setup();
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v1", None).unwrap(); // r0
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v2", None).unwrap(); // r1
        let content = undo(&fs, &paths, "doc1").unwrap();
        assert_eq!(content, Some(b"v1".to_vec()));
        let state = load_state(&fs, &paths, "doc1").unwrap();
        assert_eq!(state.head, Some("r0".to_string()));
        assert_eq!(state.redo, vec!["r1".to_string()]);
        assert_eq!(
            current_content(&fs, &paths, "doc1").unwrap(),
            Some(b"v1".to_vec())
        );
    }

    #[test]
    fn undo_at_root_returns_none_and_keeps_head() {
        let (fs, paths, clock, rng) = setup();
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v1", None).unwrap(); // r0
        let result = undo(&fs, &paths, "doc1").unwrap();
        assert_eq!(result, None);
        let state = load_state(&fs, &paths, "doc1").unwrap();
        assert_eq!(state.head, Some("r0".to_string()));
        assert!(state.redo.is_empty());
    }

    #[test]
    fn undo_empty_session_is_none() {
        let (fs, paths, _clock, _rng) = setup();
        let result = undo(&fs, &paths, "doc1").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn redo_returns_to_undone_state() {
        let (fs, paths, clock, rng) = setup();
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v1", None).unwrap(); // r0
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v2", None).unwrap(); // r1
        undo(&fs, &paths, "doc1").unwrap(); // back to r0
        let content = redo(&fs, &paths, "doc1").unwrap();
        assert_eq!(content, Some(b"v2".to_vec()));
        let state = load_state(&fs, &paths, "doc1").unwrap();
        assert_eq!(state.head, Some("r1".to_string()));
        assert!(state.redo.is_empty());
    }

    #[test]
    fn redo_without_undo_is_none() {
        let (fs, paths, clock, rng) = setup();
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v1", None).unwrap(); // r0
        let result = redo(&fs, &paths, "doc1").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn undo_undo_undo_redo_undo_sequence() {
        let (fs, paths, clock, rng) = setup();
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v1", None).unwrap(); // r0
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v2", None).unwrap(); // r1
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v3", None).unwrap(); // r2
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v4", None).unwrap(); // r3
        // undo x3 → r0 (v1)
        undo(&fs, &paths, "doc1").unwrap(); // r3 → r2
        undo(&fs, &paths, "doc1").unwrap(); // r2 → r1
        let after_third_undo = undo(&fs, &paths, "doc1").unwrap(); // r1 → r0
        assert_eq!(after_third_undo, Some(b"v1".to_vec()));
        // redo → r1 (v2)
        let after_redo = redo(&fs, &paths, "doc1").unwrap();
        assert_eq!(after_redo, Some(b"v2".to_vec()));
        // undo → r0 (v1)
        let after_final_undo = undo(&fs, &paths, "doc1").unwrap();
        assert_eq!(after_final_undo, Some(b"v1".to_vec()));
    }

    #[test]
    fn new_edit_after_undo_clears_redo_and_branches() {
        let (fs, paths, clock, rng) = setup();
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v1", None).unwrap(); // r0
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v2", None).unwrap(); // r1
        undo(&fs, &paths, "doc1").unwrap(); // back to r0; redo = [r1]
        let outcome = record_state(&fs, &paths, &clock, &rng, "doc1", b"v3", None).unwrap(); // r2
        assert_eq!(
            outcome,
            RecordOutcome::Recorded {
                id: "r2".to_string()
            }
        );
        assert_eq!(
            current_content(&fs, &paths, "doc1").unwrap(),
            Some(b"v3".to_vec())
        );
        // redo stack must be empty after a new record
        let redo_result = redo(&fs, &paths, "doc1").unwrap();
        assert_eq!(redo_result, None);
        let state = load_state(&fs, &paths, "doc1").unwrap();
        assert!(state.redo.is_empty());
    }

    #[test]
    fn round_trip_external_change_is_undoable() {
        let (fs, paths, clock, rng) = setup();
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v1", None).unwrap(); // r0
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v2", None).unwrap(); // r1
        // Simulate an external revert: record the same bytes as r0 with op_kind "external".
        let outcome =
            record_state(&fs, &paths, &clock, &rng, "doc1", b"v1", Some("external")).unwrap(); // r2
        let r2_id = match outcome {
            RecordOutcome::Recorded { ref id } => id.clone(),
            RecordOutcome::Unchanged => panic!("expected Recorded"),
        };
        // Verify op_kind and object dedup.
        let jpath = journal_path(&paths, "doc1");
        let records = read_records(&fs, &jpath).unwrap();
        let r0 = find_record(&records, "r0").unwrap();
        let r2 = find_record(&records, &r2_id).unwrap();
        assert_eq!(r2.op_kind, Some("external".to_string()));
        assert_eq!(r2.snapshot, r0.snapshot); // same bytes → same object hash
        assert_eq!(
            current_content(&fs, &paths, "doc1").unwrap(),
            Some(b"v1".to_vec())
        );
        // undo r2 → r1 (v2)
        let after_first_undo = undo(&fs, &paths, "doc1").unwrap();
        assert_eq!(after_first_undo, Some(b"v2".to_vec()));
        // undo r1 → r0 (v1)
        let after_second_undo = undo(&fs, &paths, "doc1").unwrap();
        assert_eq!(after_second_undo, Some(b"v1".to_vec()));
    }

    #[test]
    fn clear_session_removes_all_state() {
        let (fs, paths, clock, rng) = setup();
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v1", None).unwrap();
        record_state(&fs, &paths, &clock, &rng, "doc1", b"v2", None).unwrap();
        clear_session(&fs, &paths, "doc1").unwrap();
        let state = load_state(&fs, &paths, "doc1").unwrap();
        assert_eq!(state, SessionState::default());
        assert_eq!(current_content(&fs, &paths, "doc1").unwrap(), None);
        // Idempotent: second call is also Ok.
        clear_session(&fs, &paths, "doc1").unwrap();
    }
}
