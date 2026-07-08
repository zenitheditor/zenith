mod common;
use common::*;
use zenith_tx::op::OpPathTransform;
use zenith_tx::{Op, Permissions, Transaction, TxStatus, run_transaction};

fn anchor_line(source: &str, index: usize) -> &str {
    source
        .lines()
        .filter(|line| line.trim_start().starts_with("anchor "))
        .nth(index)
        .expect("anchor line should exist")
}

fn anchor_px_attr(source: &str, index: usize, attr: &str) -> f64 {
    let line = anchor_line(source, index);
    let needle = format!("{attr}=(px)");
    let start = line.find(&needle).expect("attribute should exist") + needle.len();
    let rest = &line[start..];
    let end = rest
        .find(|c: char| {
            !c.is_ascii_digit() && c != '.' && c != '-' && c != '+' && c != 'e' && c != 'E'
        })
        .unwrap_or(rest.len());
    rest[..end]
        .parse::<f64>()
        .expect("px attribute should parse")
}

fn assert_px_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1.0e-9,
        "expected {actual} to be within tolerance of {expected}"
    );
}

const TRANSFORM_PATH_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="path1" closed=#true {
        anchor x=(px)0 y=(px)0 kind="corner" out-x=(px)10 out-y=(px)0
        anchor x=(px)20 y=(px)0 kind="smooth" in-x=(px)10 in-y=(px)0
        anchor x=(px)0 y=(px)20
      }
    }
  }
}"##;

const COMPOUND_PATH_DOC: &str = r##"zenith version=1 {
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
        }
      }
    }
  }
}"##;

#[path = "path_ops/anchor_kind.rs"]
mod anchor_kind;
#[path = "path_ops/anchor_movement.rs"]
mod anchor_movement;
#[path = "path_ops/boolean.rs"]
mod boolean;
#[path = "path_ops/handle_movement.rs"]
mod handle_movement;
#[path = "path_ops/insertion.rs"]
mod insertion;
#[path = "path_ops/projection_insertion.rs"]
mod projection_insertion;
#[path = "path_ops/snap.rs"]
mod snap;
#[path = "path_ops/symmetry.rs"]
mod symmetry;

#[test]
fn transform_path_anchors_translates_anchors_and_handles() {
    let doc = parse(TRANSFORM_PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::TransformPathAnchors {
            node: "path1".to_owned(),
            transform: OpPathTransform::Translate { dx: 10.0, dy: -4.0 },
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["path1".to_owned()]);
    assert!(
        result
            .source_after
            .contains("path id=\"path1\" closed=#true")
    );
    assert_px_close(anchor_px_attr(&result.source_after, 0, "x"), 10.0);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "y"), -4.0);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "out-x"), 20.0);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "out-y"), -4.0);
    assert!(anchor_line(&result.source_after, 0).contains("kind=\"corner\""));
    assert_px_close(anchor_px_attr(&result.source_after, 1, "x"), 30.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "y"), -4.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "in-x"), 20.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "in-y"), -4.0);
    assert!(anchor_line(&result.source_after, 1).contains("kind=\"smooth\""));
}

#[test]
fn transform_path_anchors_rotates_around_pivot() {
    let doc = parse(TRANSFORM_PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::TransformPathAnchors {
            node: "path1".to_owned(),
            transform: OpPathTransform::Rotate {
                angle_degrees: 90.0,
                cx: 0.0,
                cy: 0.0,
            },
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "x"), 0.0);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "y"), 0.0);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "out-x"), 0.0);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "out-y"), 10.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "x"), 0.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "y"), 20.0);
}

#[test]
fn transform_path_anchors_reflects_across_line() {
    let doc = parse(TRANSFORM_PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::TransformPathAnchors {
            node: "path1".to_owned(),
            transform: OpPathTransform::Reflect {
                x1: 0.0,
                y1: 0.0,
                x2: 0.0,
                y2: 10.0,
            },
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "x"), 0.0);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "out-x"), -10.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "x"), -20.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "in-x"), -10.0);
}

#[test]
fn transform_path_anchors_translates_compound_subpaths() {
    let doc = parse(COMPOUND_PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::TransformPathAnchors {
            node: "compound".to_owned(),
            transform: OpPathTransform::Translate { dx: 10.0, dy: -5.0 },
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["compound".to_owned()]);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "x"), 10.0);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "y"), -5.0);
    assert_px_close(anchor_px_attr(&result.source_after, 3, "x"), 35.0);
    assert_px_close(anchor_px_attr(&result.source_after, 3, "y"), 20.0);
    assert!(
        result.source_after.contains("fill-rule=\"evenodd\""),
        "compound path styling should be preserved; got:\n{}",
        result.source_after
    );
}

#[test]
fn transform_path_anchors_unsupported_on_rect() {
    let doc = parse(RECT_GEOM_DOC);
    let tx = Transaction {
        ops: vec![Op::TransformPathAnchors {
            node: "rect".to_owned(),
            transform: OpPathTransform::Translate { dx: 1.0, dy: 2.0 },
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
fn transform_path_anchors_unknown_node_rejected() {
    let doc = parse(TRANSFORM_PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::TransformPathAnchors {
            node: "missing".to_owned(),
            transform: OpPathTransform::Translate { dx: 1.0, dy: 2.0 },
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

#[test]
fn transform_path_anchors_locked_path_rejected() {
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
        ops: vec![Op::TransformPathAnchors {
            node: "path1".to_owned(),
            transform: OpPathTransform::Translate { dx: 1.0, dy: 2.0 },
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
fn transform_path_anchors_degenerate_reflect_line_rejected() {
    let doc = parse(TRANSFORM_PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::TransformPathAnchors {
            node: "path1".to_owned(),
            transform: OpPathTransform::Reflect {
                x1: 1.0,
                y1: 1.0,
                x2: 1.0,
                y2: 1.0,
            },
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result.diagnostics.iter().any(|d| {
            d.code == "tx.invalid_geometry" && d.message.contains("two distinct points")
        }),
        "expected tx.invalid_geometry for degenerate reflect line; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn transform_path_anchors_non_finite_parameter_rejected() {
    let doc = parse(TRANSFORM_PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::TransformPathAnchors {
            node: "path1".to_owned(),
            transform: OpPathTransform::Translate {
                dx: f64::NAN,
                dy: 2.0,
            },
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result.diagnostics.iter().any(|d| {
            d.code == "tx.invalid_geometry" && d.message.contains("parameters must be finite")
        }),
        "expected tx.invalid_geometry for non-finite parameter; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn transform_path_anchors_non_px_anchor_rejected() {
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
        ops: vec![Op::TransformPathAnchors {
            node: "path1".to_owned(),
            transform: OpPathTransform::Translate { dx: 1.0, dy: 2.0 },
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

#[test]
fn transform_path_anchors_incomplete_handle_rejected() {
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
        ops: vec![Op::TransformPathAnchors {
            node: "path1".to_owned(),
            transform: OpPathTransform::Translate { dx: 1.0, dy: 2.0 },
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_path_anchor"),
        "expected tx.invalid_path_anchor; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn simplify_path_anchors_removes_near_collinear_middle_anchor() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="path1" {
        anchor x=(px)0 y=(px)0
        anchor x=(px)50 y=(px)0.1 kind="smooth"
        anchor x=(px)100 y=(px)0
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::SimplifyPathAnchors {
            node: "path1".to_owned(),
            subpath_index: None,
            tolerance: 0.5,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["path1".to_owned()]);
    assert!(result.source_after.contains("anchor x=(px)0 y=(px)0"));
    assert!(result.source_after.contains("anchor x=(px)100 y=(px)0"));
    assert!(
        !result.source_after.contains("anchor x=(px)50 y=(px)0.1"),
        "middle anchor should be removed; got:\n{}",
        result.source_after
    );
    assert!(
        !result.source_after.contains("kind="),
        "simplified handle-free anchors should not preserve smooth/symmetric intent; got:\n{}",
        result.source_after
    );
}

#[test]
fn simplify_path_anchors_requires_subpath_index_for_compound_path() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="compound" {
        subpath {
          anchor x=(px)0 y=(px)0
          anchor x=(px)10 y=(px)0
        }
        subpath {
          anchor x=(px)20 y=(px)0
          anchor x=(px)30 y=(px)0.1
          anchor x=(px)40 y=(px)0
        }
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::SimplifyPathAnchors {
            node: "compound".to_owned(),
            subpath_index: None,
            tolerance: 0.5,
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
        "expected compound subpath_index diagnostic; got {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn simplify_path_anchors_targets_compound_subpath() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="compound" {
        subpath {
          anchor x=(px)0 y=(px)0
          anchor x=(px)10 y=(px)0
        }
        subpath {
          anchor x=(px)20 y=(px)0
          anchor x=(px)30 y=(px)0.1 kind="smooth"
          anchor x=(px)40 y=(px)0
        }
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::SimplifyPathAnchors {
            node: "compound".to_owned(),
            subpath_index: Some(1),
            tolerance: 0.5,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["compound".to_owned()]);
    assert_eq!(formatted_anchor_count(&result.source_after), 4);
    assert_px_close(anchor_px_attr(&result.source_after, 0, "x"), 0.0);
    assert_px_close(anchor_px_attr(&result.source_after, 1, "x"), 10.0);
    assert_px_close(anchor_px_attr(&result.source_after, 2, "x"), 20.0);
    assert_px_close(anchor_px_attr(&result.source_after, 3, "x"), 40.0);
    assert!(
        !result.source_after.contains("anchor x=(px)30 y=(px)0.1"),
        "target subpath middle anchor should be removed; got:\n{}",
        result.source_after
    );
}

#[test]
fn simplify_path_anchors_preserves_far_bend() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="path1" {
        anchor x=(px)0 y=(px)0
        anchor x=(px)50 y=(px)40
        anchor x=(px)100 y=(px)0
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::SimplifyPathAnchors {
            node: "path1".to_owned(),
            subpath_index: None,
            tolerance: 0.5,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert!(
        result.source_after.contains("anchor x=(px)50 y=(px)40"),
        "far bend should be preserved; got:\n{}",
        result.source_after
    );
}

fn formatted_anchor_count(source: &str) -> usize {
    source.matches("anchor ").count()
}

#[test]
fn simplify_path_anchors_accepts_open_cubic_paths() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="path1" {
        anchor x=(px)0 y=(px)0 out-x=(px)0 out-y=(px)100
        anchor x=(px)100 y=(px)0 in-x=(px)100 in-y=(px)100
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::SimplifyPathAnchors {
            node: "path1".to_owned(),
            subpath_index: None,
            tolerance: 0.5,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["path1".to_owned()]);
    assert!(result.source_after.contains("anchor x=(px)0 y=(px)0"));
    assert!(result.source_after.contains("anchor x=(px)100 y=(px)0"));
    assert!(
        formatted_anchor_count(&result.source_after) > 2,
        "curve fitting should retain editable approximation anchors; got:\n{}",
        result.source_after
    );
    assert!(
        result.source_after.contains("in-x=") && result.source_after.contains("out-x="),
        "handled simplification should fit editable cubic handles; got:\n{}",
        result.source_after
    );
}

#[test]
fn simplify_path_anchors_accepts_mixed_line_cubic_line_paths() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="path1" {
        anchor x=(px)0 y=(px)0
        anchor x=(px)50 y=(px)0 out-x=(px)50 out-y=(px)80
        anchor x=(px)150 y=(px)0 in-x=(px)150 in-y=(px)80
        anchor x=(px)200 y=(px)0
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::SimplifyPathAnchors {
            node: "path1".to_owned(),
            subpath_index: None,
            tolerance: 0.5,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["path1".to_owned()]);
    assert!(result.source_after.contains("anchor x=(px)0 y=(px)0"));
    assert!(result.source_after.contains("anchor x=(px)50 y=(px)0"));
    assert!(result.source_after.contains("anchor x=(px)150 y=(px)0"));
    assert!(result.source_after.contains("anchor x=(px)200 y=(px)0"));
    assert!(
        formatted_anchor_count(&result.source_after) > 4,
        "mixed path should keep endpoints and retain editable approximation anchors; got:\n{}",
        result.source_after
    );
    assert!(
        result.source_after.contains("in-x=") && result.source_after.contains("out-x="),
        "handled simplification should fit editable cubic handles; got:\n{}",
        result.source_after
    );
}

#[test]
fn simplify_path_anchors_rejects_closed_paths() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="path1" closed=#true {
        anchor x=(px)0 y=(px)0
        anchor x=(px)50 y=(px)0
        anchor x=(px)25 y=(px)40
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::SimplifyPathAnchors {
            node: "path1".to_owned(),
            subpath_index: None,
            tolerance: 0.5,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_closed_path"),
        "expected tx.unsupported_closed_path; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn simplify_path_anchors_unsupported_on_rect() {
    let doc = parse(RECT_GEOM_DOC);
    let tx = Transaction {
        ops: vec![Op::SimplifyPathAnchors {
            node: "rect".to_owned(),
            subpath_index: None,
            tolerance: 0.5,
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
fn simplify_path_anchors_invalid_tolerance_rejected() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::SimplifyPathAnchors {
            node: "path1".to_owned(),
            subpath_index: None,
            tolerance: 0.0,
        }],
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
fn simplify_path_anchors_locked_path_rejected() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="path1" locked=#true {
        anchor x=(px)0 y=(px)0
        anchor x=(px)50 y=(px)0.1
        anchor x=(px)100 y=(px)0
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::SimplifyPathAnchors {
            node: "path1".to_owned(),
            subpath_index: None,
            tolerance: 0.5,
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
fn simplify_path_anchors_preserves_open_path_minimum() {
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
        ops: vec![Op::SimplifyPathAnchors {
            node: "path1".to_owned(),
            subpath_index: None,
            tolerance: 1.0,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["path1".to_owned()]);
    assert!(result.source_after.contains("anchor x=(px)0 y=(px)0"));
    assert!(result.source_after.contains("anchor x=(px)100 y=(px)0"));
    assert!(
        !result.source_after.contains("anchor x=(px)50 y=(px)0"),
        "open paths remain valid with two anchors; got:\n{}",
        result.source_after
    );
}

#[test]
fn simplify_path_anchors_unknown_node_rejected() {
    let doc = parse(PATH_DOC);
    let tx = Transaction {
        ops: vec![Op::SimplifyPathAnchors {
            node: "missing".to_owned(),
            subpath_index: None,
            tolerance: 0.5,
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
