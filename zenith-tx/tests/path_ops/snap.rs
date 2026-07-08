use super::*;

#[test]
fn snap_path_anchors_translates_source_to_nearest_target_boundary() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="source" {
        anchor x=(px)0 y=(px)0 kind="corner" out-x=(px)5 out-y=(px)0
        anchor x=(px)10 y=(px)0 kind="smooth" in-x=(px)5 in-y=(px)0
      }
      path id="target" locked=#true {
        anchor x=(px)13 y=(px)2
        anchor x=(px)13 y=(px)12
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::SnapPathAnchors {
            node: "source".to_owned(),
            target: "target".to_owned(),
            tolerance: 4.0,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["source".to_owned()]);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "x"), 3.0);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "y"), 2.0);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "out-x"), 8.0);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "out-y"), 2.0);
    assert!(anchor_line(&result.source_after, 0).contains("kind=\"corner\""));
    assert_px_close(anchor_px_attr(&result.source_after, 1, "x"), 13.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "y"), 2.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "in-x"), 8.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "in-y"), 2.0);
    assert!(anchor_line(&result.source_after, 1).contains("kind=\"smooth\""));
    assert_px_close(anchor_px_attr(&result.source_after, 2, "x"), 13.0);
    assert_px_close(anchor_px_attr(&result.source_after, 2, "y"), 2.0);
}

#[test]
fn snap_path_anchors_translates_compound_source() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="source" fill-rule="evenodd" {
        subpath closed=#true {
          anchor x=(px)0 y=(px)0
          anchor x=(px)100 y=(px)0
          anchor x=(px)100 y=(px)100
        }
        subpath closed=#true {
          anchor x=(px)25 y=(px)25
          anchor x=(px)75 y=(px)25
          anchor x=(px)75 y=(px)75
        }
      }
      path id="target" locked=#true {
        anchor x=(px)103 y=(px)2
        anchor x=(px)103 y=(px)12
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::SnapPathAnchors {
            node: "source".to_owned(),
            target: "target".to_owned(),
            tolerance: 4.0,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["source".to_owned()]);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "x"), 3.0);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "y"), 0.0);
    assert_px_close(anchor_px_attr(&result.source_after, 3, "x"), 28.0);
    assert_px_close(anchor_px_attr(&result.source_after, 3, "y"), 25.0);
    assert_px_close(anchor_px_attr(&result.source_after, 6, "x"), 103.0);
    assert_px_close(anchor_px_attr(&result.source_after, 6, "y"), 2.0);
}

#[test]
fn snap_path_anchors_rejects_when_gap_exceeds_tolerance() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="source" {
        anchor x=(px)0 y=(px)0
        anchor x=(px)10 y=(px)0
      }
      path id="target" {
        anchor x=(px)13 y=(px)2
        anchor x=(px)13 y=(px)12
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::SnapPathAnchors {
            node: "source".to_owned(),
            target: "target".to_owned(),
            tolerance: 2.0,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| { d.code == "tx.invalid_geometry" && d.message.contains("within tolerance") }),
        "expected tx.invalid_geometry for missing snap target; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn snap_path_anchors_rejects_invalid_tolerance_and_non_path_target() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="source" {
        anchor x=(px)0 y=(px)0
        anchor x=(px)10 y=(px)0
      }
      rect id="target" x=(px)0 y=(px)0 w=(px)10 h=(px)10
    }
  }
}"##,
    );
    let invalid_tolerance = Transaction {
        ops: vec![Op::SnapPathAnchors {
            node: "source".to_owned(),
            target: "source".to_owned(),
            tolerance: 0.0,
        }],
        permissions: Permissions::default(),
    };
    let result =
        run_transaction(&doc, &invalid_tolerance).expect("run_transaction should not error");

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

    let non_path_target = Transaction {
        ops: vec![Op::SnapPathAnchors {
            node: "source".to_owned(),
            target: "target".to_owned(),
            tolerance: 4.0,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &non_path_target).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property" && d.message.contains("rect")),
        "expected tx.unsupported_property for non-path target; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn snap_path_anchors_locked_source_rejected() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="source" locked=#true {
        anchor x=(px)0 y=(px)0
        anchor x=(px)10 y=(px)0
      }
      path id="target" {
        anchor x=(px)13 y=(px)2
        anchor x=(px)13 y=(px)12
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::SnapPathAnchors {
            node: "source".to_owned(),
            target: "target".to_owned(),
            tolerance: 4.0,
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
