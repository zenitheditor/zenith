use super::*;

fn insert_at_point(node: &str, x: f64, y: f64, tolerance: f64) -> Op {
    Op::InsertPathAnchorAtPoint {
        node: node.to_owned(),
        x,
        y,
        tolerance,
    }
}

#[test]
fn insert_path_anchor_at_point_open_line_inserts_exact_midpoint() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![insert_at_point("path1", 50.0, 2.0, 3.0)],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["path1".to_owned()]);
    assert_eq!(formatted_anchor_count(&result.source_after), 3);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "x"), 50.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "y"), 0.0);
    assert!(
        !anchor_line(&result.source_after, 1).contains("kind="),
        "line split anchor should not claim smooth/symmetric intent; got:\n{}",
        result.source_after
    );
}

#[test]
fn insert_path_anchor_at_point_cubic_marks_inserted_anchor_smooth_and_keeps_handles() {
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
        ops: vec![insert_at_point("path1", 50.0, 75.0, 1.0)],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(formatted_anchor_count(&result.source_after), 3);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "x"), 50.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "y"), 75.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "in-x"), 25.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "in-y"), 75.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "out-x"), 75.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "out-y"), 75.0);
    assert!(
        anchor_line(&result.source_after, 0).contains("kind=\"corner\"")
            && anchor_line(&result.source_after, 1).contains("kind=\"smooth\"")
            && anchor_line(&result.source_after, 2).contains("kind=\"symmetric\""),
        "cubic split should preserve existing kinds and mark inserted anchor smooth; got:\n{}",
        result.source_after
    );
}

#[test]
fn insert_path_anchor_at_point_closed_path_closing_edge_appends_anchor() {
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
        ops: vec![insert_at_point("path1", 4.5, 4.5, 0.1)],
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
    assert_px_close(anchor_px_attr(&result.source_after, 3, "x"), 4.5);
    assert_px_close(anchor_px_attr(&result.source_after, 3, "y"), 4.5);
}

#[test]
fn insert_path_anchor_at_point_targets_nearest_compound_subpath() {
    let doc = parse(COMPOUND_PATH_DOC);
    let tx = Transaction {
        ops: vec![insert_at_point("compound", 50.0, 27.0, 3.0)],
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
fn insert_path_anchor_at_point_outside_tolerance_rejects_without_source_change() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![insert_at_point("path1", 50.0, 5.0, 4.0)],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| { d.code == "tx.invalid_geometry" && d.message.contains("within tolerance") }),
        "expected tx.invalid_geometry for projection outside tolerance; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn insert_path_anchor_at_point_invalid_tolerance_rejects_without_source_change() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![insert_at_point("path1", 50.0, 0.0, 0.0)],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_geometry_tolerance"),
        "expected tx.invalid_geometry_tolerance; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn insert_path_anchor_at_point_non_finite_point_rejects_without_source_change() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![insert_at_point("path1", f64::NAN, 0.0, 1.0)],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result.diagnostics.iter().any(|d| {
            d.code == "tx.invalid_geometry" && d.message.contains("point coordinates")
        }),
        "expected tx.invalid_geometry for non-finite point; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn insert_path_anchor_at_point_locked_path_rejects_without_source_change() {
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
        ops: vec![insert_at_point("path1", 10.0, 0.0, 1.0)],
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
fn insert_path_anchor_at_point_unsupported_rect_rejects_without_source_change() {
    let doc = parse(RECT_GEOM_DOC);
    let tx = Transaction {
        ops: vec![insert_at_point("rect", 50.0, 0.0, 1.0)],
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
fn insert_path_anchor_at_point_unknown_node_rejects_without_source_change() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![insert_at_point("missing", 50.0, 0.0, 1.0)],
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

#[test]
fn insert_path_anchor_at_point_non_px_anchor_rejects_via_shared_resolver() {
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
        ops: vec![insert_at_point("path1", 10.0, 0.0, 1.0)],
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

#[test]
fn insert_path_anchor_at_point_malformed_handle_rejects_via_shared_resolver() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="path1" {
        anchor x=(px)0 y=(px)0 out-x=(px)10
        anchor x=(px)20 y=(px)0
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![insert_at_point("path1", 10.0, 0.0, 1.0)],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| { d.code == "tx.invalid_path_anchor" && d.message.contains("requires both") }),
        "expected tx.invalid_path_anchor for malformed handle; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}
