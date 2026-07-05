//! Integration tests for `zenith theme apply` — re-skinning a document's
//! token values from a theme pack via the `tx` pipeline.
//!
//! Calls `zenith_cli::commands::theme::apply_run` and `zenith_cli::commands::new::run_in`
//! directly with a tempdir-rooted `StorePaths` so no real data directory or
//! embedded-pack file is touched by the test process itself (embedded packs
//! are compiled in).

use std::path::PathBuf;

use tempfile::TempDir;
use zenith_cli::commands::new::{self, DEFAULT_PAGE};
use zenith_cli::commands::theme::apply_run;
use zenith_core::{KdlAdapter, KdlSource as _, Severity, validate as core_validate};
use zenith_session::StorePaths;

fn store_in(tmp: &TempDir) -> StorePaths {
    StorePaths::new(tmp.path())
}

fn doc_path(tmp: &TempDir, name: &str) -> PathBuf {
    tmp.path().join(name)
}

/// Scaffold a `--theme sunset` document into `tmp` and return its path plus
/// its on-disk source.
fn sunset_doc(tmp: &TempDir) -> (PathBuf, String) {
    let paths = store_in(tmp);
    let path = doc_path(tmp, "poster.zen");
    new::run_in(&paths, &path, Some("Poster"), DEFAULT_PAGE, Some("sunset"))
        .expect("themed new must succeed");
    let src = std::fs::read_to_string(&path).expect("scaffolded file must be readable");
    (path, src)
}

fn hard_error_count(src: &str) -> usize {
    let doc = KdlAdapter.parse(src.as_bytes()).expect("doc must parse");
    core_validate(&doc)
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count()
}

/// A hand-built document exercising all four pre-filter outcomes against the
/// `cobalt`/`sunset` theme packs in one shot:
/// - `color.primary` (type `color`) matches the theme's type → update.
/// - `radius.box` is declared `color` here, but the theme declares it
///   `dimension` → type mismatch, must be skipped.
/// - `color.custom` has no counterpart in either theme pack → doc-only,
///   must never be touched.
/// - every other theme token id (`color.secondary`, `color.accent`, …) is
///   absent from this document → must be created.
const MIXED_DOC: &str = r##"zenith version=1 {
  project id="proj.mixed" name="Mixed"
  tokens format="zenith-token-v1" {
    token id="color.primary" type="color" value="#000000"
    token id="radius.box" type="color" value="#111111"
    token id="color.custom" type="color" value="#222222"
  }
  styles { }
  document id="doc.mixed" title="Mixed" {
    page id="pg" w=(px)400 h=(px)300 background=(token)"color.primary" {
      rect id="box" x=(px)0 y=(px)0 w=(px)400 h=(px)300 fill=(token)"color.primary"
    }
  }
}"##;

// ── Full re-skin ──────────────────────────────────────────────────────────────

/// A full sunset→cobalt re-skin of a `new --theme sunset` document: the
/// dry-run reports a change, and the applied result still validates with zero
/// hard (Error) diagnostics.
#[test]
fn full_reskin_sunset_to_cobalt_dry_run_then_validates_clean() {
    let tmp = TempDir::new().unwrap();
    let (path, src) = sunset_doc(&tmp);

    let outcome = apply_run(path.parent(), "cobalt", &src).expect("cobalt pack must resolve");

    assert_eq!(outcome.exit_code, 0, "accepted re-skin must exit 0");
    assert_ne!(
        outcome.result.source_before, outcome.result.source_after,
        "dry-run must report a change"
    );
    assert!(
        outcome.result.source_after.contains("#605dff"),
        "re-skinned source must carry cobalt's primary colour; got:\n{}",
        outcome.result.source_after
    );
    assert!(
        outcome
            .result
            .source_after
            .contains("id=\"color.primary\" type=\"color\" set=\"@zenith/theme.cobalt\""),
        "re-skinned token must carry the NEW pack id in `set`, not the original \
         sunset provenance; got:\n{}",
        outcome.result.source_after
    );
    assert!(
        !outcome.result.source_after.contains("@zenith/theme.sunset"),
        "re-skinned document must not retain the old theme's set provenance; got:\n{}",
        outcome.result.source_after
    );
    assert_eq!(
        hard_error_count(&outcome.result.source_after),
        0,
        "re-skinned document must validate clean; got:\n{}",
        outcome.result.source_after
    );
}

// ── Missing tokens are created ────────────────────────────────────────────────

#[test]
fn tokens_absent_from_doc_are_created() {
    let outcome = apply_run(None, "cobalt", MIXED_DOC).expect("cobalt pack must resolve");

    assert!(
        outcome
            .added_tokens
            .contains(&"color.secondary".to_string()),
        "color.secondary must be reported as added; got: {:?}",
        outcome.added_tokens
    );
    assert!(
        outcome
            .result
            .source_after
            .contains("id=\"color.secondary\""),
        "created token must appear in source_after; got:\n{}",
        outcome.result.source_after
    );
}

// ── Doc-only tokens are left alone ────────────────────────────────────────────

#[test]
fn doc_only_token_is_untouched() {
    let outcome = apply_run(None, "cobalt", MIXED_DOC).expect("cobalt pack must resolve");

    assert!(
        outcome
            .result
            .source_after
            .contains("id=\"color.custom\" type=\"color\" value=\"#222222\""),
        "doc-only token must be byte-identical; got:\n{}",
        outcome.result.source_after
    );
    assert!(
        !outcome.added_tokens.contains(&"color.custom".to_string()),
        "doc-only token must not be reported as added"
    );
}

// ── Type mismatch is skipped, the rest of the transaction still applies ─────

#[test]
fn type_mismatch_is_skipped_while_rest_applies() {
    let outcome = apply_run(None, "cobalt", MIXED_DOC).expect("cobalt pack must resolve");

    // radius.box (doc: color, theme: dimension) must be reported as skipped
    // and left with its original (doc-side) literal value.
    let skip = outcome
        .skipped
        .iter()
        .find(|s| s.id == "radius.box")
        .expect("radius.box must be reported as skipped");
    assert_eq!(skip.doc_type.as_deref(), Some("color"));
    assert_eq!(skip.theme_type, "dimension");
    assert!(
        outcome
            .result
            .source_after
            .contains("id=\"radius.box\" type=\"color\" value=\"#111111\""),
        "mismatched token must be untouched; got:\n{}",
        outcome.result.source_after
    );

    // color.primary (matching type) must still have been updated in the same
    // transaction — one skip never blocks the rest of the re-skin.
    assert!(
        outcome.result.source_after.contains("#605dff"),
        "color.primary must still be updated to cobalt's value; got:\n{}",
        outcome.result.source_after
    );
    assert_eq!(
        outcome.exit_code, 0,
        "partial skip must not reject the transaction"
    );
}

// ── Unknown pack ──────────────────────────────────────────────────────────────

#[test]
fn unknown_pack_exits_2_and_lists_available_names() {
    let err = apply_run(None, "not-a-real-theme", MIXED_DOC)
        .expect_err("an unknown theme must be rejected");

    assert_eq!(err.exit_code, 2, "unknown pack must exit 2");
    assert!(
        err.message.contains("unknown theme"),
        "message must say 'unknown theme'; got: {}",
        err.message
    );
    assert!(
        err.message.contains("cobalt") && err.message.contains("sunset"),
        "message must list available theme names; got: {}",
        err.message
    );
}

// ── --json is a superset of the tx schema ────────────────────────────────────

#[test]
fn json_output_is_tx_schema_superset() {
    let outcome = apply_run(None, "cobalt", MIXED_DOC).expect("cobalt pack must resolve");

    let value: serde_json::Value =
        serde_json::from_str(&outcome.json_str).expect("json_str must be valid JSON");
    assert_eq!(value["schema"], "zenith-tx-v1");
    assert!(value["status"].is_string());
    assert!(value["diagnostics"].is_array());
    assert!(value["changed"].is_boolean());

    let added = value["added_tokens"]
        .as_array()
        .expect("added_tokens must be a JSON array");
    assert!(
        added.iter().any(|v| v.as_str() == Some("color.secondary")),
        "added_tokens must list color.secondary; got: {:?}",
        added
    );

    let skipped = value["skipped_token_mismatches"]
        .as_array()
        .expect("skipped_token_mismatches must be a JSON array");
    assert!(
        skipped.iter().any(|v| v["id"] == "radius.box"),
        "skipped_token_mismatches must list radius.box; got: {:?}",
        skipped
    );
}

// ── --apply persists; a second dry-run shows no further changes ─────────────

#[test]
fn apply_persists_and_second_dry_run_shows_no_changes() {
    let tmp = TempDir::new().unwrap();
    let (path, src) = sunset_doc(&tmp);

    let first = apply_run(path.parent(), "cobalt", &src).expect("cobalt pack must resolve");
    assert_ne!(first.result.source_before, first.result.source_after);

    // Persist the re-skinned source (mirrors the CLI's `--apply` write).
    std::fs::write(&path, first.result.source_after.as_bytes())
        .expect("persisting the re-skinned source must succeed");

    let persisted = std::fs::read_to_string(&path).expect("persisted file must be readable");
    let second =
        apply_run(path.parent(), "cobalt", &persisted).expect("cobalt pack must resolve again");

    assert_eq!(
        second.result.source_before, second.result.source_after,
        "re-applying the same theme must be a no-op"
    );
    assert!(second.added_tokens.is_empty());
    assert!(second.skipped.is_empty());
    assert_eq!(second.exit_code, 0);
}

// ── Sanity: MIXED_DOC parses and the fixture setup is itself valid KDL ──────

#[test]
fn mixed_doc_fixture_parses() {
    KdlAdapter
        .parse(MIXED_DOC.as_bytes())
        .expect("MIXED_DOC fixture must parse");
}

// ── A PROJECT pack (not `@zenith/theme.*`) is a valid `theme apply` target ──

/// A minimal project pack, self-identified via its `libraries` self-entry as
/// `@acme/brand` (not a `@zenith/theme.*` preset), carrying a handful of color
/// tokens. `theme apply` is not theme-preset-specific: any pack id carrying a
/// `tokens` block should be mergeable this way.
const ACME_PACK_SRC: &str = r##"zenith version=1 {
  project id="@acme/brand" name="Acme Brand"
  libraries {
    library id="@acme/brand" version="1.0.0"
  }
  tokens format="zenith-token-v1" {
    token id="color.primary" type="color" value="#1d4ed8"
    token id="color.secondary" type="color" value="#7c3aed"
    token id="color.accent" type="color" value="#f59e0b"
  }
  styles {}
  document id="acme.preview" title="Acme brand preview" {
    page id="pg" w=(px)200 h=(px)200 {}
  }
}
"##;

const ACME_TARGET_DOC: &str = r##"zenith version=1 {
  project id="proj.acme.target" name="Target"
  tokens format="zenith-token-v1" {
    token id="color.primary" type="color" value="#000000"
  }
  styles {}
  document id="doc.acme.target" title="Target" {
    page id="pg" w=(px)400 h=(px)300 {}
  }
}
"##;

#[test]
fn project_pack_tokens_land_with_pack_id_set_provenance() {
    let tmp = TempDir::new().unwrap();
    let lib_dir = tmp.path().join("libraries");
    std::fs::create_dir_all(&lib_dir).expect("create libraries dir");
    std::fs::write(lib_dir.join("acme.zen"), ACME_PACK_SRC).expect("write project pack");

    let outcome =
        apply_run(Some(tmp.path()), "@acme/brand", ACME_TARGET_DOC).expect("acme pack resolves");

    assert_eq!(outcome.exit_code, 0, "accepted re-skin must exit 0");

    // `color.primary` already exists in the target → updated in place, with
    // `set` stamped to the project pack's own id.
    assert!(
        outcome
            .result
            .source_after
            .contains("id=\"color.primary\" type=\"color\" set=\"@acme/brand\" value=\"#1d4ed8\""),
        "existing token must be updated with the project pack's set provenance; got:\n{}",
        outcome.result.source_after
    );

    // `color.secondary` / `color.accent` are absent from the target → created,
    // also stamped with the project pack's id.
    assert!(
        outcome
            .added_tokens
            .contains(&"color.secondary".to_string()),
        "color.secondary must be reported as added; got: {:?}",
        outcome.added_tokens
    );
    assert!(
        outcome.added_tokens.contains(&"color.accent".to_string()),
        "color.accent must be reported as added; got: {:?}",
        outcome.added_tokens
    );
    assert!(
        outcome.result.source_after.contains(
            "id=\"color.secondary\" type=\"color\" set=\"@acme/brand\" value=\"#7c3aed\""
        ),
        "created token must carry the project pack's set provenance; got:\n{}",
        outcome.result.source_after
    );
    assert!(
        outcome
            .result
            .source_after
            .contains("id=\"color.accent\" type=\"color\" set=\"@acme/brand\" value=\"#f59e0b\""),
        "created token must carry the project pack's set provenance; got:\n{}",
        outcome.result.source_after
    );

    assert_eq!(
        hard_error_count(&outcome.result.source_after),
        0,
        "re-skinned document must validate clean; got:\n{}",
        outcome.result.source_after
    );
}
