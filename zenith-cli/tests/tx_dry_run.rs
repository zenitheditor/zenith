//! `zenith tx` review output: a dry-run prints the source diff and the
//! moved/resized boxes; `--apply` hides them unless `--diff` is set.

mod common;

use common::{CARDS, Env, INTO_ROW, json, stdout};

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
