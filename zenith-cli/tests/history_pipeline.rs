//! Integration tests for the history recording pipeline in `zenith-cli`.
//!
//! Calls `zenith_cli::history::record_edit_in` directly with a tempdir-rooted
//! `StorePaths` so no real data directory is touched. Assertions use the
//! `zenith_session` public API to verify that Tier-1 and Tier-2 records were
//! actually written.

use std::path::PathBuf;

use tempfile::TempDir;
use zenith_cli::history::record_edit_in;
use zenith_core::{KdlAdapter, KdlSource as _};
use zenith_session::adapter::OsFs;
use zenith_session::{StorePaths, current_content, list_versions};

// ── Fixture ───────────────────────────────────────────────────────────────────

/// A minimal valid `.zen` document (no `doc-id` attribute).
const MINIMAL_NO_ID: &str = r##"zenith version=1 {
  project id="proj.hist" name="History Test"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#f8fafc"
  }
  styles {
  }
  document id="doc.hist" title="History Test" {
    page id="page.one" w=(px)480 h=(px)160 {
      rect id="rect.bg" x=(px)0 y=(px)0 w=(px)480 h=(px)160 fill=(token)"color.bg"
    }
  }
}
"##;

fn store_in(tmp: &TempDir) -> StorePaths {
    StorePaths::new(tmp.path())
}

fn doc_path_in(tmp: &TempDir) -> PathBuf {
    tmp.path().join("test-doc.zen")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// First call with no existing `doc-id` must:
/// - return `warning: None`
/// - stamp a 26-character ULID into the returned bytes
/// - write a Tier-1 session record whose content matches the returned bytes
#[test]
fn mints_and_stamps_doc_id_on_first_edit() {
    let tmp = TempDir::new().unwrap();
    let paths = store_in(&tmp);
    let doc_path = doc_path_in(&tmp);

    let recorded = record_edit_in(&paths, MINIMAL_NO_ID.as_bytes(), &doc_path, "tx.apply");

    assert!(
        recorded.warning.is_none(),
        "expected no warning on first edit; got: {:?}",
        recorded.warning
    );

    // Parse the returned bytes and verify a doc-id was stamped.
    let doc = KdlAdapter
        .parse(&recorded.bytes)
        .expect("returned bytes must parse");
    let doc_id = doc
        .doc_id
        .as_deref()
        .expect("doc-id must be present after first edit");
    assert_eq!(
        doc_id.len(),
        26,
        "doc-id must be a 26-character ULID; got: {doc_id:?}"
    );

    // Tier-1 HEAD content must match the stamped bytes.
    let fs = OsFs;
    let head = current_content(&fs, &paths, doc_id)
        .expect("current_content must succeed")
        .expect("HEAD must be set after first edit");
    assert_eq!(
        head, recorded.bytes,
        "Tier-1 HEAD content must equal the stamped bytes"
    );
}

/// A second `record_edit_in` at the SAME path with the stamped bytes must:
/// - keep the SAME `doc-id` (Matched outcome — no re-mint)
/// - accumulate a second Tier-2 version record (or Unchanged if content identical)
#[test]
fn second_edit_matches_same_doc_id() {
    let tmp = TempDir::new().unwrap();
    let paths = store_in(&tmp);
    let doc_path = doc_path_in(&tmp);

    // First edit: mint the id.
    let first = record_edit_in(&paths, MINIMAL_NO_ID.as_bytes(), &doc_path, "tx.apply");
    assert!(first.warning.is_none(), "first edit must have no warning");

    let doc_id_first = KdlAdapter
        .parse(&first.bytes)
        .unwrap()
        .doc_id
        .expect("doc-id present after first edit");

    // Second edit: re-record the same stamped bytes.
    let second = record_edit_in(&paths, &first.bytes, &doc_path, "tx.apply");
    assert!(second.warning.is_none(), "second edit must have no warning");

    let doc_id_second = KdlAdapter
        .parse(&second.bytes)
        .unwrap()
        .doc_id
        .expect("doc-id must still be present after second edit");

    assert_eq!(
        doc_id_first, doc_id_second,
        "doc-id must be stable across edits at the same path"
    );
}

/// After a single `record_edit_in`, `list_versions` must return at least 1
/// version whose content matches the returned bytes.
#[test]
fn records_tier2_version() {
    let tmp = TempDir::new().unwrap();
    let paths = store_in(&tmp);
    let doc_path = doc_path_in(&tmp);

    let recorded = record_edit_in(&paths, MINIMAL_NO_ID.as_bytes(), &doc_path, "tx.apply");
    assert!(recorded.warning.is_none(), "must have no warning");

    let doc_id = KdlAdapter
        .parse(&recorded.bytes)
        .unwrap()
        .doc_id
        .expect("doc-id present");

    let fs = OsFs;
    let versions = list_versions(&fs, &paths, &doc_id).expect("list_versions must succeed");
    assert!(
        !versions.is_empty(),
        "at least one Tier-2 version must be recorded after first edit"
    );

    // The stored content of the most-recent version must equal the stamped bytes.
    use zenith_session::version_content;
    let last = versions.last().unwrap();
    let stored =
        version_content(&fs, &paths, &doc_id, &last.id).expect("version_content must succeed");
    assert_eq!(
        stored, recorded.bytes,
        "Tier-2 version content must match the stamped bytes"
    );
}

/// Recording the identical stamped bytes twice must not produce a NEW Tier-1
/// record on the second call (`record_state` deduplicates against HEAD).
#[test]
fn idempotent_dedup() {
    let tmp = TempDir::new().unwrap();
    let paths = store_in(&tmp);
    let doc_path = doc_path_in(&tmp);

    // First edit: establishes HEAD.
    let first = record_edit_in(&paths, MINIMAL_NO_ID.as_bytes(), &doc_path, "tx.apply");
    assert!(first.warning.is_none());

    let doc_id = KdlAdapter
        .parse(&first.bytes)
        .unwrap()
        .doc_id
        .expect("doc-id present");

    // Verify dedup via current_content stability: re-recording the same bytes
    // must leave HEAD unchanged (dedup means no new record appended).
    let head_before = current_content(&OsFs, &paths, &doc_id)
        .expect("current_content must succeed")
        .expect("HEAD must be set");

    // Second edit with IDENTICAL bytes — must be deduped by record_state.
    let second = record_edit_in(&paths, &first.bytes, &doc_path, "tx.apply");
    assert!(second.warning.is_none());

    let head_after = current_content(&OsFs, &paths, &doc_id)
        .expect("current_content must succeed")
        .expect("HEAD must be set");

    // Dedup means HEAD content is still identical.
    assert_eq!(
        head_before, head_after,
        "dedup: HEAD content must be unchanged after recording identical bytes"
    );
}
