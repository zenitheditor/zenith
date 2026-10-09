use super::*;

/// Rect inside a group.
const NESTED_RECT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="grp1" {
        rect id="inner" x=(px)0 y=(px)0 w=(px)50 h=(px)50
      }
    }
  }
}"##;

// ── SetVisible tests ──────────────────────────────────────────────────────

#[test]
fn set_visible_hides_node() {
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetVisible {
            node: "a".to_owned(),
            visible: false,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["a".to_owned()]);
    assert!(
        result.source_after.contains("visible=#false"),
        "source_after must contain visible=#false; got:\n{}",
        result.source_after
    );
    assert_ne!(result.source_before, result.source_after);
}

#[test]
fn set_visible_on_nested_node() {
    let doc = parse(NESTED_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetVisible {
            node: "inner".to_owned(),
            visible: false,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["inner".to_owned()]);
    assert!(
        result.source_after.contains("visible=#false"),
        "source_after must contain visible=#false for nested node; got:\n{}",
        result.source_after
    );
}

// ── SetLocked tests ───────────────────────────────────────────────────────

#[test]
fn set_locked_sets_lock() {
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetLocked {
            node: "b".to_owned(),
            locked: true,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["b".to_owned()]);
    assert!(
        result.source_after.contains("locked=#true"),
        "source_after must contain locked=#true; got:\n{}",
        result.source_after
    );
    assert_ne!(result.source_before, result.source_after);
}

// ── Unknown node targeting ────────────────────────────────────────────────

// Unknown nodes without an authored `id` are not addressable via `Node::id()`
// (`None`). Targeting a non-existent id therefore yields `tx.unknown_node`.
#[test]
fn set_visible_on_nonexistent_id_is_unknown_node() {
    // Using TEXT_DOC — there is no node with id "does_not_exist".
    // The important thing: we get tx.unknown_node, not a panic or
    // tx.unsupported_property.
    let doc = parse(TEXT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetVisible {
            node: "does_not_exist".to_owned(),
            visible: false,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unknown_node"),
        "expected tx.unknown_node; got: {:?}",
        result.diagnostics
    );
}
