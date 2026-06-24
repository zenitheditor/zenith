//! Integration tests for `zenith workspace scratch` and `zenith workspace candidate`.
//!
//! Uses the `_in` variants of the command functions so that a tempdir-rooted
//! `StorePaths` is passed explicitly — no real data directory is touched and
//! no `ZENITH_DATA_DIR` env-var is required. The harness mirrors
//! `history_pipeline.rs`.

use tempfile::TempDir;
use zenith_cli::cli::ScratchNewArgs;
use zenith_cli::commands::workspace::{
    candidate_set_status_in, scratch_list_in, scratch_new_in, scratch_show_in,
};
use zenith_session::StorePaths;

// ── Fixture ───────────────────────────────────────────────────────────────────

/// A minimal valid `.zen` document WITH a `doc-id` attribute (required by
/// `read_doc_id`, which errors when no id is present).
const FIXTURE: &str = r##"zenith version=1 doc-id="01HWSCRATCHTEST000000000001" {
  project id="proj.ws" name="Workspace Test"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
  }
  styles {
  }
  document id="doc.ws" title="Workspace Test" {
    page id="page.main" w=(px)400 h=(px)300 {
      rect id="rect.bg" x=(px)0 y=(px)0 w=(px)400 h=(px)300 fill=(token)"color.bg"
    }
  }
}
"##;

const DOC_ID: &str = "01HWSCRATCHTEST000000000001";

fn setup() -> (TempDir, StorePaths, std::path::PathBuf) {
    let tmp = TempDir::new().unwrap();
    let paths = StorePaths::new(tmp.path());
    let doc_path = tmp.path().join("doc.zen");
    std::fs::write(&doc_path, FIXTURE).unwrap();
    (tmp, paths, doc_path)
}

fn new_args(
    doc_path: &std::path::Path,
    page: Option<&str>,
    status: &str,
    notes: Option<&str>,
    promotion_target: Option<&str>,
    cleanup_policy: Option<&str>,
    workspace_role: Option<&str>,
) -> ScratchNewArgs {
    ScratchNewArgs {
        doc: doc_path.to_path_buf(),
        page: page.map(str::to_owned),
        status: status.to_owned(),
        notes: notes.map(str::to_owned),
        promotion_target: promotion_target.map(str::to_owned),
        cleanup_policy: cleanup_policy.map(str::to_owned),
        workspace_role: workspace_role.map(str::to_owned),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[test]
fn scratch_new_returns_candidate_id() {
    let (_tmp, paths, doc_path) = setup();

    let id = scratch_new_in(
        &paths,
        FIXTURE.as_bytes(),
        &doc_path,
        &new_args(&doc_path, None, "draft", None, None, None, None),
    )
    .unwrap();

    assert_eq!(id, "cand0", "first candidate must be cand0");
}

#[test]
fn scratch_list_shows_new_candidate() {
    let (_tmp, paths, doc_path) = setup();

    scratch_new_in(
        &paths,
        FIXTURE.as_bytes(),
        &doc_path,
        &new_args(
            &doc_path,
            Some("page.main"),
            "draft",
            Some("first"),
            None,
            None,
            None,
        ),
    )
    .unwrap();

    let out = scratch_list_in(&paths, &doc_path, false).unwrap();
    assert!(out.contains("cand0"), "listing must mention cand0");
    assert!(out.contains("draft"), "listing must show draft status");
    assert!(out.contains("page.main"), "listing must show page id");
    assert!(out.contains("first"), "listing must show notes");
}

#[test]
fn scratch_list_empty_when_no_candidates() {
    let (_tmp, paths, doc_path) = setup();

    let out = scratch_list_in(&paths, &doc_path, false).unwrap();
    assert!(
        out.contains("no scratch candidates"),
        "empty listing must say so"
    );
}

#[test]
fn scratch_list_json_is_array() {
    let (_tmp, paths, doc_path) = setup();

    scratch_new_in(
        &paths,
        FIXTURE.as_bytes(),
        &doc_path,
        &new_args(&doc_path, None, "draft", None, None, None, None),
    )
    .unwrap();

    let out = scratch_list_in(&paths, &doc_path, true).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(parsed.is_array(), "JSON output must be an array");
    assert_eq!(parsed.as_array().unwrap().len(), 1);
}

#[test]
fn candidate_set_status_selected_reflected_in_list() {
    let (_tmp, paths, doc_path) = setup();

    scratch_new_in(
        &paths,
        FIXTURE.as_bytes(),
        &doc_path,
        &new_args(&doc_path, None, "draft", None, None, None, None),
    )
    .unwrap();

    let confirm = candidate_set_status_in(&paths, &doc_path, "cand0", "selected").unwrap();
    assert!(
        confirm.contains("cand0"),
        "confirmation must mention the candidate id"
    );
    assert!(
        confirm.contains("selected"),
        "confirmation must mention the new status"
    );

    // The store must reflect the new status.
    let out = scratch_list_in(&paths, &doc_path, false).unwrap();
    assert!(
        out.contains("selected"),
        "list must show 'selected' after status transition; got: {out}"
    );

    // Verify via session API directly.
    use zenith_session::adapter::OsFs;
    use zenith_session::{CandidateStatus, list_scratch};
    let entries = list_scratch(&OsFs, &paths, DOC_ID).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].status, CandidateStatus::Selected);
}

#[test]
fn scratch_show_unknown_id_errors() {
    let (_tmp, paths, doc_path) = setup();

    // Record one candidate so the index exists.
    scratch_new_in(
        &paths,
        FIXTURE.as_bytes(),
        &doc_path,
        &new_args(&doc_path, None, "draft", None, None, None, None),
    )
    .unwrap();

    let result = scratch_show_in(&paths, &doc_path, "cand99", false);
    assert!(result.is_err(), "unknown candidate must error");
    let msg = result.unwrap_err();
    assert!(
        msg.contains("cand99"),
        "error message must mention the missing id; got: {msg}"
    );
}

#[test]
fn bad_status_string_errors() {
    let (_tmp, paths, doc_path) = setup();

    let result = scratch_new_in(
        &paths,
        FIXTURE.as_bytes(),
        &doc_path,
        &new_args(&doc_path, None, "nonsense", None, None, None, None),
    );
    assert!(result.is_err(), "bad status string must error");
    let msg = result.unwrap_err();
    assert!(
        msg.contains("nonsense"),
        "error must include the bad value; got: {msg}"
    );
}

#[test]
fn scratch_show_returns_detail() {
    let (_tmp, paths, doc_path) = setup();

    scratch_new_in(
        &paths,
        FIXTURE.as_bytes(),
        &doc_path,
        &new_args(
            &doc_path,
            Some("page.main"),
            "draft",
            Some("show-me"),
            Some("slot-a"),
            Some("on_select"),
            Some("hero"),
        ),
    )
    .unwrap();

    let out = scratch_show_in(&paths, &doc_path, "cand0", false).unwrap();
    assert!(out.contains("cand0"), "show must include id");
    assert!(out.contains("page.main"), "show must include page");
    assert!(out.contains("draft"), "show must include status");
    assert!(out.contains("show-me"), "show must include notes");
    assert!(out.contains("slot-a"), "show must include promotion_target");
    assert!(
        out.contains("on_select"),
        "show must include cleanup_policy"
    );
    assert!(out.contains("hero"), "show must include workspace_role");
}

#[test]
fn scratch_show_json_roundtrip() {
    let (_tmp, paths, doc_path) = setup();

    scratch_new_in(
        &paths,
        FIXTURE.as_bytes(),
        &doc_path,
        &new_args(
            &doc_path,
            Some("page.main"),
            "selected",
            Some("json-test"),
            None,
            None,
            None,
        ),
    )
    .unwrap();

    let out = scratch_show_in(&paths, &doc_path, "cand0", true).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(parsed["id"], "cand0");
    assert_eq!(parsed["status"], "selected");
    assert_eq!(parsed["page_id"], "page.main");
    assert_eq!(parsed["notes"], "json-test");
}
