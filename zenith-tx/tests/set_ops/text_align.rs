use super::*;

const ELLIPSE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      ellipse id="dot" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      text id="lbl" x=(px)10 y=(px)10 w=(px)200 h=(px)40 {
        span "Hi"
      }
    }
  }
}"##;

const GROUP_TEXT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Nest"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="grp1" {
        text id="nested.label" x=(px)10 y=(px)10 w=(px)200 h=(px)40 align="start" {
          span "Hello"
        }
      }
    }
  }
}"##;

// ── 1. SetTextAlign: accepted, affected ids, source diff ──────────────────

#[test]
fn set_text_align_accepted() {
    let doc = parse(TEXT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "label".to_owned(),
            align: "center".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["label".to_owned()]);
    assert!(
        result.source_after.contains("center"),
        "source_after should contain align=\"center\""
    );
    assert!(
        !result.source_before.contains("center"),
        "source_before should not contain center"
    );
    assert_ne!(result.source_before, result.source_after);
}

// ── 5. SetTextAlign on a rect → wrong_node_type, Rejected ────────────────

#[test]
fn set_text_align_wrong_node_type() {
    let doc = parse(MIXED_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "box1".to_owned(),
            align: "center".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.wrong_node_type"),
        "expected tx.wrong_node_type diagnostic"
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── 5b. SetTextAlign on an ellipse → wrong_node_type, Rejected ───────────

#[test]
fn set_text_align_on_ellipse_wrong_node_type() {
    let doc = parse(ELLIPSE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "dot".to_owned(),
            align: "center".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.wrong_node_type" && d.message.contains("ellipse")),
        "expected tx.wrong_node_type diagnostic naming the ellipse kind"
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── 5c. SetTextAlign on an image → wrong_node_type, Rejected ─────────────

#[test]
fn set_text_align_on_image_wrong_node_type() {
    let doc = parse(IMAGE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "pic".to_owned(),
            align: "center".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.wrong_node_type" && d.message.contains("image")),
        "expected tx.wrong_node_type diagnostic naming the image kind"
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── SetTextAlign: recursion into group children ───────────────────────────

#[test]
fn tx_set_text_align_targets_nested_text() {
    // A text node nested inside a group should now be reachable via
    // recursive descent; the tx engine is no longer limited to top-level
    // page children.
    let doc = parse(GROUP_TEXT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "nested.label".to_owned(),
            align: "center".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["nested.label".to_owned()]);
    assert!(
        result.source_after.contains("center"),
        "source_after should contain align=\"center\""
    );
    assert!(!result.source_before.contains("center"));
    assert_ne!(result.source_before, result.source_after);
}

#[test]
fn tx_set_text_align_on_group_itself_wrong_type() {
    // Targeting the group's own id with SetTextAlign must yield
    // tx.wrong_node_type mentioning "group".
    let doc = parse(GROUP_TEXT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "grp1".to_owned(),
            align: "center".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.wrong_node_type" && d.message.contains("group")),
        "expected tx.wrong_node_type diagnostic naming \"group\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}
