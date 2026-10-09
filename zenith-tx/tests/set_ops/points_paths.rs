use super::*;

/// Polygon with exactly 3 points and a fill token (to keep post-validate happy).
const POLY_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#ff0000"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      polygon id="poly" fill=(token)"color.fill" {
        point x=(px)0 y=(px)0
        point x=(px)100 y=(px)0
        point x=(px)50 y=(px)80
      }
    }
  }
}"##;

// ── SetPoints tests ───────────────────────────────────────────────────────

#[test]
fn set_points_replaces_polygon() {
    let doc = parse(POLY_DOC);
    // Replace the 3 original points with 3 different ones.
    let tx = Transaction {
        ops: vec![Op::SetPoints {
            node: "poly".to_owned(),
            points: vec![
                OpPoint { x: 10.0, y: 20.0 },
                OpPoint { x: 90.0, y: 20.0 },
                OpPoint { x: 50.0, y: 70.0 },
            ],
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["poly".to_owned()]);

    // New coordinates appear in source_after.
    assert!(
        result.source_after.contains("x=(px)10"),
        "source_after must contain x=(px)10; got:\n{}",
        result.source_after
    );
    assert!(
        result.source_after.contains("y=(px)20"),
        "source_after must contain y=(px)20; got:\n{}",
        result.source_after
    );
    // Old distinctive coordinate (x=50, y=80) from original must be gone.
    assert!(
        !result.source_after.contains("y=(px)80"),
        "old y=(px)80 must not appear in source_after"
    );
    assert_ne!(result.source_before, result.source_after);
}

#[test]
fn set_points_too_few_rejected() {
    // Start from a valid 3-point polygon; replace with only 2 points →
    // post-validation rejects with shape.insufficient_points.
    let doc = parse(POLY_DOC);
    let tx = Transaction {
        ops: vec![Op::SetPoints {
            node: "poly".to_owned(),
            points: vec![OpPoint { x: 0.0, y: 0.0 }, OpPoint { x: 100.0, y: 0.0 }],
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "shape.insufficient_points"),
        "expected shape.insufficient_points diagnostic; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_points_unsupported_on_rect() {
    let doc = parse(RECT_GEOM_DOC);
    let tx = Transaction {
        ops: vec![Op::SetPoints {
            node: "rect".to_owned(),
            points: vec![
                OpPoint { x: 0.0, y: 0.0 },
                OpPoint { x: 100.0, y: 0.0 },
                OpPoint { x: 50.0, y: 80.0 },
            ],
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
        "expected tx.unsupported_property mentioning \"rect\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_path_anchors_replaces_path_with_handles() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::SetPathAnchors {
            node: "path1".to_owned(),
            subpath_index: None,
            anchors: vec![
                OpPathAnchor {
                    x: 10.0,
                    y: 20.0,
                    kind: Some("smooth".to_owned()),
                    in_x: None,
                    in_y: None,
                    out_x: Some(40.0),
                    out_y: Some(20.0),
                },
                OpPathAnchor {
                    x: 90.0,
                    y: 20.0,
                    kind: None,
                    in_x: Some(60.0),
                    in_y: Some(20.0),
                    out_x: None,
                    out_y: None,
                },
            ],
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["path1".to_owned()]);
    assert!(
        result
            .source_after
            .contains("anchor x=(px)10 y=(px)20 kind=\"smooth\" out-x=(px)40 out-y=(px)20"),
        "source_after must contain outgoing handle; got:\n{}",
        result.source_after
    );
    assert!(
        result
            .source_after
            .contains("anchor x=(px)90 y=(px)20 in-x=(px)60 in-y=(px)20"),
        "source_after must contain incoming handle; got:\n{}",
        result.source_after
    );
    assert_ne!(result.source_before, result.source_after);
}

#[test]
fn set_path_anchors_replaces_compound_subpath() {
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
        subpath {
          anchor x=(px)25 y=(px)25
          anchor x=(px)75 y=(px)25
          anchor x=(px)75 y=(px)75
        }
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::SetPathAnchors {
            node: "compound".to_owned(),
            subpath_index: Some(1),
            anchors: vec![
                OpPathAnchor {
                    x: 20.0,
                    y: 20.0,
                    kind: Some("corner".to_owned()),
                    in_x: None,
                    in_y: None,
                    out_x: None,
                    out_y: None,
                },
                OpPathAnchor {
                    x: 80.0,
                    y: 20.0,
                    kind: None,
                    in_x: None,
                    in_y: None,
                    out_x: None,
                    out_y: None,
                },
            ],
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["compound".to_owned()]);
    assert!(
        result.source_after.contains("anchor x=(px)0 y=(px)0"),
        "first subpath should be preserved; got:\n{}",
        result.source_after
    );
    assert!(
        result
            .source_after
            .contains("anchor x=(px)20 y=(px)20 kind=\"corner\""),
        "target subpath should be replaced; got:\n{}",
        result.source_after
    );
    assert!(
        !result.source_after.contains("anchor x=(px)25 y=(px)25"),
        "old target subpath anchors should be removed; got:\n{}",
        result.source_after
    );
}

#[test]
fn set_path_anchors_unsupported_on_rect_and_polygon() {
    for (src, node, kind) in [
        (RECT_GEOM_DOC, "rect", "rect"),
        (POLY_DOC, "poly", "polygon"),
    ] {
        let doc = parse(src);
        let tx = Transaction {
            ops: vec![Op::SetPathAnchors {
                node: node.to_owned(),
                subpath_index: None,
                anchors: vec![
                    OpPathAnchor {
                        x: 0.0,
                        y: 0.0,
                        kind: None,
                        in_x: None,
                        in_y: None,
                        out_x: None,
                        out_y: None,
                    },
                    OpPathAnchor {
                        x: 100.0,
                        y: 0.0,
                        kind: None,
                        in_x: None,
                        in_y: None,
                        out_x: None,
                        out_y: None,
                    },
                ],
            }],
            permissions: Permissions::default(),
        };
        let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

        assert_eq!(result.status, TxStatus::Rejected);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == "tx.unsupported_property" && d.message.contains(kind)),
            "expected tx.unsupported_property mentioning {kind:?}; got: {:?}",
            result.diagnostics
        );
        assert_eq!(result.source_after, result.source_before);
    }
}

#[test]
fn set_path_anchors_too_few_rejected_by_validation() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::SetPathAnchors {
            node: "path1".to_owned(),
            subpath_index: None,
            anchors: vec![OpPathAnchor {
                x: 0.0,
                y: 0.0,
                kind: None,
                in_x: None,
                in_y: None,
                out_x: None,
                out_y: None,
            }],
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "shape.insufficient_points"),
        "expected shape.insufficient_points diagnostic; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_path_anchors_incomplete_handle_pair_rejected_by_validation() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::SetPathAnchors {
            node: "path1".to_owned(),
            subpath_index: None,
            anchors: vec![
                OpPathAnchor {
                    x: 0.0,
                    y: 0.0,
                    kind: None,
                    in_x: None,
                    in_y: None,
                    out_x: Some(40.0),
                    out_y: None,
                },
                OpPathAnchor {
                    x: 100.0,
                    y: 0.0,
                    kind: None,
                    in_x: None,
                    in_y: None,
                    out_x: None,
                    out_y: None,
                },
            ],
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "node.invalid_geometry"),
        "expected node.invalid_geometry diagnostic; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}
