//! Integration tests for `zenith workspace scratch`, `zenith workspace candidate`,
//! `zenith workspace promote`, `zenith workspace bundle`, and
//! `zenith workspace unbundle`.
//!
//! Uses the `_in` variants of the command functions so that a tempdir-rooted
//! `StorePaths` is passed explicitly — no real data directory is touched and
//! no `ZENITH_DATA_DIR` env-var is required. The harness mirrors
//! `history_pipeline.rs`.

use tempfile::TempDir;
use zenith_cli::cli::ScratchNewArgs;
use zenith_cli::commands::workspace::{
    bundle_doc_in, candidate_set_status_in, finalize_in, promote_in, scratch_list_in,
    scratch_new_in, scratch_show_in, unbundle_doc_in,
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
    .unwrap()
    .id;

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

// ── Promote tests ─────────────────────────────────────────────────────────────

/// Deliverable document: has `doc-id`, two pages — `page.export` is the
/// target page with a placeholder rect.
const DELIVERABLE: &str = r##"zenith version=1 doc-id="01HWSCRATCHTEST000000000001" {
  project id="proj.del" name="Deliverable"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
  }
  styles {
  }
  document id="doc.del" title="Deliverable" {
    page id="page.export" w=(px)400 h=(px)300 {
      rect id="placeholder" x=(px)0 y=(px)0 w=(px)400 h=(px)300
    }
  }
}
"##;

/// Candidate snapshot: a selected-status page with content nodes.
const CANDIDATE_SNAP: &str = r##"zenith version=1 doc-id="01HWSCRATCHTEST000000000001" {
  project id="proj.cand" name="Candidate"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
  }
  styles {
  }
  document id="doc.cand" title="Candidate" {
    page id="page.source" w=(px)400 h=(px)300 {
      rect id="hero" x=(px)0 y=(px)0 w=(px)400 h=(px)300
      rect id="sub" x=(px)10 y=(px)10 w=(px)100 h=(px)50
    }
  }
}
"##;

fn setup_promote() -> (TempDir, StorePaths, std::path::PathBuf) {
    let tmp = TempDir::new().unwrap();
    let paths = StorePaths::new(tmp.path());
    let doc_path = tmp.path().join("deliver.zen");
    std::fs::write(&doc_path, DELIVERABLE).unwrap();
    (tmp, paths, doc_path)
}

fn record_selected_candidate(
    paths: &StorePaths,
    doc_path: &std::path::Path,
    page_id: &str,
    snap: &[u8],
) -> String {
    let cand_id = scratch_new_in(
        paths,
        snap,
        doc_path,
        &new_args(doc_path, Some(page_id), "draft", None, None, None, None),
    )
    .unwrap()
    .id;
    candidate_set_status_in(paths, doc_path, &cand_id, "selected").unwrap();
    cand_id
}

#[test]
fn promote_selected_candidate_merges_content() {
    let (_tmp, paths, doc_path) = setup_promote();
    let cand_id =
        record_selected_candidate(&paths, &doc_path, "page.source", CANDIDATE_SNAP.as_bytes());

    let out = promote_in(&paths, &doc_path, &cand_id, "page.export", ".promoted").unwrap();
    assert!(
        out.contains(&cand_id),
        "confirmation must mention candidate id"
    );
    assert!(
        out.contains("page.export"),
        "confirmation must mention target page"
    );

    // Read the written file and verify the promoted content.
    let written = std::fs::read_to_string(&doc_path).unwrap();
    assert!(
        written.contains("hero.promoted"),
        "written doc must contain suffixed hero id; got:\n{written}"
    );
    assert!(
        written.contains("sub.promoted"),
        "written doc must contain suffixed sub id; got:\n{written}"
    );
    // The old placeholder must be gone (replaced).
    assert!(
        !written.contains("\"placeholder\""),
        "placeholder must be replaced; got:\n{written}"
    );
}

#[test]
fn promote_draft_candidate_errors() {
    let (_tmp, paths, doc_path) = setup_promote();
    // Record a draft candidate (do NOT transition to selected).
    let cand_id = scratch_new_in(
        &paths,
        CANDIDATE_SNAP.as_bytes(),
        &doc_path,
        &new_args(
            &doc_path,
            Some("page.source"),
            "draft",
            None,
            None,
            None,
            None,
        ),
    )
    .unwrap()
    .id;

    let result = promote_in(&paths, &doc_path, &cand_id, "page.export", ".promoted");
    assert!(result.is_err(), "promoting a draft candidate must error");
    let msg = result.unwrap_err();
    assert!(
        msg.contains("selected"),
        "error must mention 'selected'; got: {msg}"
    );
    assert!(
        msg.contains(&cand_id),
        "error must mention the candidate id; got: {msg}"
    );
}

#[test]
fn promote_missing_candidate_errors() {
    let (_tmp, paths, doc_path) = setup_promote();

    let result = promote_in(&paths, &doc_path, "cand99", "page.export", ".promoted");
    assert!(result.is_err(), "unknown candidate must error");
    let msg = result.unwrap_err();
    assert!(
        msg.contains("cand99"),
        "error must mention the missing id; got: {msg}"
    );
}

#[test]
fn promote_missing_target_page_errors() {
    let (_tmp, paths, doc_path) = setup_promote();
    let cand_id =
        record_selected_candidate(&paths, &doc_path, "page.source", CANDIDATE_SNAP.as_bytes());

    let result = promote_in(
        &paths,
        &doc_path,
        &cand_id,
        "page.does-not-exist",
        ".promoted",
    );
    assert!(result.is_err(), "missing target page must error");
    let msg = result.unwrap_err();
    assert!(
        msg.contains("page.does-not-exist"),
        "error must mention the missing page id; got: {msg}"
    );
}

// ── Token-palette promote tests ───────────────────────────────────────────────

/// Deliverable with palette B: has `color.shared` (value "#ffffff") and
/// `color.del-only` ("#aabbcc"). The candidate will carry a DIFFERENT value for
/// `color.shared` and an additional `color.cand-only` token.
const DELIVERABLE_PALETTE_B: &str = r##"zenith version=1 doc-id="01HWSCRATCHTEST000000000002" {
  project id="proj.del2" name="Deliverable-B"
  tokens format="zenith-token-v1" {
    token id="color.shared" type="color" value="#ffffff"
    token id="color.del-only" type="color" value="#aabbcc"
  }
  styles {
  }
  document id="doc.del2" title="Deliverable-B" {
    page id="page.export" w=(px)400 h=(px)300 {
      rect id="placeholder" x=(px)0 y=(px)0 w=(px)400 h=(px)300
    }
  }
}
"##;

/// Candidate snapshot with palette A: `color.shared` has a DIFFERENT value
/// ("#000000"), and it introduces `color.cand-only` ("#112233") while being
/// silent about `color.del-only`.
const CANDIDATE_PALETTE_A: &str = r##"zenith version=1 doc-id="01HWSCRATCHTEST000000000002" {
  project id="proj.cand2" name="Candidate-A"
  tokens format="zenith-token-v1" {
    token id="color.shared" type="color" value="#000000"
    token id="color.cand-only" type="color" value="#112233"
  }
  styles {
  }
  document id="doc.cand2" title="Candidate-A" {
    page id="page.source" w=(px)400 h=(px)300 {
      rect id="hero2" x=(px)0 y=(px)0 w=(px)400 h=(px)300
    }
  }
}
"##;

fn setup_promote_palette() -> (TempDir, StorePaths, std::path::PathBuf) {
    let tmp = TempDir::new().unwrap();
    let paths = StorePaths::new(tmp.path());
    let doc_path = tmp.path().join("deliver2.zen");
    std::fs::write(&doc_path, DELIVERABLE_PALETTE_B).unwrap();
    (tmp, paths, doc_path)
}

/// Promoting a candidate whose palette diverges from the deliverable must:
/// - overwrite the shared token id with the candidate's value,
/// - retain the deliverable-only token,
/// - append the candidate-only token,
/// - produce a deterministic final ordering.
#[test]
fn promote_reconciles_candidate_tokens_into_deliverable() {
    use zenith_core::{KdlAdapter, KdlSource as _};

    let (_tmp, paths, doc_path) = setup_promote_palette();
    let cand_id = record_selected_candidate(
        &paths,
        &doc_path,
        "page.source",
        CANDIDATE_PALETTE_A.as_bytes(),
    );

    let out = promote_in(&paths, &doc_path, &cand_id, "page.export", ".p").unwrap();
    assert!(
        out.contains(&cand_id),
        "confirmation must mention candidate id"
    );

    // Parse the written document and inspect its token block.
    let written_bytes = std::fs::read(&doc_path).unwrap();
    let doc = KdlAdapter.parse(written_bytes.as_slice()).unwrap();
    let tokens = &doc.tokens.tokens;

    // Collect ids in order for determinism checks.
    let ids: Vec<&str> = tokens.iter().map(|t| t.id.as_str()).collect();

    // shared id must be present and carry the candidate's value.
    let shared = tokens
        .iter()
        .find(|t| t.id == "color.shared")
        .expect("color.shared must be present after promote");
    let shared_val = match &shared.value {
        zenith_core::ast::token::TokenValue::Literal(
            zenith_core::ast::token::TokenLiteral::String(s),
        ) => s.as_str(),
        other => panic!("unexpected token value kind: {other:?}"),
    };
    assert_eq!(
        shared_val, "#000000",
        "color.shared must carry the candidate's value '#000000'; got: {shared_val}"
    );

    // deliverable-only token must be retained.
    assert!(
        tokens.iter().any(|t| t.id == "color.del-only"),
        "color.del-only must be retained; ids present: {ids:?}"
    );

    // candidate-only token must be appended.
    assert!(
        tokens.iter().any(|t| t.id == "color.cand-only"),
        "color.cand-only must be appended; ids present: {ids:?}"
    );

    // Ordering: deliverable order first (shared, del-only), then appended cand-only.
    assert_eq!(
        ids,
        ["color.shared", "color.del-only", "color.cand-only"],
        "token order must be deterministic; got: {ids:?}"
    );
}

// ── Bundle / unbundle tests ───────────────────────────────────────────────────

#[test]
fn bundle_unbundle_roundtrip_through_cli_fns() {
    let (_tmp, paths, doc_path) = setup();

    // Record a scratch candidate so the store has content worth bundling.
    scratch_new_in(
        &paths,
        FIXTURE.as_bytes(),
        &doc_path,
        &new_args(
            &doc_path,
            None,
            "draft",
            Some("bundle-test"),
            None,
            None,
            None,
        ),
    )
    .unwrap();

    // Bundle the doc into a temp file.
    let bundle_file = _tmp.path().join("test.zenithbundle");
    let confirm = bundle_doc_in(&paths, &doc_path, &bundle_file).unwrap();
    assert!(
        confirm.contains(DOC_ID),
        "confirmation must contain doc_id; got: {confirm}"
    );
    assert!(bundle_file.exists(), "bundle file must exist on disk");

    // Unbundle into a completely fresh store root.
    let tmp2 = TempDir::new().unwrap();
    let paths2 = StorePaths::new(tmp2.path());
    let restored_id = unbundle_doc_in(&paths2, &bundle_file).unwrap();
    assert_eq!(restored_id, DOC_ID, "restored doc_id must match original");

    // The scratch candidate list must be accessible in the fresh store.
    let out = scratch_list_in(&paths2, &doc_path, false).unwrap();
    assert!(
        out.contains("cand0"),
        "fresh store must contain the bundled candidate; got: {out}"
    );
}

#[test]
fn bundle_missing_doc_errors() {
    let tmp = TempDir::new().unwrap();
    let paths = StorePaths::new(tmp.path());
    // Create a doc_path that has a doc-id but whose store directory was never created.
    let doc_path = tmp.path().join("ghost.zen");
    std::fs::write(&doc_path, FIXTURE).unwrap();

    let bundle_file = tmp.path().join("ghost.zenithbundle");
    let result = bundle_doc_in(&paths, &doc_path, &bundle_file);
    assert!(result.is_err(), "bundling a non-existent store must error");
    let msg = result.unwrap_err();
    assert!(
        msg.contains(DOC_ID),
        "error must mention the doc_id; got: {msg}"
    );
}

// ── Finalize tests ────────────────────────────────────────────────────────────

#[test]
fn finalize_removes_rejected_delete_candidate() {
    let (_tmp, paths, doc_path) = setup();

    // cand0: rejected + delete → should be removed
    scratch_new_in(
        &paths,
        FIXTURE.as_bytes(),
        &doc_path,
        &new_args(
            &doc_path,
            None,
            "rejected",
            None,
            None,
            Some("delete"),
            None,
        ),
    )
    .unwrap();

    // cand1: draft → kept
    scratch_new_in(
        &paths,
        FIXTURE.as_bytes(),
        &doc_path,
        &new_args(&doc_path, None, "draft", None, None, None, None),
    )
    .unwrap();

    let out = finalize_in(&paths, &doc_path, false).unwrap();
    assert!(
        out.contains("cand0"),
        "report must mention deleted id; got: {out}"
    );
    assert!(
        out.contains("deleted"),
        "report must say 'deleted'; got: {out}"
    );

    // cand0 must be gone from the listing
    let list = scratch_list_in(&paths, &doc_path, false).unwrap();
    assert!(
        !list.contains("cand0"),
        "cand0 must be absent after finalize; got: {list}"
    );
    assert!(
        list.contains("cand1"),
        "cand1 must still be present; got: {list}"
    );
}

#[test]
fn finalize_json_output_is_valid() {
    let (_tmp, paths, doc_path) = setup();

    scratch_new_in(
        &paths,
        FIXTURE.as_bytes(),
        &doc_path,
        &new_args(
            &doc_path,
            None,
            "rejected",
            None,
            None,
            Some("delete"),
            None,
        ),
    )
    .unwrap();

    let out = finalize_in(&paths, &doc_path, true).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
    let deleted = parsed["deleted"].as_array().unwrap();
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0], "cand0");
    assert_eq!(parsed["kept"], 0);
}

#[test]
fn finalize_noop_when_no_delete_policy() {
    let (_tmp, paths, doc_path) = setup();

    scratch_new_in(
        &paths,
        FIXTURE.as_bytes(),
        &doc_path,
        &new_args(
            &doc_path,
            None,
            "rejected",
            None,
            None,
            Some("archive"),
            None,
        ),
    )
    .unwrap();

    let out = finalize_in(&paths, &doc_path, false).unwrap();
    assert!(
        out.contains("nothing to delete"),
        "report must say nothing to delete; got: {out}"
    );

    let list = scratch_list_in(&paths, &doc_path, false).unwrap();
    assert!(
        list.contains("cand0"),
        "archive-policy candidate must survive; got: {list}"
    );
}

#[test]
fn unbundle_bad_file_errors() {
    let tmp = TempDir::new().unwrap();
    let paths = StorePaths::new(tmp.path());
    let bad_file = tmp.path().join("bad.zenithbundle");
    std::fs::write(&bad_file, b"not-a-bundle").unwrap();

    let result = unbundle_doc_in(&paths, &bad_file);
    assert!(result.is_err(), "bad bundle file must error");
    let msg = result.unwrap_err();
    assert!(
        msg.contains("magic"),
        "error must mention 'magic'; got: {msg}"
    );
}

// ── auto-attach on `scratch new` ────────────────────────────────────────────────

/// A minimal valid `.zen` document WITHOUT a `doc-id` attribute. `scratch new`
/// must transparently mint + stamp one rather than erroring.
const FIXTURE_NO_ID: &str = r##"zenith version=1 {
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

fn setup_no_id() -> (TempDir, StorePaths, std::path::PathBuf) {
    let tmp = TempDir::new().unwrap();
    let paths = StorePaths::new(tmp.path());
    let doc_path = tmp.path().join("doc.zen");
    std::fs::write(&doc_path, FIXTURE_NO_ID).unwrap();
    (tmp, paths, doc_path)
}

/// `scratch new` on a doc with NO doc-id must succeed, stamp a doc-id into the
/// on-disk file, record an initial Tier-2 version (whose content is the stamped
/// bytes), and create exactly one candidate.
#[test]
fn scratch_new_auto_attaches_doc_id_when_absent() {
    use zenith_core::{KdlAdapter, KdlSource as _};
    use zenith_session::adapter::OsFs;
    use zenith_session::{list_scratch, list_versions, version_content};

    let (_tmp, paths, doc_path) = setup_no_id();

    let id = scratch_new_in(
        &paths,
        FIXTURE_NO_ID.as_bytes(),
        &doc_path,
        &new_args(&doc_path, None, "draft", None, None, None, None),
    )
    .expect("scratch new on a doc with no doc-id must succeed")
    .id;
    assert_eq!(id, "cand0", "first candidate must be cand0");

    // The on-disk file must now carry a stamped doc-id.
    let on_disk = std::fs::read(&doc_path).unwrap();
    let stamped_id = KdlAdapter
        .parse(&on_disk)
        .unwrap()
        .doc_id
        .expect("file must carry a doc-id after auto-attach");
    assert_eq!(
        stamped_id.len(),
        26,
        "stamped doc-id must be a 26-char ULID"
    );

    let fs = OsFs;

    // An initial Tier-2 version must have been recorded by the attach.
    let versions = list_versions(&fs, &paths, &stamped_id).unwrap();
    assert!(
        !versions.is_empty(),
        "auto-attach must record an initial version"
    );

    // The candidate snapshot must equal the post-stamp on-disk bytes.
    let entries = list_scratch(&fs, &paths, &stamped_id).unwrap();
    assert_eq!(entries.len(), 1, "exactly one candidate must exist");
    let stored_version = version_content(&fs, &paths, &stamped_id, &versions[0].id).unwrap();
    assert_eq!(
        stored_version, on_disk,
        "recorded version content must equal the stamped on-disk bytes"
    );
}

/// `scratch new` on a doc that ALREADY has a doc-id must NOT add a spurious
/// history version (behavior unchanged from before auto-attach existed).
#[test]
fn scratch_new_does_not_record_history_when_doc_id_present() {
    use zenith_session::adapter::OsFs;
    use zenith_session::list_versions;

    let (_tmp, paths, doc_path) = setup();

    let fs = OsFs;
    let before = list_versions(&fs, &paths, DOC_ID).unwrap();

    scratch_new_in(
        &paths,
        FIXTURE.as_bytes(),
        &doc_path,
        &new_args(&doc_path, None, "draft", None, None, None, None),
    )
    .unwrap();

    let after = list_versions(&fs, &paths, DOC_ID).unwrap();
    assert_eq!(
        before.len(),
        after.len(),
        "scratch new on a doc with an existing doc-id must not record a version"
    );

    // The on-disk file must be byte-identical (no re-stamp / re-format).
    let on_disk = std::fs::read(&doc_path).unwrap();
    assert_eq!(
        on_disk,
        FIXTURE.as_bytes(),
        "file with an existing doc-id must be left untouched"
    );
}
