//! `zenith tx` review output: a dry-run prints the source diff and the
//! moved/resized boxes; `--apply` hides them unless `--diff` is set.

mod common;

use common::{CARDS, DOC, Env, INTO_ROW, json, stdout};

#[test]
fn dry_run_shows_diff_and_moved_line() {
    let env = Env::with_doc(CARDS);
    let before = env.read();
    let out = env.tx(INTO_ROW, &[]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("--- a/t.zen\n+++ b/t.zen"), "{text}");
    assert!(text.contains("\nboxes:"), "{text}");
    assert!(
        text.contains("  moved card.3: (526,40 219x120) -> ("),
        "{text}"
    );
    assert!(text.contains("  moved c1: "), "{text}");
    assert_eq!(env.read(), before, "a dry-run writes nothing");
}

#[test]
fn dry_run_json_has_source_diff_and_sorted_boxes() {
    let env = Env::with_doc(CARDS);
    let v = json(&env.tx(INTO_ROW, &["--json"]));
    let diff = v["source_diff"].as_str().expect("source_diff");
    assert!(diff.starts_with("--- a/t.zen"), "{diff}");
    let boxes = v["boxes"].as_array().expect("boxes");
    let ids: Vec<&str> = boxes.iter().filter_map(|b| b["id"].as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "boxes are sorted by id");
    let card = boxes
        .iter()
        .find(|b| b["id"] == "card.3")
        .expect("card.3 moved");
    assert_eq!(card["before"]["x"], 526.0);
    assert_eq!(card["before"]["w"], 219.0);
    assert!(card["after"]["y"].as_f64().is_some(), "{card}");
}

#[test]
fn apply_hides_diff_and_boxes() {
    let env = Env::with_doc(CARDS);
    let out = env.tx(INTO_ROW, &["--apply"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(!text.contains("--- a/"), "{text}");
    assert!(!text.contains("boxes:"), "{text}");

    let env = Env::with_doc(CARDS);
    let v = json(&env.tx(INTO_ROW, &["--apply", "--json"]));
    assert!(v.get("source_diff").is_none(), "{v:#}");
    assert!(v.get("boxes").is_none(), "{v:#}");
}

#[test]
fn apply_with_diff_shows_them() {
    let env = Env::with_doc(CARDS);
    let out = env.tx(INTO_ROW, &["--apply", "--diff"]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("--- a/t.zen"), "{text}");
    assert!(text.contains("  moved card.3: "), "{text}");
    assert!(env.read().contains("layout=\"row\""));
}

#[test]
fn dry_run_says_nothing_was_written() {
    let env = Env::with_doc(CARDS);
    let text = stdout(&env.tx(INTO_ROW, &[]));
    assert!(
        text.contains("applied: false (dry run, nothing written; add --apply to write the file)"),
        "{text}"
    );
    let v = json(&env.tx(INTO_ROW, &["--json"]));
    assert_eq!(v["applied"], false, "{v:#}");
    assert_eq!(v["changed"], true, "{v:#}");
}

#[test]
fn apply_says_it_wrote() {
    let env = Env::with_doc(CARDS);
    let text = stdout(&env.tx(INTO_ROW, &["--apply"]));
    assert!(text.contains("applied: true\n"), "{text}");
    let env = Env::with_doc(CARDS);
    let v = json(&env.tx(INTO_ROW, &["--apply", "--json"]));
    assert_eq!(v["applied"], true, "{v:#}");
}

#[test]
fn rejected_and_unchanged_are_not_applied() {
    let env = Env::with_doc(CARDS);
    let v = json(&env.tx(
        r#"{"ops":[{"op":"set_fill","node":"nope","fill":"color.w"}]}"#,
        &["--apply", "--json"],
    ));
    assert_eq!(v["applied"], false, "{v:#}");
    let text = stdout(&env.tx(
        r#"{"ops":[{"op":"set_fill","node":"card.3","fill":"color.k"}]}"#,
        &["--apply"],
    ));
    assert!(
        text.contains("applied: false\n"),
        "unchanged has no dry-run hint: {text}"
    );
}

#[test]
fn inline_ops_match_the_file_form() {
    let env = Env::with_doc(CARDS);
    let body = r#"{"op":"set_fill","node":"card.3","fill":"color.w"}"#;
    let wrapped = env.zenith(&[
        "tx",
        DOC,
        "--ops",
        &format!(r#"{{"ops":[{body}]}}"#),
        "--json",
    ]);
    let bare = env.zenith(&["tx", DOC, "--ops", &format!("[{body}]"), "--json"]);
    assert_eq!(wrapped.status.code(), Some(0));
    assert_eq!(json(&wrapped), json(&bare));
    assert_eq!(json(&bare)["changed"], true);
}

#[test]
fn inline_ops_apply_writes_the_file() {
    let env = Env::with_doc(CARDS);
    let before = env.read();
    let out = env.zenith(&[
        "tx",
        "t.zen",
        "--apply",
        "--ops",
        r#"[{"op":"set_fill","node":"card.3","fill":"color.w"}]"#,
    ]);
    assert_eq!(out.status.code(), Some(0));
    assert_ne!(env.read(), before);
}

#[test]
fn tx_needs_a_file_or_inline_ops_and_not_both() {
    let env = Env::with_doc(CARDS);
    assert_eq!(env.zenith(&["tx", DOC]).status.code(), Some(2));
    let both = env.zenith(&["tx", DOC, "ops.json", "--ops", "[]"]);
    assert_eq!(both.status.code(), Some(2));
}

#[test]
fn no_geometry_op_has_empty_boxes() {
    let env = Env::with_doc(CARDS);
    let out = env.tx(
        r#"{"ops":[{"op":"set_fill","node":"card.3","fill":"color.w"}]}"#,
        &["--json"],
    );
    assert_eq!(out.status.code(), Some(0));
    let v = json(&out);
    assert_eq!(v["changed"], true);
    assert!(
        v["source_diff"]
            .as_str()
            .is_some_and(|d| d.contains("color.w")),
        "{v:#}"
    );
    assert_eq!(v["boxes"], serde_json::json!([]));

    let text = stdout(&env.tx(
        r#"{"ops":[{"op":"set_fill","node":"card.3","fill":"color.w"}]}"#,
        &[],
    ));
    assert!(text.contains("boxes: (none)"), "{text}");
}

#[test]
fn rejected_or_unchanged_has_no_review() {
    let env = Env::with_doc(CARDS);
    let out = env.tx(
        r#"{"ops":[{"op":"set_fill","node":"nope","fill":"color.w"}]}"#,
        &["--json"],
    );
    assert_eq!(out.status.code(), Some(1));
    let v = json(&out);
    assert!(v.get("source_diff").is_none(), "{v:#}");
    assert!(v.get("boxes").is_none(), "{v:#}");

    let out = env.tx(
        r#"{"ops":[{"op":"set_fill","node":"card.3","fill":"color.k"}]}"#,
        &["--json"],
    );
    let v = json(&out);
    assert_eq!(v["changed"], false, "{v:#}");
    assert!(v.get("source_diff").is_none(), "{v:#}");
    assert!(v.get("boxes").is_none(), "{v:#}");
}

#[test]
fn help_names_the_real_review() {
    let env = Env::with_doc(CARDS);
    for args in [&["--help"][..], &["tx", "--help"][..]] {
        let text = stdout(&env.zenith(args));
        assert!(!text.contains("scene diff"), "{args:?}: {text}");
    }
    let text = stdout(&env.zenith(&["tx", "--help"]));
    assert!(
        text.contains("source diff and moved/resized node boxes"),
        "{text}"
    );
    assert!(text.contains("--diff"), "{text}");
}

// `zenith tx --help` lists common edits as runnable JSON. Each op in that list
// must exist.

fn common_edits(help: &str) -> Vec<String> {
    help.lines()
        .skip_while(|l| !l.starts_with("COMMON EDITS"))
        .skip(1)
        .take_while(|l| !l.trim().is_empty())
        .filter_map(|l| {
            let start = l.find('{')?;
            let end = l.rfind('}')?;
            Some(l[start..=end].to_owned())
        })
        .collect()
}

#[test]
fn every_common_edit_names_a_real_op() {
    let env = Env::with_doc(CARDS);
    let help = stdout(&env.zenith(&["tx", "--help"]));
    let edits = common_edits(&help);
    assert!(
        edits.len() >= 8,
        "common edits missing from tx --help:\n{help}"
    );
    for edit in edits {
        let v: serde_json::Value = serde_json::from_str(&edit)
            .unwrap_or_else(|e| panic!("example is not JSON ({e}): {edit}"));
        let op = v["op"].as_str().expect("op name");
        let out = env.zenith(&["schema", "op", op]);
        assert_eq!(out.status.code(), Some(0), "unknown op {op} in: {edit}");
        let parsed = zenith_tx::Transaction::from_json(&format!("{{\"ops\":[{edit}]}}"));
        assert!(
            parsed.is_ok(),
            "fields do not match the op {op}: {edit}: {:?}",
            parsed.err().map(|e| e.message)
        );
    }
}

#[test]
fn op_is_an_alias_of_ops() {
    let env = Env::with_doc(CARDS);
    let out = env.zenith(&[
        "tx",
        "t.zen",
        "--op",
        r#"[{"op":"set_fill","node":"card.3","fill":"color.w"}]"#,
        "--json",
    ]);
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn tx_reports_a_warning_the_edit_introduces() {
    let env = Env::with_doc(CARDS);
    let v = json(&env.tx(
        r#"{"ops":[{"op":"set_geometry","node":"card.3","x":10}]}"#,
        &["--json"],
    ));
    let codes: Vec<&str> = v["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .filter_map(|d| d["code"].as_str())
        .collect();
    assert!(codes.contains(&"layout.block_overlap"), "{codes:?}");
    // A warning the input already had is not repeated by an unrelated edit.
    let again = json(&env.tx(
        r#"{"ops":[{"op":"set_fill","node":"card.3","fill":"color.w"}]}"#,
        &["--json"],
    ));
    let again_codes: Vec<&str> = again["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .filter_map(|d| d["code"].as_str())
        .collect();
    assert!(
        !again_codes.contains(&"layout.block_overlap"),
        "{again_codes:?}"
    );
}

/// A text node that must fit a 100x24 box and has room only for a short line.
const FIT_DOC: &str = r##"zenith version=1 {
  project id="p" name="P"
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
    token id="font.body" type="fontFamily" value="Noto Sans"
    token id="size.big" type="dimension" value=(px)20
  }
  styles {}
  document id="d" title="T" {
    page id="pg" w=(px)400 h=(px)200 {
      text id="t" x=(px)10 y=(px)10 w=(px)100 h=(px)24 overflow="fit" fill=(token)"color.k" font-family=(token)"font.body" font-size=(token)"size.big" { span "Hi" }
    }
  }
}
"##;

#[test]
fn an_edit_that_breaks_the_render_is_rejected_and_not_written() {
    let env = Env::with_doc(FIT_DOC);
    let before = env.read();
    let ops = r#"{"ops":[{"op":"set_span_text","node":"t","span":0,"text":"A much longer headline that cannot fit in one small box at all"}]}"#;
    let out = env.tx(ops, &["--apply", "--json"]);
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    let v = json(&out);
    assert_eq!(v["status"], "rejected");
    assert_eq!(v["applied"], false);
    let codes: Vec<&str> = v["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .filter_map(|d| d["code"].as_str())
        .collect();
    assert!(codes.contains(&"text.fit_failed"), "{codes:?}");
    assert_eq!(env.read(), before, "a rejected edit writes nothing");
}

#[test]
fn tx_diagnostics_are_not_repeated() {
    let env = Env::with_doc(common::CARDS);
    let v = json(&env.tx(
        r#"{"ops":[{"op":"set_geometry","node":"card.3","x":10}]}"#,
        &["--json"],
    ));
    let mut seen = std::collections::BTreeSet::new();
    for d in v["diagnostics"].as_array().expect("diagnostics") {
        let key = (
            d["code"].as_str().unwrap_or_default().to_owned(),
            d["subject_id"].as_str().unwrap_or_default().to_owned(),
            d["message"].as_str().unwrap_or_default().to_owned(),
        );
        assert!(seen.insert(key.clone()), "repeated diagnostic {key:?}");
    }
}

// ── `--apply --png`: the edit and its render in one call ──────────────────

const SET_FILL: &str = r#"{"ops":[{"op":"set_fill","node":"card.3","fill":"color.w"}]}"#;

fn is_png(env: &Env, name: &str) -> bool {
    std::fs::read(env.path(name)).is_ok_and(|b| b.starts_with(b"\x89PNG\r\n\x1a\n"))
}

#[test]
fn apply_with_png_writes_the_edit_and_the_render() {
    let env = Env::with_doc(CARDS);
    let before = env.read();
    let out = env.tx(SET_FILL, &["--apply", "--png", "c.png"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert_ne!(env.read(), before, "the edit is written");
    assert!(is_png(&env, "c.png"), "the render is a PNG");
    let text = stdout(&out);
    assert!(text.contains("png: c.png ("), "{text}");
    assert!(text.contains("page 1 of "), "{text}");
}

#[test]
fn json_reports_the_render() {
    let env = Env::with_doc(CARDS);
    let v = json(&env.tx(SET_FILL, &["--apply", "--png", "c.png", "--json"]));
    assert_eq!(v["png"]["path"], "c.png");
    assert_eq!(v["png"]["page"], 1);
    assert!(v["png"]["width"].as_u64().is_some_and(|w| w > 0), "{v}");
    assert!(v["png"]["pages"].as_u64().is_some_and(|n| n >= 1), "{v}");
}

#[test]
fn png_needs_apply() {
    let env = Env::with_doc(CARDS);
    let out = env.tx(SET_FILL, &["--png", "c.png"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(!is_png(&env, "c.png"));
}

#[test]
fn a_rejected_edit_writes_neither_file() {
    let env = Env::with_doc(CARDS);
    let before = env.read();
    let out = env.tx(
        r#"{"ops":[{"op":"set_fill","node":"no-such-node","fill":"color.w"}]}"#,
        &["--apply", "--png", "c.png"],
    );
    assert_eq!(out.status.code(), Some(1), "{}", stdout(&out));
    assert_eq!(env.read(), before);
    assert!(!is_png(&env, "c.png"));
}

#[test]
fn png_may_not_overwrite_the_document() {
    let env = Env::with_doc(CARDS);
    let before = env.read();
    let out = env.tx(SET_FILL, &["--apply", "--png", DOC]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        env.read(),
        before,
        "the edit is refused before it is written"
    );
}

#[test]
fn a_page_that_does_not_render_writes_neither_file() {
    let env = Env::with_doc(CARDS);
    let before = env.read();
    let out = env.tx(SET_FILL, &["--apply", "--png", "c.png", "--page", "9"]);
    assert_eq!(out.status.code(), Some(2), "{}", stdout(&out));
    assert_eq!(env.read(), before, "the edit is not written");
    assert!(!is_png(&env, "c.png"));
    let text = stdout(&out);
    assert!(text.contains("tx.png_failed"), "{text}");
    assert!(text.contains("neither the edit nor the PNG"), "{text}");
}

#[test]
fn a_png_failure_in_json_mode_is_one_object() {
    let env = Env::with_doc(CARDS);
    let out = env.tx(
        SET_FILL,
        &["--apply", "--png", "c.png", "--page", "9", "--json"],
    );
    assert_eq!(out.status.code(), Some(2));
    let v = json(&out);
    assert!(
        v["png_error"]
            .as_str()
            .is_some_and(|m| m.contains("tx.png_failed")),
        "{v}"
    );
    assert!(v.get("png").is_none(), "{v}");
}

#[test]
fn page_zero_is_a_usage_error_and_writes_nothing() {
    let env = Env::with_doc(CARDS);
    let before = env.read();
    let out = env.tx(SET_FILL, &["--apply", "--png", "c.png", "--page", "0"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(env.read(), before);
}

#[test]
fn png_may_not_overwrite_the_tx_file_or_a_directory() {
    let env = Env::with_doc(CARDS);
    let before = env.read();
    let tx_file = env.tx(SET_FILL, &["--apply", "--png", "tx.json"]);
    assert_eq!(tx_file.status.code(), Some(2));
    assert_eq!(env.read(), before, "refused before the edit is written");
    let missing = env.tx(SET_FILL, &["--apply", "--png", "no-such-dir/c.png"]);
    assert_eq!(missing.status.code(), Some(2));
    assert_eq!(
        env.read(),
        before,
        "a missing directory is refused up front"
    );
    std::fs::create_dir(env.path("outdir")).expect("mkdir");
    let dir = env.tx(SET_FILL, &["--apply", "--png", "outdir"]);
    assert_eq!(dir.status.code(), Some(2));
    assert_eq!(env.read(), before, "refused before the edit is written");
}

#[test]
fn a_partial_fill_does_not_report_the_placeholder_warning_as_introduced() {
    let dir = tempfile::tempdir().expect("tempdir");
    let data = tempfile::tempdir().expect("data");
    let zenith = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_zenith"))
            .args(args)
            .current_dir(dir.path())
            .env("ZENITH_DATA_DIR", data.path())
            .output()
            .expect("run zenith")
    };
    assert!(
        zenith(&["new", "c.zen", "--starter", "card"])
            .status
            .success()
    );
    let out = zenith(&[
        "tx",
        "c.zen",
        "--apply",
        "--json",
        "--ops",
        r#"[{"op":"set_span_text","node":"card.name","span":0,"text":"Real"}]"#,
    ]);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    let n = v["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .filter(|d| d["code"] == "content.placeholder")
        .count();
    assert_eq!(n, 1, "one placeholder warning, listed once: {v}");
    assert_eq!(v["status"], "accepted_with_warnings");
}
