mod common;
use common::*;
use zenith_tx::{Op, Permissions, Transaction, TxStatus, run_transaction};

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

#[test]
fn simplify_path_anchors_rejects_paths_with_handles() {
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      path id="path1" {
        anchor x=(px)0 y=(px)0 out-x=(px)20 out-y=(px)0
        anchor x=(px)100 y=(px)0
      }
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::SimplifyPathAnchors {
            node: "path1".to_owned(),
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
            .any(|d| d.code == "tx.unsupported_path_handles"),
        "expected tx.unsupported_path_handles; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
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
