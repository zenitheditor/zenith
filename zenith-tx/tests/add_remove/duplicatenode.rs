use super::*;

/// Document with a single rect and a fill token (needed for post-validate).
const DUP_RECT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.a" type="color" value="#ff0000"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="orig" x=(px)10 y=(px)20 w=(px)80 h=(px)60 fill=(token)"color.a"
    }
  }
}"##;

/// Document with a group containing a rect (for container-rejection test).
const DUP_GROUP_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      group id="grp" {
        rect id="inner" x=(px)0 y=(px)0 w=(px)50 h=(px)50
      }
    }
  }
}"##;

// ── DuplicateNode tests ───────────────────────────────────────────────────

/// Duplicate a leaf rect: parent now has 2 rects, clone right after original,
/// clone has new_id and same geometry/fill.
#[test]
fn duplicate_node_leaf_rect_accepted() {
    let doc = parse(DUP_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::DuplicateNode {
            node: "orig".to_owned(),
            new_id: "orig-copy".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "expected Accepted; diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["orig-copy".to_owned()]);

    // Both ids must appear in source_after.
    assert!(
        result.source_after.contains("id=\"orig\""),
        "original must still be present; got:\n{}",
        result.source_after
    );
    assert!(
        result.source_after.contains("id=\"orig-copy\""),
        "clone must be present; got:\n{}",
        result.source_after
    );

    // Clone must appear AFTER the original in source text.
    let pos_orig = result
        .source_after
        .find("id=\"orig\"")
        .expect("orig in source_after");
    let pos_copy = result
        .source_after
        .find("id=\"orig-copy\"")
        .expect("orig-copy in source_after");
    assert!(
        pos_orig < pos_copy,
        "clone should appear after original in source_after"
    );

    // Clone must carry the same geometry and fill as the original.
    // Count occurrences: both nodes should have x=(px)10, y=(px)20, etc.
    assert_eq!(
        result.source_after.matches("x=(px)10").count(),
        2,
        "both orig and clone should have x=(px)10; got:\n{}",
        result.source_after
    );
    assert_eq!(
        result.source_after.matches("w=(px)80").count(),
        2,
        "both orig and clone should have w=(px)80; got:\n{}",
        result.source_after
    );
    assert_eq!(
        result.source_after.matches("(token)\"color.a\"").count(),
        2,
        "both orig and clone should reference color.a; got:\n{}",
        result.source_after
    );

    // source_before has only one rect.
    assert_eq!(
        result.source_before.matches("id=\"orig").count(),
        1,
        "source_before should have only one orig* node"
    );
}

/// Duplicate with a new_id that already exists → post-validate rejects (id.duplicate).
#[test]
fn duplicate_node_colliding_new_id_rejected() {
    // TWO_RECT_DOC has rect "a" and rect "b"; duplicating "a" with new_id="b"
    // creates a second node with id "b" → id.duplicate from post-validate.
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::DuplicateNode {
            node: "a".to_owned(),
            new_id: "b".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Rejected,
        "colliding new_id must be rejected; diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result.diagnostics.iter().any(|d| d.code == "id.duplicate"),
        "expected id.duplicate diagnostic; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

/// Duplicating a group copies the subtree: the descendant gets a suffixed id.
#[test]
fn duplicate_node_container_group_accepted() {
    let doc = parse(DUP_GROUP_DOC);
    let tx = Transaction {
        ops: vec![Op::DuplicateNode {
            node: "grp".to_owned(),
            new_id: "grp-copy".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert!(result.source_after.contains("id=\"grp-copy\""));
    assert!(result.source_after.contains("id=\"inner-copy\""));
}

/// Attempting to duplicate an unknown node id → tx.unknown_node.
#[test]
fn duplicate_node_unknown_id_rejected() {
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::DuplicateNode {
            node: "does_not_exist".to_owned(),
            new_id: "copy".to_owned(),
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
    assert_eq!(result.source_after, result.source_before);
}
