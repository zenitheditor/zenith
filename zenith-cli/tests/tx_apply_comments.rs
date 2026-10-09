//! `zenith tx --apply` patches the source in place: comments and layout
//! survive a property edit, and the history `doc-id` stamp touches only the
//! root line. Structural edits (add, remove, reparent) patch in place too.

mod common;

use common::{CARDS, Env, INTO_ROW, json, stdout};

const COMMENTED: &str = r##"zenith version=1 {
  // Brand palette.
  tokens format="zenith-token-v1" {
    token id="c.a" type="color" value="#112233" // primary
    token id="c.b" type="color" value="#445566"
  }
  styles {}

  /* The only document. */
  document id="d" {
    page id="p" w=(px)400 h=(px)300 {
        // Hand-indented on purpose.
        rect id="box" x=(px)10 y=(px)10 w=(px)100 h=(px)50 fill=(token)"c.a" // the box
        rect id="other" x=(px)10 y=(px)100 w=(px)100 h=(px)50 fill=(token)"c.a"
    }
  }
}
"##;

const SET_FILL: &str = r#"{"ops":[{"op":"set_fill","node":"box","fill":"c.b"}]}"#;

#[test]
fn apply_keeps_comments_and_layout() {
    let env = Env::with_doc(COMMENTED);
    let out = env.tx(SET_FILL, &["--apply"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let written = env.read();

    let edited = r#"        rect id="box" x=(px)10 y=(px)10 w=(px)100 h=(px)50 fill=(token)"c.b" // the box"#;
    assert!(written.contains(edited), "{written}");

    // First edit: history stamps a `doc-id` on the root line only.
    let mut lines = written.lines();
    let root = lines.next().expect("root line");
    assert!(
        root.starts_with("zenith version=1 doc-id=\"") && root.ends_with("\" {"),
        "{root}"
    );
    let rest: Vec<&str> = lines.collect();
    let expected: Vec<String> = COMMENTED
        .lines()
        .skip(1)
        .map(|l| {
            l.replace(
                r#"fill=(token)"c.a" // the box"#,
                r#"fill=(token)"c.b" // the box"#,
            )
        })
        .collect();
    assert_eq!(rest, expected, "{written}");
}

#[test]
fn second_apply_leaves_the_doc_id_alone() {
    let env = Env::with_doc(COMMENTED);
    assert_eq!(env.tx(SET_FILL, &["--apply"]).status.code(), Some(0));
    let first = env.read();
    let out = env.tx(
        r#"{"ops":[{"op":"set_opacity","node":"other","opacity":0.5}]}"#,
        &["--apply"],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let second = env.read();
    let changed: Vec<(&str, &str)> = first
        .lines()
        .zip(second.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(changed.len(), 1, "{second}");
    assert!(changed[0].1.contains("opacity=0.5"), "{second}");
}

#[test]
fn dry_run_diff_shows_only_the_edited_line() {
    let env = Env::with_doc(COMMENTED);
    let out = env.tx(SET_FILL, &[]);
    assert_eq!(out.status.code(), Some(0));
    let text = stdout(&out);
    let removed: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with('-') && !l.starts_with("---"))
        .collect();
    let added: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
        .collect();
    assert_eq!(removed.len(), 1, "{text}");
    assert_eq!(added.len(), 1, "{text}");
    assert!(
        added[0].ends_with(r#"fill=(token)"c.b" // the box"#),
        "{text}"
    );
    assert_eq!(env.read(), COMMENTED, "a dry-run writes nothing");
}

#[test]
fn dry_run_json_diff_keeps_comment_context() {
    let env = Env::with_doc(COMMENTED);
    let v = json(&env.tx(SET_FILL, &["--json"]));
    let diff = v["source_diff"].as_str().expect("source_diff");
    assert!(diff.contains(" // the box"), "{diff}");
    assert!(
        diff.contains("        // Hand-indented on purpose."),
        "{diff}"
    );
}

/// The written file without its root line, which carries the `doc-id` stamp.
fn body(text: &str) -> Vec<&str> {
    text.lines().skip(1).collect()
}

#[test]
fn remove_apply_keeps_the_other_comments() {
    let env = Env::with_doc(COMMENTED);
    let out = env.tx(
        r#"{"ops":[{"op":"remove_node","node":"other"}]}"#,
        &["--apply"],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let written = env.read();
    let expected = COMMENTED.replace(
        "        rect id=\"other\" x=(px)10 y=(px)100 w=(px)100 h=(px)50 fill=(token)\"c.a\"\n",
        "",
    );
    assert_eq!(body(&written), body(&expected), "{written}");
}

#[test]
fn add_apply_keeps_comments_and_hand_indentation() {
    let env = Env::with_doc(COMMENTED);
    let out = env.tx(
        r#"{"ops":[{"op":"add_node","parent":"p","source":"rect id=\"new\" x=(px)1 y=(px)1 w=(px)2 h=(px)2"}]}"#,
        &["--apply"],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let written = env.read();
    let other =
        "        rect id=\"other\" x=(px)10 y=(px)100 w=(px)100 h=(px)50 fill=(token)\"c.a\"\n";
    let expected = COMMENTED.replace(
        other,
        &format!("{other}        rect id=\"new\" x=(px)1 y=(px)1 w=(px)2 h=(px)2\n"),
    );
    assert_eq!(body(&written), body(&expected), "{written}");
}

#[test]
fn reparent_apply_keeps_comments() {
    let env = Env::with_doc(&format!("// Cards.\n{CARDS}"));
    let out = env.tx(INTO_ROW, &["--apply"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let written = env.read();
    assert!(written.starts_with("// Cards.\n"), "{written}");
    assert!(
        written.contains("  styles { }\n"),
        "untouched layout:\n{written}"
    );
    let row = written.find("id=\"cards.row2\"").expect("row");
    let card = written.find("id=\"card.3\"").expect("card");
    let c1 = written.find("id=\"c1\"").expect("c1");
    assert!(
        row < card && card < c1,
        "card.3 must lead the row:\n{written}"
    );
}
