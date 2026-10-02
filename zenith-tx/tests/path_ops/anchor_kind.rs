use super::*;

#[test]
fn set_path_anchor_kind_sets_metadata_without_moving_anchor() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::SetPathAnchorKind {
            node: "path1".to_owned(),
            subpath_index: None,
            anchor_index: 1,
            kind: Some("smooth".to_owned()),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["path1".to_owned()]);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "x"), 0.0);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "y"), 0.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "x"), 100.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "y"), 0.0);
    assert!(
        anchor_line(&result.source_after, 1).contains("kind=\"smooth\""),
        "anchor kind should be formatted on anchor 1; got:\n{}",
        result.source_after
    );
}

#[test]
fn set_path_anchor_kind_clears_existing_metadata_without_moving_anchor() {
    let doc = parse(TRANSFORM_PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::SetPathAnchorKind {
            node: "path1".to_owned(),
            subpath_index: None,
            anchor_index: 1,
            kind: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "x"), 20.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "y"), 0.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "in-x"), 10.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "in-y"), 0.0);
    assert!(
        !anchor_line(&result.source_after, 1).contains("kind="),
        "anchor kind should be removed from anchor 1; got:\n{}",
        result.source_after
    );
}

#[test]
fn set_path_anchor_kind_targets_compound_subpath() {
    let doc = parse(COMPOUND_PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::SetPathAnchorKind {
            node: "compound".to_owned(),
            subpath_index: Some(1),
            anchor_index: 1,
            kind: Some("smooth".to_owned()),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["compound".to_owned()]);
    assert!(
        anchor_line(&result.source_after, 4).contains("kind=\"smooth\""),
        "subpath anchor kind should be updated; got:\n{}",
        result.source_after
    );
    assert!(
        !anchor_line(&result.source_after, 1).contains("kind="),
        "first contour should remain unchanged; got:\n{}",
        result.source_after
    );
}

#[test]
fn set_path_anchor_kind_rejects_unrecognized_kind_without_source_change() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::SetPathAnchorKind {
            node: "path1".to_owned(),
            subpath_index: None,
            anchor_index: 0,
            kind: Some("future".to_owned()),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result.diagnostics.iter().any(|d| {
            d.code == "node.invalid_value"
                && d.severity == zenith_core::Severity::Error
                && d.message.contains("'future'")
                && d.message.contains("corner, smooth, symmetric")
        }),
        "expected node.invalid_value error naming allowed kinds; got: {:?}",
        result.diagnostics
    );
    assert_eq!(
        result.source_after, result.source_before,
        "a rejected transaction must leave the source unchanged"
    );
}

#[test]
fn set_path_anchor_kind_out_of_range_rejected_without_source_change() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::SetPathAnchorKind {
            node: "path1".to_owned(),
            subpath_index: None,
            anchor_index: 2,
            kind: Some("smooth".to_owned()),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| { d.code == "tx.out_of_range" && d.message.contains("anchor_index") }),
        "expected tx.out_of_range mentioning anchor_index; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_path_anchor_kind_locked_path_rejected() {
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
        ops: vec![Op::SetPathAnchorKind {
            node: "path1".to_owned(),
            subpath_index: None,
            anchor_index: 1,
            kind: Some("smooth".to_owned()),
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
fn set_path_anchor_kind_unsupported_on_rect() {
    let doc = parse(RECT_GEOM_DOC);
    let tx = Transaction {
        ops: vec![Op::SetPathAnchorKind {
            node: "rect".to_owned(),
            subpath_index: None,
            anchor_index: 0,
            kind: Some("smooth".to_owned()),
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
