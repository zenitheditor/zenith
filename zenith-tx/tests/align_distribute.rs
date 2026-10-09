mod common;
use common::parse;
#[path = "common/px_attr.rs"]
mod px_attr;
#[path = "common/three_rects_doc.rs"]
mod three_rects_doc;
use px_attr::extract_px_attr;
use three_rects_doc::THREE_RECTS_DOC;
use zenith_tx::{Op, Permissions, Transaction, TxStatus, run_transaction};

/// Doc with two rects and one group; the group has no resolvable bbox.
const RECTS_AND_GROUP_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="r1" x=(px)20 y=(px)0 w=(px)60 h=(px)40
      rect id="r2" x=(px)80 y=(px)0 w=(px)60 h=(px)40
      group id="grp1" { }
    }
  }
}"##;

/// Three rects unevenly placed on the x axis: positions 0, 30, 100, widths 20.
/// Span = (100+20) - 0 = 120. Σsizes = 60. gap = (120-60)/2 = 30.
/// Distributed leading edges: 0, 0+20+30=50, 50+20+30=100.
const DISTRIBUTE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="p1" x=(px)0 y=(px)0 w=(px)20 h=(px)20
      rect id="p2" x=(px)30 y=(px)0 w=(px)20 h=(px)20
      rect id="p3" x=(px)100 y=(px)0 w=(px)20 h=(px)20
    }
  }
}"##;

// ── AlignNodes tests ──────────────────────────────────────────────────────

// ── align "left" anchor "selection" → all get x = min(x) = 10 ───────────

#[test]
fn align_left_selection() {
    let doc = parse(THREE_RECTS_DOC);
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned(), "r2".to_owned(), "r3".to_owned()],
            align: "left".to_owned(),
            anchor: "selection".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    // All three nodes must be affected.
    assert!(result.affected_node_ids.contains(&"r1".to_owned()));
    assert!(result.affected_node_ids.contains(&"r2".to_owned()));
    assert!(result.affected_node_ids.contains(&"r3".to_owned()));

    // All three must have x = 10 (the minimum original x).
    for id in ["r1", "r2", "r3"] {
        let x = extract_px_attr(&result.source_after, id, "x")
            .unwrap_or_else(|| panic!("could not extract x for {id}"));
        assert!((x - 10.0).abs() < 1e-9, "expected x=10 for {id}, got {x}");
    }
}

// ── align "right" anchor "selection" → all right edges equal max(x+w) ───

#[test]
fn align_right_selection() {
    let doc = parse(THREE_RECTS_DOC);
    // ref_right = max(x+w) = max(90, 130, 170) = 170
    // each node: x = 170 - 80 = 90
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned(), "r2".to_owned(), "r3".to_owned()],
            align: "right".to_owned(),
            anchor: "selection".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    for id in ["r1", "r2", "r3"] {
        let x = extract_px_attr(&result.source_after, id, "x")
            .unwrap_or_else(|| panic!("could not extract x for {id}"));
        // ref_right=170, w=80 → x = 90
        assert!((x - 90.0).abs() < 1e-9, "expected x=90 for {id}, got {x}");
    }
}

// ── align "hcenter" anchor "page" → x = page_w/2 − w/2 ──────────────────

#[test]
fn align_hcenter_page() {
    let doc = parse(THREE_RECTS_DOC);
    // page_w=400, each rect w=80 → centered x = 400/2 − 80/2 = 200 − 40 = 160
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned(), "r2".to_owned(), "r3".to_owned()],
            align: "hcenter".to_owned(),
            anchor: "page".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    for id in ["r1", "r2", "r3"] {
        let x = extract_px_attr(&result.source_after, id, "x")
            .unwrap_or_else(|| panic!("could not extract x for {id}"));
        assert!((x - 160.0).abs() < 1e-9, "expected x=160 for {id}, got {x}");
    }
}

// ── node without geometry (group) in the set → skipped, others aligned ───

#[test]
fn align_skips_non_geometry_node() {
    let doc = parse(RECTS_AND_GROUP_DOC);
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned(), "grp1".to_owned(), "r2".to_owned()],
            align: "left".to_owned(),
            anchor: "selection".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    // grp1 skipped → advisory, but the tx is still accepted.
    assert_eq!(
        result.status,
        TxStatus::AcceptedWithWarnings,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.geometry_unresolved" && d.message.contains("grp1")),
        "expected tx.geometry_unresolved advisory for grp1; got: {:?}",
        result.diagnostics
    );
    // r1 and r2 must still have been aligned (x=20, the minimum).
    for id in ["r1", "r2"] {
        let x = extract_px_attr(&result.source_after, id, "x")
            .unwrap_or_else(|| panic!("could not extract x for {id}"));
        assert!((x - 20.0).abs() < 1e-9, "expected x=20 for {id}, got {x}");
    }
    // grp1 must not appear in affected.
    assert!(
        !result.affected_node_ids.contains(&"grp1".to_owned()),
        "grp1 must not be in affected_node_ids"
    );
}

// ── unknown align value → tx.unsupported_property, rejected ──────────────

#[test]
fn align_nodes_unknown_align_rejected() {
    let doc = parse(THREE_RECTS_DOC);
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned()],
            align: "diagonal".to_owned(),
            anchor: "selection".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property" && d.message.contains("diagonal")),
        "expected tx.unsupported_property naming \"diagonal\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── AlignNodes: explicit-dimension anchor "(px)N" ─────────────────────────

// ── align "left" anchor "(px)120" → all left edges become 120 ─────────────

#[test]
fn align_left_dimension_anchor() {
    let doc = parse(THREE_RECTS_DOC);
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned(), "r2".to_owned(), "r3".to_owned()],
            align: "left".to_owned(),
            anchor: "(px)120".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    for id in ["r1", "r2", "r3"] {
        let x = extract_px_attr(&result.source_after, id, "x")
            .unwrap_or_else(|| panic!("could not extract x for {id}"));
        assert!((x - 120.0).abs() < 1e-9, "expected x=120 for {id}, got {x}");
    }
}

// ── vertical edge "top" anchor "(px)55" → all top edges (y) become 55 ─────

#[test]
fn align_top_dimension_anchor_sets_y() {
    let doc = parse(THREE_RECTS_DOC);
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned(), "r2".to_owned(), "r3".to_owned()],
            align: "top".to_owned(),
            anchor: "(px)55".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    for id in ["r1", "r2", "r3"] {
        let y = extract_px_attr(&result.source_after, id, "y")
            .unwrap_or_else(|| panic!("could not extract y for {id}"));
        assert!((y - 55.0).abs() < 1e-9, "expected y=55 for {id}, got {y}");
        // x must be untouched by a vertical-axis align.
    }
    // x of r2 should still be its original 50 (vertical align leaves x alone).
    let x2 = extract_px_attr(&result.source_after, "r2", "x").expect("x for r2");
    assert!(
        (x2 - 50.0).abs() < 1e-9,
        "expected x unchanged (50), got {x2}"
    );
}

// ── right edge "right" anchor "(px)200" → x = 200 - w ─────────────────────

#[test]
fn align_right_dimension_anchor() {
    let doc = parse(THREE_RECTS_DOC); // each w=80
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned(), "r2".to_owned(), "r3".to_owned()],
            align: "right".to_owned(),
            anchor: "(px)200".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    for id in ["r1", "r2", "r3"] {
        let x = extract_px_attr(&result.source_after, id, "x")
            .unwrap_or_else(|| panic!("could not extract x for {id}"));
        // right edge = 200, w = 80 → x = 120
        assert!((x - 120.0).abs() < 1e-9, "expected x=120 for {id}, got {x}");
    }
}

// ── invalid dimension anchor → tx.invalid_value, rejected ─────────────────

#[test]
fn align_invalid_dimension_anchor_rejected() {
    let doc = parse(THREE_RECTS_DOC);
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned()],
            align: "left".to_owned(),
            anchor: "(px)notanumber".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_value"),
        "expected tx.invalid_value; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── existing "page" / "selection" anchors still work ──────────────────────

#[test]
fn align_page_anchor_still_works() {
    let doc = parse(THREE_RECTS_DOC);
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned(), "r2".to_owned(), "r3".to_owned()],
            align: "left".to_owned(),
            anchor: "page".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    // page left edge = 0 → all x = 0.
    for id in ["r1", "r2", "r3"] {
        let x = extract_px_attr(&result.source_after, id, "x").expect("x");
        assert!((x - 0.0).abs() < 1e-9, "expected x=0 for {id}, got {x}");
    }
}

#[test]
fn align_selection_anchor_still_works() {
    let doc = parse(THREE_RECTS_DOC);
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["r1".to_owned(), "r2".to_owned(), "r3".to_owned()],
            align: "left".to_owned(),
            anchor: "selection".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    // selection min x = 10.
    for id in ["r1", "r2", "r3"] {
        let x = extract_px_attr(&result.source_after, id, "x").expect("x");
        assert!((x - 10.0).abs() < 1e-9, "expected x=10 for {id}, got {x}");
    }
}

// ── DistributeNodes tests ─────────────────────────────────────────────────

#[test]
fn distribute_horizontal_equal_gaps() {
    let doc = parse(DISTRIBUTE_DOC);
    let tx = Transaction {
        ops: vec![Op::DistributeNodes {
            node_ids: vec!["p1".to_owned(), "p2".to_owned(), "p3".to_owned()],
            axis: "horizontal".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );

    let x1 = extract_px_attr(&result.source_after, "p1", "x").expect("x1");
    let x2 = extract_px_attr(&result.source_after, "p2", "x").expect("x2");
    let x3 = extract_px_attr(&result.source_after, "p3", "x").expect("x3");

    // Endpoints fixed.
    assert!((x1 - 0.0).abs() < 1e-9, "x1 should stay 0, got {x1}");
    assert!((x3 - 100.0).abs() < 1e-9, "x3 should stay 100, got {x3}");
    // Middle node placed for equal gaps: 50.
    assert!((x2 - 50.0).abs() < 1e-9, "x2 should be 50, got {x2}");

    // Gaps between consecutive trailing/leading edges must be equal (= 30).
    let gap_a = x2 - (x1 + 20.0);
    let gap_b = x3 - (x2 + 20.0);
    assert!(
        (gap_a - gap_b).abs() < 1e-9,
        "gaps unequal: {gap_a} vs {gap_b}"
    );
    assert!((gap_a - 30.0).abs() < 1e-9, "gap should be 30, got {gap_a}");
}

#[test]
fn distribute_orders_by_position_first() {
    // Same geometry but listed out of order: p3, p1, p2. Result must match the
    // position-ordered distribution (p1 fixed at 0, p3 fixed at 100, p2 at 50).
    let doc = parse(DISTRIBUTE_DOC);
    let tx = Transaction {
        ops: vec![Op::DistributeNodes {
            node_ids: vec!["p3".to_owned(), "p1".to_owned(), "p2".to_owned()],
            axis: "horizontal".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    let x1 = extract_px_attr(&result.source_after, "p1", "x").expect("x1");
    let x2 = extract_px_attr(&result.source_after, "p2", "x").expect("x2");
    let x3 = extract_px_attr(&result.source_after, "p3", "x").expect("x3");
    assert!((x1 - 0.0).abs() < 1e-9, "x1={x1}");
    assert!((x2 - 50.0).abs() < 1e-9, "x2={x2}");
    assert!((x3 - 100.0).abs() < 1e-9, "x3={x3}");
}

#[test]
fn distribute_too_few_nodes_is_noop() {
    let doc = parse(DISTRIBUTE_DOC);
    let tx = Transaction {
        ops: vec![Op::DistributeNodes {
            node_ids: vec!["p1".to_owned(), "p2".to_owned()],
            axis: "horizontal".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    // Degenerate input → tx.noop advisory, document unchanged, still Accepted.
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    assert!(
        result.diagnostics.iter().any(|d| d.code == "tx.noop"),
        "expected tx.noop advisory; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
    assert!(result.affected_node_ids.is_empty());
}

#[test]
fn distribute_missing_node_rejected() {
    let doc = parse(DISTRIBUTE_DOC);
    let tx = Transaction {
        ops: vec![Op::DistributeNodes {
            node_ids: vec!["p1".to_owned(), "ghost".to_owned(), "p3".to_owned()],
            axis: "horizontal".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unknown_node" && d.message.contains("ghost")),
        "expected tx.unknown_node naming \"ghost\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn distribute_vertical_equal_gaps() {
    // Place three rects unevenly on y: 0, 30, 100, height 20. Same arithmetic.
    let doc = parse(
        r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="q1" x=(px)0 y=(px)0 w=(px)20 h=(px)20
      rect id="q2" x=(px)0 y=(px)30 w=(px)20 h=(px)20
      rect id="q3" x=(px)0 y=(px)100 w=(px)20 h=(px)20
    }
  }
}"##,
    );
    let tx = Transaction {
        ops: vec![Op::DistributeNodes {
            node_ids: vec!["q1".to_owned(), "q2".to_owned(), "q3".to_owned()],
            axis: "vertical".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    let y2 = extract_px_attr(&result.source_after, "q2", "y").expect("y2");
    assert!((y2 - 50.0).abs() < 1e-9, "y2 should be 50, got {y2}");
}

#[test]
fn distribute_unknown_axis_rejected() {
    let doc = parse(DISTRIBUTE_DOC);
    let tx = Transaction {
        ops: vec![Op::DistributeNodes {
            node_ids: vec!["p1".to_owned(), "p2".to_owned(), "p3".to_owned()],
            axis: "diagonal".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property" && d.message.contains("diagonal")),
        "expected tx.unsupported_property naming \"diagonal\"; got: {:?}",
        result.diagnostics
    );
}

// ── Instance geometry: align/distribute write path ───────────────────────────

const TWO_INSTANCES_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles {}
  components {
    component id="badge" {
      rect id="shape" x=(px)0 y=(px)0 w=(px)24 h=(px)24
    }
  }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      instance id="icon.a" component="badge" x=(px)10 y=(px)20 w=(px)48 h=(px)48
      instance id="icon.b" component="badge" x=(px)100 y=(px)80 w=(px)48 h=(px)48
    }
  }
}"##;

#[test]
fn align_left_selection_moves_instances() {
    let doc = parse(TWO_INSTANCES_DOC);
    let tx = Transaction {
        ops: vec![Op::AlignNodes {
            node_ids: vec!["icon.a".to_owned(), "icon.b".to_owned()],
            align: "left".to_owned(),
            anchor: "selection".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert!(result.affected_node_ids.contains(&"icon.a".to_owned()));
    assert!(result.affected_node_ids.contains(&"icon.b".to_owned()));
    for id in ["icon.a", "icon.b"] {
        let x = extract_px_attr(&result.source_after, id, "x")
            .unwrap_or_else(|| panic!("could not extract x for {id}"));
        assert!(
            (x - 10.0).abs() < 1e-9,
            "expected instance {id} x=10 after align, got {x}; source:\n{}",
            result.source_after
        );
    }
}
