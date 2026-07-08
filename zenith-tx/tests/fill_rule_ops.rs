mod common;
use common::*;
use zenith_tx::{Op, Permissions, Transaction, TxStatus, run_transaction};

fn set_fill_rule_op(node: &str, fill_rule: &str) -> Op {
    Op::SetFillRule {
        node: node.to_owned(),
        fill_rule: fill_rule.to_owned(),
    }
}

#[test]
fn set_fill_rule_updates_path() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![set_fill_rule_op("path1", "evenodd")],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["path1".to_owned()]);
    assert!(
        result.source_after.contains("fill-rule=\"evenodd\""),
        "source_after must contain fill-rule=\"evenodd\"; got:\n{}",
        result.source_after
    );
    assert_ne!(result.source_before, result.source_after);
}

#[test]
fn set_fill_rule_updates_polygon_and_polyline() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      polygon id="poly" {
        point x=(px)0 y=(px)0
        point x=(px)100 y=(px)0
        point x=(px)50 y=(px)80
      }
      polyline id="line.poly" {
        point x=(px)0 y=(px)100
        point x=(px)100 y=(px)100
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![
            set_fill_rule_op("poly", "evenodd"),
            set_fill_rule_op("line.poly", "nonzero"),
        ],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(
        result.affected_node_ids,
        vec!["poly".to_owned(), "line.poly".to_owned()]
    );
    assert!(
        result
            .source_after
            .contains("polygon id=\"poly\" fill-rule=\"evenodd\""),
        "source_after must update polygon fill-rule; got:\n{}",
        result.source_after
    );
    assert!(
        result
            .source_after
            .contains("polyline id=\"line.poly\" fill-rule=\"nonzero\""),
        "source_after must update polyline fill-rule; got:\n{}",
        result.source_after
    );
}

#[test]
fn set_fill_rule_invalid_value_rejected_at_tx_boundary() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![set_fill_rule_op("path1", "oddeven")],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result.diagnostics.iter().any(|d| {
            d.code == "tx.invalid_value"
                && d.message.contains("fill-rule")
                && d.message.contains("nonzero|evenodd")
                && d.message.contains("oddeven")
        }),
        "expected tx.invalid_value naming bad fill-rule; got: {:?}",
        result.diagnostics
    );
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|d| d.code == "node.unknown_property"),
        "invalid fill-rule should be rejected by tx before core warning; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_fill_rule_unsupported_on_rect() {
    let doc = parse(RECT_GEOM_DOC);
    let tx = Transaction {
        ops: vec![set_fill_rule_op("rect", "evenodd")],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property" && d.message.contains("rect")),
        "expected tx.unsupported_property mentioning rect; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_fill_rule_unknown_node_rejected() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![set_fill_rule_op("missing.path", "evenodd")],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unknown_node" && d.message.contains("missing.path")),
        "expected tx.unknown_node naming target; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_fill_rule_rejects_locked_path() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="path1" locked=#true {
        anchor x=(px)0 y=(px)0
        anchor x=(px)100 y=(px)0
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![set_fill_rule_op("path1", "evenodd")],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result.diagnostics.iter().any(|d| d.code == "node.locked"),
        "expected node.locked; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_fill_rule_allows_locked_path_with_permission() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="path1" locked=#true {
        anchor x=(px)0 y=(px)0
        anchor x=(px)100 y=(px)0
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![set_fill_rule_op("path1", "evenodd")],
        permissions: Permissions {
            allow_locked: true,
            ..Permissions::default()
        },
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["path1".to_owned()]);
    assert!(
        result.source_after.contains("fill-rule=\"evenodd\""),
        "source_after must contain fill-rule=\"evenodd\"; got:\n{}",
        result.source_after
    );
}
