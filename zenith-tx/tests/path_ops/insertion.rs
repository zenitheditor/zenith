use super::*;

#[test]
fn insert_path_anchor_open_line_split_inserts_midpoint() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::InsertPathAnchor {
            node: "path1".to_owned(),
            subpath_index: None,
            segment_index: 0,
            t: 0.5,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["path1".to_owned()]);
    assert_eq!(formatted_anchor_count(&result.source_after), 3);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "x"), 0.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "x"), 50.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "y"), 0.0);
    assert_px_close(anchor_px_attr(&result.source_after, 2, "x"), 100.0);
    assert!(
        !anchor_line(&result.source_after, 1).contains("kind="),
        "line split anchor should not claim smooth/symmetric intent; got:\n{}",
        result.source_after
    );
}

#[test]
fn anchor_indexed_path_ops_require_subpath_index_for_compound_paths() {
    let doc = parse(COMPOUND_PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::SetPathAnchorKind {
            node: "compound".to_owned(),
            subpath_index: None,
            anchor_index: 0,
            kind: Some("smooth".to_owned()),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "tx.unsupported_property"
                && diagnostic.message.contains("subpath_index")
        }),
        "expected compound path rejection diagnostic; got {:?}",
        result.diagnostics
    );
}

#[test]
fn remove_path_anchor_removes_direct_anchor() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="path1" {
        anchor x=(px)0 y=(px)0
        anchor x=(px)50 y=(px)0
        anchor x=(px)100 y=(px)0
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::RemovePathAnchor {
            node: "path1".to_owned(),
            subpath_index: None,
            anchor_index: 1,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["path1".to_owned()]);
    assert_eq!(formatted_anchor_count(&result.source_after), 2);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "x"), 0.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "x"), 100.0);
}

#[test]
fn remove_path_anchor_targets_compound_subpath() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="compound" fill-rule="evenodd" {
        subpath closed=#true {
          anchor x=(px)0 y=(px)0
          anchor x=(px)100 y=(px)0
          anchor x=(px)100 y=(px)100
        }
        subpath closed=#true {
          anchor x=(px)25 y=(px)25
          anchor x=(px)75 y=(px)25
          anchor x=(px)75 y=(px)75
          anchor x=(px)25 y=(px)75
        }
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::RemovePathAnchor {
            node: "compound".to_owned(),
            subpath_index: Some(1),
            anchor_index: 1,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["compound".to_owned()]);
    assert_eq!(formatted_anchor_count(&result.source_after), 6);
    assert!(!result.source_after.contains("x=(px)75 y=(px)25"));
    assert!(result.source_after.contains("x=(px)75 y=(px)75"));
}

#[test]
fn remove_path_anchor_requires_subpath_index_for_compound_paths() {
    let doc = parse(COMPOUND_PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::RemovePathAnchor {
            node: "compound".to_owned(),
            subpath_index: None,
            anchor_index: 0,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "tx.unsupported_property"
                && diagnostic.message.contains("subpath_index")
        }),
        "expected compound path rejection diagnostic; got {:?}",
        result.diagnostics
    );
}

#[test]
fn remove_path_anchor_rejects_out_of_range_index() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::RemovePathAnchor {
            node: "path1".to_owned(),
            subpath_index: None,
            anchor_index: 2,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.out_of_range" && d.message.contains("anchor_index")),
        "expected tx.out_of_range mentioning anchor_index; got: {:?}",
        result.diagnostics
    );
}

#[test]
fn insert_path_anchor_targets_compound_subpath() {
    let doc = parse(COMPOUND_PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::InsertPathAnchor {
            node: "compound".to_owned(),
            subpath_index: Some(1),
            segment_index: 0,
            t: 0.5,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["compound".to_owned()]);
    assert_eq!(formatted_anchor_count(&result.source_after), 7);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "x"), 0.0);
    assert_px_close(anchor_px_attr(&result.source_after, 4, "x"), 50.0);
    assert_px_close(anchor_px_attr(&result.source_after, 4, "y"), 25.0);
}

#[test]
fn insert_path_anchor_cubic_split_preserves_generated_handles() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="path1" {
        anchor x=(px)0 y=(px)0 kind="corner" out-x=(px)0 out-y=(px)100
        anchor x=(px)100 y=(px)0 kind="symmetric" in-x=(px)100 in-y=(px)100
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::InsertPathAnchor {
            node: "path1".to_owned(),
            subpath_index: None,
            segment_index: 0,
            t: 0.5,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["path1".to_owned()]);
    assert_eq!(formatted_anchor_count(&result.source_after), 3);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "out-x"), 0.0);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "out-y"), 50.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "x"), 50.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "y"), 75.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "in-x"), 25.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "in-y"), 75.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "out-x"), 75.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "out-y"), 75.0);
    assert_px_close(anchor_px_attr(&result.source_after, 2, "in-x"), 100.0);
    assert_px_close(anchor_px_attr(&result.source_after, 2, "in-y"), 50.0);
    assert!(
        anchor_line(&result.source_after, 0).contains("kind=\"corner\"")
            && anchor_line(&result.source_after, 1).contains("kind=\"smooth\"")
            && anchor_line(&result.source_after, 2).contains("kind=\"symmetric\""),
        "cubic split should preserve existing kinds and mark inserted anchor smooth; got:\n{}",
        result.source_after
    );
}

#[test]
fn insert_path_anchor_closed_closing_edge_appends_midpoint() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="path1" closed=#true {
        anchor x=(px)0 y=(px)0
        anchor x=(px)10 y=(px)0
        anchor x=(px)10 y=(px)10
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::InsertPathAnchor {
            node: "path1".to_owned(),
            subpath_index: None,
            segment_index: 2,
            t: 0.5,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert!(
        result
            .source_after
            .contains("path id=\"path1\" closed=#true")
    );
    assert_eq!(formatted_anchor_count(&result.source_after), 4);
    assert_px_close(anchor_px_attr(&result.source_after, 3, "x"), 5.0);
    assert_px_close(anchor_px_attr(&result.source_after, 3, "y"), 5.0);
}

#[test]
fn insert_path_anchor_invalid_segment_index_rejected_without_source_change() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::InsertPathAnchor {
            node: "path1".to_owned(),
            subpath_index: None,
            segment_index: 1,
            t: 0.5,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| { d.code == "tx.invalid_geometry" && d.message.contains("segment_index") }),
        "expected tx.invalid_geometry for invalid segment index; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn insert_path_anchor_invalid_t_rejected_without_source_change() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::InsertPathAnchor {
            node: "path1".to_owned(),
            subpath_index: None,
            segment_index: 0,
            t: 1.1,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_geometry" && d.message.contains("t")),
        "expected tx.invalid_geometry for invalid t; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn insert_path_anchor_locked_path_rejected() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="path1" locked=#true {
        anchor x=(px)0 y=(px)0
        anchor x=(px)20 y=(px)0
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::InsertPathAnchor {
            node: "path1".to_owned(),
            subpath_index: None,
            segment_index: 0,
            t: 0.5,
        }],
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
fn insert_path_anchor_unsupported_on_rect() {
    let doc = parse(RECT_GEOM_DOC);
    let tx = Transaction {
        ops: vec![Op::InsertPathAnchor {
            node: "rect".to_owned(),
            subpath_index: None,
            segment_index: 0,
            t: 0.5,
        }],
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
fn insert_path_anchor_non_px_anchor_rejected_by_shared_resolver() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="path1" {
        anchor x=(pt)0 y=(px)0
        anchor x=(px)20 y=(px)0
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::InsertPathAnchor {
            node: "path1".to_owned(),
            subpath_index: None,
            segment_index: 0,
            t: 0.5,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result.diagnostics.iter().any(|d| {
            d.code == "tx.invalid_path_anchor" && d.message.contains("must be a px value")
        }),
        "expected tx.invalid_path_anchor for non-px anchor; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}
