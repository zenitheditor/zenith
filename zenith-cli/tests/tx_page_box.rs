//! `tx.page_box_changed`: reparent, group, ungroup, and detach_anchor warn
//! when a subject's compiled page box changes. The warning sets the status,
//! keeps exit 0, and does not stop `--apply`. A later op that edits the
//! subject suppresses the warning.

mod common;

use common::{CARDS, Env, INTO_ROW, json, stdout};
use serde_json::Value;

fn diags_with<'a>(v: &'a Value, code: &str) -> Vec<&'a Value> {
    v["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .filter(|d| d["code"] == code)
        .collect()
}

#[test]
fn reparent_into_row_warns_with_flow_cause() {
    let env = Env::with_doc(CARDS);
    let out = env.tx(INTO_ROW, &["--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let v = json(&out);
    assert_eq!(v["status"], "accepted_with_warnings");

    let warnings = diags_with(&v, "tx.page_box_changed");
    assert_eq!(warnings.len(), 1, "{v:#}");
    let w = warnings[0];
    assert_eq!(w["severity"], "warning");
    assert_eq!(w["subject_id"], "card.3");
    let msg = w["message"].as_str().expect("message");
    assert!(
        msg.starts_with("reparent: node \"card.3\" page box changed from (526,40 219x120) to ("),
        "{msg}"
    );
    assert!(msg.contains("reflowed siblings: c1, c2"), "{msg}");
    let cause = w["cause"].as_str().expect("cause");
    assert!(
        cause.contains("is placed by layout frame \"cards.row2\" (row)"),
        "{cause}"
    );
    assert!(
        diags_with(&v, "tx.flow_placed").is_empty(),
        "flow_placed must move into the cause: {v:#}"
    );
}

#[test]
fn human_output_shows_warning_and_cause() {
    let env = Env::with_doc(CARDS);
    let out = env.tx(INTO_ROW, &[]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    assert!(text.contains("status: accepted (with warnings)"), "{text}");
    assert!(
        text.contains("warning[tx.page_box_changed] (card.3): reparent:"),
        "{text}"
    );
    assert!(
        text.contains("    cause: reparent: node \"card.3\" is placed by layout frame"),
        "{text}"
    );
}

#[test]
fn apply_still_writes_with_the_warning() {
    let env = Env::with_doc(CARDS);
    let before = env.read();
    let out = env.tx(INTO_ROW, &["--apply", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let v = json(&out);
    assert_eq!(v["status"], "accepted_with_warnings");
    assert_eq!(diags_with(&v, "tx.page_box_changed").len(), 1);
    let after = env.read();
    assert_ne!(before, after, "--apply must write");
    let row = after.find("id=\"cards.row2\"").expect("row");
    let card = after.find("id=\"card.3\"").expect("card");
    let c1 = after.find("id=\"c1\"").expect("c1");
    assert!(
        row < card && card < c1,
        "card.3 must be the row's first child"
    );
}

#[test]
fn reparent_into_plain_frame_keeps_box_and_does_not_warn() {
    let env = Env::with_doc(CARDS);
    let out = env.tx(
        r#"{"ops":[{"op":"reparent","node":"card.3","new_parent":"free"}]}"#,
        &["--json"],
    );
    assert_eq!(out.status.code(), Some(0));
    let v = json(&out);
    assert_eq!(v["status"], "accepted", "{v:#}");
    assert!(diags_with(&v, "tx.page_box_changed").is_empty(), "{v:#}");
}

#[test]
fn non_structural_op_does_not_warn() {
    let env = Env::with_doc(CARDS);
    let out = env.tx(
        r#"{"ops":[{"op":"set_geometry","node":"card.3","x":10}]}"#,
        &["--json"],
    );
    assert_eq!(out.status.code(), Some(0));
    let v = json(&out);
    assert_eq!(v["status"], "accepted", "{v:#}");
    assert!(diags_with(&v, "tx.page_box_changed").is_empty(), "{v:#}");
}

#[test]
fn group_outside_flow_keeps_box_and_does_not_warn() {
    let env = Env::with_doc(CARDS);
    let out = env.tx(
        r#"{"ops":[{"op":"group","node_ids":["f1"],"group_id":"solo"}]}"#,
        &["--json"],
    );
    assert_eq!(out.status.code(), Some(0));
    let v = json(&out);
    assert_eq!(v["status"], "accepted", "{v:#}");
    assert_eq!(v["changed"], true);
    assert!(diags_with(&v, "tx.page_box_changed").is_empty(), "{v:#}");
}

#[test]
fn later_edit_of_the_subject_does_not_warn() {
    let env = Env::with_doc(CARDS);
    let out = env.tx(
        r#"{"ops":[{"op":"reparent","node":"card.3","new_parent":"free"},{"op":"nudge_geometry","node":"card.3","dx":12}]}"#,
        &["--json"],
    );
    assert_eq!(out.status.code(), Some(0));
    let v = json(&out);
    assert_eq!(v["status"], "accepted", "{v:#}");
    assert!(diags_with(&v, "tx.page_box_changed").is_empty(), "{v:#}");
}

#[test]
fn detach_anchor_keeps_box_and_does_not_warn() {
    let env = Env::with_doc(&CARDS.replace(
        r#"rect id="card.3" x=(px)526 y=(px)40"#,
        r#"rect id="card.3" anchor="top-right""#,
    ));
    let out = env.tx(
        r#"{"ops":[{"op":"detach_anchor","node":"card.3"}]}"#,
        &["--json"],
    );
    assert_eq!(out.status.code(), Some(0));
    let v = json(&out);
    assert_eq!(v["status"], "accepted", "{v:#}");
    assert!(diags_with(&v, "tx.page_box_changed").is_empty(), "{v:#}");
}
