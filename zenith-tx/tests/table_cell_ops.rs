//! Integration tests verifying that tx ops can find, mutate, remove, and
//! duplicate nodes that live inside table cell children — the same operations
//! that already work for nodes inside a `frame` or `group`.

mod common;
use common::*;
use zenith_tx::{Op, Permissions, Transaction, TxStatus, run_transaction};

// ── Shared test document ──────────────────────────────────────────────────────

/// A table whose first cell contains a `rect` and second cell contains a
/// `text` node. All node ids are distinct and stable.
const TABLE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.a" type="color" value="#ff0000"
    token id="color.b" type="color" value="#0000ff"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      table id="tbl1" x=(px)0 y=(px)0 w=(px)300 h=(px)200 {
        column width=(px)150
        column width=(px)150
        row {
          cell {
            rect id="cell.rect" x=(px)0 y=(px)0 w=(px)50 h=(px)50
          }
          cell {
            text id="cell.text" x=(px)0 y=(px)0 w=(px)100 h=(px)30 {
              span "hello"
            }
          }
        }
      }
    }
  }
}"##;

// ── set_fill: mutate a node inside a table cell ───────────────────────────────

#[test]
fn set_fill_on_cell_child_succeeds() {
    let doc = parse(TABLE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetFill {
            node: "cell.rect".to_owned(),
            fill: "color.a".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "set_fill on cell child must be accepted; diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["cell.rect".to_owned()]);
    assert!(
        result.source_after.contains("id=\"cell.rect\""),
        "cell.rect must remain in source_after"
    );
}

// ── set_visible: mutate a node inside a table cell ───────────────────────────

#[test]
fn set_visible_on_cell_child_succeeds() {
    let doc = parse(TABLE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetVisible {
            node: "cell.text".to_owned(),
            visible: false,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "set_visible on cell child must be accepted; diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["cell.text".to_owned()]);
    assert!(
        result.source_after.contains("visible=#false"),
        "source_after must carry the updated visible flag"
    );
}

// ── set_geometry: mutate a node inside a table cell ──────────────────────────

#[test]
fn set_geometry_on_cell_child_succeeds() {
    let doc = parse(TABLE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetGeometry {
            node: "cell.rect".to_owned(),
            x: None,
            y: None,
            w: Some(80.0.into()),
            h: None,
            rotate: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "set_geometry on cell child must be accepted; diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["cell.rect".to_owned()]);
    assert!(
        result.source_after.contains("w=(px)80"),
        "source_after must reflect the new width; got:\n{}",
        result.source_after
    );
}

// ── remove_node: delete a node inside a table cell ───────────────────────────

#[test]
fn remove_node_inside_table_cell_succeeds() {
    let doc = parse(TABLE_DOC);
    let tx = Transaction {
        ops: vec![Op::RemoveNode {
            node: "cell.rect".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "remove_node on cell child must be accepted; diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["cell.rect".to_owned()]);
    assert!(
        !result.source_after.contains("id=\"cell.rect\""),
        "cell.rect must be absent from source_after"
    );
    // The other cell child must survive.
    assert!(
        result.source_after.contains("id=\"cell.text\""),
        "cell.text must remain in source_after"
    );
}

// ── duplicate_node: clone a node inside a table cell ─────────────────────────

#[test]
fn duplicate_node_inside_table_cell_succeeds() {
    let doc = parse(TABLE_DOC);
    let tx = Transaction {
        ops: vec![Op::DuplicateNode {
            node: "cell.rect".to_owned(),
            new_id: "cell.rect.copy".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "duplicate_node on cell child must be accepted; diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["cell.rect.copy".to_owned()]);
    assert!(
        result.source_after.contains("id=\"cell.rect\""),
        "original cell.rect must remain"
    );
    assert!(
        result.source_after.contains("id=\"cell.rect.copy\""),
        "cloned cell.rect.copy must appear in source_after"
    );
    // Clone should follow original in document order.
    let pos_orig = result
        .source_after
        .find("id=\"cell.rect\"")
        .expect("original present");
    let pos_copy = result
        .source_after
        .find("id=\"cell.rect.copy\"")
        .expect("copy present");
    assert!(
        pos_orig < pos_copy,
        "clone must come after the original in source order"
    );
}

// ── non-existent id still errors ─────────────────────────────────────────────

#[test]
fn op_on_nonexistent_id_errors() {
    let doc = parse(TABLE_DOC);
    let tx = Transaction {
        ops: vec![Op::RemoveNode {
            node: "does.not.exist".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Rejected,
        "unknown node id must reject the transaction"
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unknown_node"),
        "must emit tx.unknown_node diagnostic; got: {:?}",
        result.diagnostics
    );
}

// ── reorder: move a node inside a table cell ──────────────────────────────────

#[test]
fn reorder_inside_table_cell_succeeds() {
    // Build a doc with two rects inside a single table cell so reorder is
    // meaningful.
    const TWO_CELL_RECTS: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      table id="tbl1" x=(px)0 y=(px)0 w=(px)200 h=(px)100 {
        column width=(px)200
        row {
          cell {
            rect id="cr1" x=(px)0 y=(px)0 w=(px)50 h=(px)50
            rect id="cr2" x=(px)0 y=(px)0 w=(px)50 h=(px)50
          }
        }
      }
    }
  }
}"##;

    let doc = parse(TWO_CELL_RECTS);
    let tx = Transaction {
        ops: vec![Op::MoveForward {
            node: "cr1".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "MoveForward on cell child must be accepted; diagnostics: {:?}",
        result.diagnostics
    );
    // After MoveForward, cr1 should appear after cr2.
    let pos_cr1 = result
        .source_after
        .find("id=\"cr1\"")
        .expect("cr1 in source_after");
    let pos_cr2 = result
        .source_after
        .find("id=\"cr2\"")
        .expect("cr2 in source_after");
    assert!(
        pos_cr2 < pos_cr1,
        "cr2 must appear before cr1 after MoveForward"
    );
}
