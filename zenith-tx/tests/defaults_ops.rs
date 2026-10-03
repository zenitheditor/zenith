//! Integration tests for the `set_default` and `remove_default` transaction ops.

mod common;
use common::*;
use zenith_core::{DefaultsKind, Document, KdlAdapter, KdlSource};
use zenith_tx::{Op, Permissions, Transaction, TxResult, TxStatus, run_transaction};

const DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="size.md" type="dimension" value=(px)16
  }
  styles {
    style id="box" {
      font-size (token)"size.md"
    }
    style id="body" {
      font-size (token)"size.md"
    }
    style id="box.label" {
      font-size (token)"size.md"
    }
  }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
    page id="pg2" w=(px)400 h=(px)300 {
      rect id="r2" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}"##;

fn set(page: Option<&str>, kind: &str, style: &str, text_style: Option<Option<&str>>) -> Op {
    Op::SetDefault {
        page: page.map(str::to_owned),
        kind: kind.to_owned(),
        style: style.to_owned(),
        text_style: text_style.map(|t| t.map(str::to_owned)),
    }
}

fn remove(page: Option<&str>, kind: &str) -> Op {
    Op::RemoveDefault {
        page: page.map(str::to_owned),
        kind: kind.to_owned(),
    }
}

fn run(src: &str, ops: Vec<Op>) -> TxResult {
    let doc = parse(src);
    let tx = Transaction {
        ops,
        permissions: Permissions::default(),
    };
    run_transaction(&doc, &tx).expect("run_transaction must not error")
}

fn assert_accepted(r: &TxResult) {
    assert_eq!(
        r.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        r.diagnostics
    );
}

fn assert_rejected(r: &TxResult, code: &str, needle: &str) {
    assert_eq!(r.status, TxStatus::Rejected, "expected Rejected");
    let d = r
        .diagnostics
        .iter()
        .find(|d| d.code == code)
        .unwrap_or_else(|| panic!("expected {code}; got {:?}", r.diagnostics));
    assert!(
        d.message.contains(needle),
        "message {:?} must contain {needle:?}",
        d.message
    );
}

fn reparse(r: &TxResult) -> Document {
    KdlAdapter
        .parse(r.source_after.as_bytes())
        .expect("source_after must parse")
}

#[test]
fn upsert_document_scope() {
    let r = run(DOC, vec![set(None, "text", "body", None)]);
    assert_accepted(&r);
    let doc = reparse(&r);
    let e = doc.defaults.get(DefaultsKind::Text).expect("entry");
    assert_eq!(e.style, "body");
    assert_eq!(e.text_style, None);
    assert!(r.source_after.contains("defaults {"));
}

#[test]
fn upsert_page_scope_leaves_document_block_untouched() {
    let r = run(DOC, vec![set(Some("pg2"), "rect", "box", None)]);
    assert_accepted(&r);
    assert_eq!(r.affected_node_ids, vec!["pg2".to_owned()]);
    let doc = reparse(&r);
    assert!(doc.defaults.entries.is_empty());
    let pg1 = doc.body.pages.iter().find(|p| p.id == "pg1").expect("pg1");
    assert!(pg1.defaults.entries.is_empty());
    let pg2 = doc.body.pages.iter().find(|p| p.id == "pg2").expect("pg2");
    assert_eq!(
        pg2.defaults
            .get(DefaultsKind::Rect)
            .map(|e| e.style.as_str()),
        Some("box")
    );
}

#[test]
fn overwrite_replaces_style_and_keeps_text_style_when_absent() {
    let r = run(
        DOC,
        vec![
            set(None, "shape", "box", Some(Some("box.label"))),
            set(None, "shape", "body", None),
        ],
    );
    assert_accepted(&r);
    let doc = reparse(&r);
    let e = doc.defaults.get(DefaultsKind::Shape).expect("entry");
    assert_eq!(e.style, "body");
    assert_eq!(e.text_style.as_deref(), Some("box.label"));
}

#[test]
fn null_text_style_clears_it() {
    let r = run(
        DOC,
        vec![
            set(None, "connector", "box", Some(Some("box.label"))),
            set(None, "connector", "box", Some(None)),
        ],
    );
    assert_accepted(&r);
    let doc = reparse(&r);
    let e = doc.defaults.get(DefaultsKind::Connector).expect("entry");
    assert_eq!(e.text_style, None);
}

#[test]
fn remove_entry() {
    let r = run(
        DOC,
        vec![set(None, "text", "body", None), remove(None, "text")],
    );
    assert_accepted(&r);
    assert!(reparse(&r).defaults.entries.is_empty());
}

#[test]
fn remove_from_page_scope() {
    let r = run(
        DOC,
        vec![
            set(Some("pg1"), "rect", "box", None),
            set(None, "rect", "body", None),
            remove(Some("pg1"), "rect"),
        ],
    );
    assert_accepted(&r);
    let doc = reparse(&r);
    assert!(doc.body.pages.iter().all(|p| p.defaults.entries.is_empty()));
    assert!(doc.defaults.get(DefaultsKind::Rect).is_some());
}

#[test]
fn remove_missing_entry_is_rejected() {
    let r = run(DOC, vec![remove(None, "text")]);
    assert_rejected(&r, "tx.invalid_value", "nothing to remove");
}

#[test]
fn unknown_kind_suggests_nearest() {
    let r = run(DOC, vec![set(None, "txet", "body", None)]);
    assert_rejected(&r, "tx.invalid_value", "did you mean \"text\"");
    let r = run(DOC, vec![remove(None, "txet")]);
    assert_rejected(&r, "tx.invalid_value", "did you mean \"text\"");
}

#[test]
fn unknown_kind_without_near_match_lists_kinds() {
    let r = run(DOC, vec![set(None, "zzzzzzzz", "body", None)]);
    assert_rejected(&r, "tx.invalid_value", "Kinds: chart, code, connector");
}

#[test]
fn unknown_style_suggests_nearest() {
    let r = run(DOC, vec![set(None, "text", "bdoy", None)]);
    assert_rejected(&r, "tx.unknown_style", "did you mean \"body\"");
}

#[test]
fn unknown_style_without_near_match_lists_styles() {
    let r = run(DOC, vec![set(None, "text", "zzzzzzzz", None)]);
    assert_rejected(
        &r,
        "tx.unknown_style",
        "declared styles: body, box, box.label",
    );
}

#[test]
fn unknown_text_style_is_rejected() {
    let r = run(
        DOC,
        vec![set(None, "shape", "box", Some(Some("box.lable")))],
    );
    assert_rejected(&r, "tx.unknown_style", "did you mean \"box.label\"");
}

#[test]
fn unsupported_kind_is_rejected() {
    for kind in ["instance", "light", "mesh"] {
        let r = run(DOC, vec![set(None, kind, "body", None)]);
        assert_rejected(&r, "tx.invalid_value", "takes no defaults entry");
    }
}

#[test]
fn text_style_on_wrong_kind_is_rejected() {
    let r = run(
        DOC,
        vec![set(None, "text", "body", Some(Some("box.label")))],
    );
    assert_rejected(&r, "tx.invalid_value", "only shape and connector");
}

#[test]
fn unknown_page_is_rejected() {
    let r = run(DOC, vec![set(Some("nope"), "text", "body", None)]);
    assert_rejected(&r, "tx.unknown_node", "page \"nope\" not found");
    let r = run(DOC, vec![remove(Some("nope"), "text")]);
    assert_rejected(&r, "tx.unknown_node", "page \"nope\" not found");
}

#[test]
fn entries_are_written_sorted_by_kind_and_fmt_is_idempotent() {
    let r = run(
        DOC,
        vec![
            set(None, "text", "body", None),
            set(None, "rect", "box", None),
            set(None, "shape", "box", Some(Some("box.label"))),
            set(Some("pg1"), "frame", "box", None),
        ],
    );
    assert_accepted(&r);
    let rect = r.source_after.find("rect style=").expect("rect row");
    let shape = r.source_after.find("shape style=").expect("shape row");
    let text = r.source_after.find("text style=").expect("text row");
    assert!(rect < shape && shape < text, "{}", r.source_after);

    let again = run(&r.source_after, Vec::new());
    assert_eq!(again.source_after, r.source_after);
}

#[test]
fn dry_run_input_document_is_not_mutated() {
    let doc = parse(DOC);
    let tx = Transaction {
        ops: vec![set(None, "text", "body", None)],
        permissions: Permissions::default(),
    };
    let r = run_transaction(&doc, &tx).expect("run");
    assert_accepted(&r);
    assert_ne!(r.source_before, r.source_after);
    assert!(doc.defaults.entries.is_empty());
}

#[test]
fn op_serde_names_and_tri_state() {
    let op: Op = serde_json::from_str(
        r#"{"op":"set_default","kind":"shape","style":"box","text_style":null}"#,
    )
    .expect("parse");
    assert_eq!(op, set(None, "shape", "box", Some(None)));

    let op: Op = serde_json::from_str(r#"{"op":"set_default","kind":"shape","style":"box"}"#)
        .expect("parse");
    assert_eq!(op, set(None, "shape", "box", None));

    let op: Op = serde_json::from_str(
        r#"{"op":"set_default","page":"pg1","kind":"shape","style":"box","text_style":"box.label"}"#,
    )
    .expect("parse");
    assert_eq!(
        op,
        set(Some("pg1"), "shape", "box", Some(Some("box.label")))
    );

    let op: Op = serde_json::from_str(r#"{"op":"remove_default","page":"pg1","kind":"text"}"#)
        .expect("parse");
    assert_eq!(op, remove(Some("pg1"), "text"));

    let json = serde_json::to_value(set(None, "shape", "box", Some(None))).expect("ser");
    assert_eq!(json["op"], "set_default");
    assert!(json["text_style"].is_null());
    assert!(json.as_object().expect("obj").contains_key("text_style"));
    let json = serde_json::to_value(remove(None, "text")).expect("ser");
    assert_eq!(json["op"], "remove_default");
}
