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
use crate::store::{get_object, object_hash, put_object};

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

    // Store the object (dedups bytes) and append a new record.
    let hash = put_object(fs, paths, doc_id, content)?;
    let seq = u64::try_from(records.len())
        .map_err(|_| SessionError::new("session record count exceeds u64"))?;
    let id = format!("r{seq}");
    let mut rec = HistoryRecord::new(id.clone(), seq, state.head.clone(), hash);
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
}
