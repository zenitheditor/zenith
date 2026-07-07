mod common;
use common::*;
use zenith_tx::op::OpPathAnchor;
use zenith_tx::{Op, OpPoint, Permissions, Transaction, TxStatus, run_transaction};

// ── 1. SetTextAlign: accepted, affected ids, source diff ──────────────────

#[test]
fn set_text_align_accepted() {
    let doc = parse(TEXT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "label".to_owned(),
            align: "center".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["label".to_owned()]);
    assert!(
        result.source_after.contains("center"),
        "source_after should contain align=\"center\""
    );
    assert!(
        !result.source_before.contains("center"),
        "source_before should not contain center"
    );
    assert_ne!(result.source_before, result.source_after);
}

// ── 5. SetTextAlign on a rect → wrong_node_type, Rejected ────────────────

#[test]
fn set_text_align_wrong_node_type() {
    let doc = parse(MIXED_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "box1".to_owned(),
            align: "center".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.wrong_node_type"),
        "expected tx.wrong_node_type diagnostic"
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── 5b. SetTextAlign on an ellipse → wrong_node_type, Rejected ───────────

#[test]
fn set_text_align_on_ellipse_wrong_node_type() {
    let doc = parse(ELLIPSE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "dot".to_owned(),
            align: "center".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.wrong_node_type" && d.message.contains("ellipse")),
        "expected tx.wrong_node_type diagnostic naming the ellipse kind"
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── 5c. SetTextAlign on an image → wrong_node_type, Rejected ─────────────

#[test]
fn set_text_align_on_image_wrong_node_type() {
    let doc = parse(IMAGE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "pic".to_owned(),
            align: "center".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.wrong_node_type" && d.message.contains("image")),
        "expected tx.wrong_node_type diagnostic naming the image kind"
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── SetTextAlign: recursion into group children ───────────────────────────

#[test]
fn tx_set_text_align_targets_nested_text() {
    // A text node nested inside a group should now be reachable via
    // recursive descent; the tx engine is no longer limited to top-level
    // page children.
    let doc = parse(GROUP_TEXT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "nested.label".to_owned(),
            align: "center".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["nested.label".to_owned()]);
    assert!(
        result.source_after.contains("center"),
        "source_after should contain align=\"center\""
    );
    assert!(!result.source_before.contains("center"));
    assert_ne!(result.source_before, result.source_after);
}

#[test]
fn tx_set_text_align_on_group_itself_wrong_type() {
    // Targeting the group's own id with SetTextAlign must yield
    // tx.wrong_node_type mentioning "group".
    let doc = parse(GROUP_TEXT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "grp1".to_owned(),
            align: "center".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.wrong_node_type" && d.message.contains("group")),
        "expected tx.wrong_node_type diagnostic naming \"group\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── SetFill tests ─────────────────────────────────────────────────────────

#[test]
fn set_fill_recolors_rect() {
    let doc = parse(FILL_DOC);
    let tx = Transaction {
        ops: vec![Op::SetFill {
            node: "r1".to_owned(),
            fill: "color.b".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["r1".to_owned()]);
    // TokenRef("color.b") serialises as fill=(token)"color.b"
    assert!(
        result.source_after.contains("(token)\"color.b\""),
        "source_after must reference color.b; got:\n{}",
        result.source_after
    );
    assert!(
        !result.source_after.contains("(token)\"color.a\""),
        "old token must not appear in source_after"
    );
    assert_ne!(result.source_before, result.source_after);
}

#[test]
fn set_fill_unsupported_on_line() {
    let doc = parse(LINE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetFill {
            node: "ln1".to_owned(),
            fill: "color.a".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property" && d.message.contains("line")),
        "expected tx.unsupported_property mentioning \"line\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_fill_unknown_token_rejected() {
    // color.nope is not declared → post-validate emits token.unknown_reference → Rejected
    let doc = parse(FILL_DOC);
    let tx = Transaction {
        ops: vec![Op::SetFill {
            node: "r1".to_owned(),
            fill: "color.nope".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "token.unknown_reference"),
        "expected token.unknown_reference diagnostic; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── SetStroke / SetStrokeWidth tests ──────────────────────────────────────

#[test]
fn set_stroke_recolors_rect() {
    let doc = parse(STROKE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetStroke {
            node: "r1".to_owned(),
            stroke: "color.rule".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["r1".to_owned()]);
    assert!(
        result.source_after.contains("stroke=(token)\"color.rule\""),
        "source_after must reference color.rule as stroke; got:\n{}",
        result.source_after
    );
}

#[test]
fn set_stroke_unknown_token_rejected() {
    // color.nope is not declared → post-validate emits token.unknown_reference.
    let doc = parse(STROKE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetStroke {
            node: "r1".to_owned(),
            stroke: "color.nope".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "token.unknown_reference"),
        "expected token.unknown_reference diagnostic; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_stroke_accepted_on_ellipse() {
    // Ellipse now supports stroke — set_stroke must be Accepted.
    let doc = parse(STROKE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetStroke {
            node: "dot".to_owned(),
            stroke: "color.rule".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "set_stroke on an ellipse must be Accepted; got: {:?}",
        result.diagnostics
    );
    assert!(
        result.source_after.contains("stroke=(token)\"color.rule\""),
        "formatted source must contain the new stroke property; got:\n{}",
        result.source_after
    );
}

#[test]
fn set_stroke_unknown_node_rejected() {
    let doc = parse(STROKE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetStroke {
            node: "nope".to_owned(),
            stroke: "color.rule".to_owned(),
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
        "expected tx.unknown_node diagnostic; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_stroke_width_on_polygon() {
    let doc = parse(STROKE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetStrokeWidth {
            node: "poly1".to_owned(),
            stroke_width: "size.stroke".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["poly1".to_owned()]);
    assert!(
        result
            .source_after
            .contains("stroke-width=(token)\"size.stroke\""),
        "source_after must reference size.stroke as stroke-width; got:\n{}",
        result.source_after
    );
}

#[test]
fn set_stroke_width_unsupported_on_text() {
    let doc = parse(STROKE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetStrokeWidth {
            node: "lbl".to_owned(),
            stroke_width: "size.stroke".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property"
                && d.message
                    .contains("set_stroke_width is not supported on a text node")),
        "expected tx.unsupported_property naming text; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

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

// `UnknownNode` has no `id` field, so `node_id_of` returns `None` for it.
// `subtree_contains` will never match an unknown node by id, and
// `find_node_any_mut` returns `None` → tx.unknown_node.
// We verify this by targeting a non-existent id that would match an unknown
// node if it had an id; since it doesn't, we just get tx.unknown_node.
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

// ── SetGeometry tests ─────────────────────────────────────────────────────

#[test]
fn set_geometry_moves_rect() {
    let doc = parse(RECT_GEOM_DOC);
    let tx = Transaction {
        ops: vec![Op::SetGeometry {
            node: "rect".to_owned(),
            x: Some(50.0),
            y: None,
            w: Some(200.0),
            h: None,
            rotate: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["rect".to_owned()]);

    // Changed fields appear in source_after.
    assert!(
        result.source_after.contains("x=(px)50"),
        "source_after must contain x=(px)50; got:\n{}",
        result.source_after
    );
    assert!(
        result.source_after.contains("w=(px)200"),
        "source_after must contain w=(px)200; got:\n{}",
        result.source_after
    );
    // Untouched fields stay at their original values.
    assert!(
        result.source_after.contains("y=(px)0"),
        "source_after must retain y=(px)0; got:\n{}",
        result.source_after
    );
    assert!(
        result.source_after.contains("h=(px)100"),
        "source_after must retain h=(px)100; got:\n{}",
        result.source_after
    );
    assert_ne!(result.source_before, result.source_after);
}

#[test]
fn set_geometry_unsupported_on_line() {
    let doc = parse(LINE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetGeometry {
            node: "ln1".to_owned(),
            x: Some(10.0),
            y: None,
            w: None,
            h: None,
            rotate: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property" && d.message.contains("line")),
        "expected tx.unsupported_property mentioning \"line\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_geometry_no_fields_is_noop() {
    let doc = parse(RECT_GEOM_DOC);
    let tx = Transaction {
        ops: vec![Op::SetGeometry {
            node: "rect".to_owned(),
            x: None,
            y: None,
            w: None,
            h: None,
            rotate: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    // All-None must produce Accepted (advisory is not an error/warning) with
    // no affected nodes and identical source.
    assert_eq!(result.status, TxStatus::Accepted);
    assert!(
        result.affected_node_ids.is_empty(),
        "affected must be empty for a noop; got: {:?}",
        result.affected_node_ids
    );
    assert!(
        result.diagnostics.iter().any(|d| d.code == "tx.noop"),
        "expected tx.noop advisory; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── Code node tx tests ────────────────────────────────────────────────────

#[test]
fn set_visible_on_code_accepted() {
    let doc = parse(CODE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetVisible {
            node: "snip".to_owned(),
            visible: false,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["snip".to_owned()]);
    assert!(
        result.source_after.contains("visible=#false"),
        "source_after must contain visible=#false; got:\n{}",
        result.source_after
    );
    // Content blob must survive the edit untouched.
    assert!(result.source_after.contains("content \"fn main() {}\""));
}

#[test]
fn set_fill_on_code_accepted() {
    let doc = parse(CODE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetFill {
            node: "snip".to_owned(),
            fill: "color.b".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["snip".to_owned()]);
    assert!(
        result.source_after.contains("(token)\"color.b\""),
        "source_after must reference color.b; got:\n{}",
        result.source_after
    );
}

#[test]
fn set_geometry_supported_on_code() {
    let doc = parse(CODE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetGeometry {
            node: "snip".to_owned(),
            x: Some(10.0),
            y: None,
            w: None,
            h: None,
            rotate: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["snip".to_owned()]);
    assert!(
        result.source_after.contains("x=(px)10"),
        "source_after must contain x=(px)10; got:\n{}",
        result.source_after
    );
    assert_ne!(result.source_after, result.source_before);
}

#[test]
fn set_geometry_supported_on_text() {
    let doc = parse(TEXT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetGeometry {
            node: "label".to_owned(),
            x: Some(-200.0),
            y: None,
            w: None,
            h: None,
            rotate: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["label".to_owned()]);
    assert!(
        result.source_after.contains("x=(px)-200"),
        "source_after must contain x=(px)-200; got:\n{}",
        result.source_after
    );
    assert_ne!(result.source_after, result.source_before);
}

// ── SetGeometry rotate tests ─────────────────────────────────────────────

#[test]
fn set_geometry_rotate_on_image_accepted() {
    let doc = parse(IMAGE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetGeometry {
            node: "pic".to_owned(),
            x: None,
            y: None,
            w: None,
            h: None,
            rotate: Some(45.0),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["pic".to_owned()]);
    assert!(
        result.source_after.contains("rotate=(deg)45"),
        "source_after must contain rotate=(deg)45; got:\n{}",
        result.source_after
    );
    assert_ne!(result.source_before, result.source_after);
}

#[test]
fn set_geometry_rotate_on_line_rejected() {
    let doc = parse(LINE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetGeometry {
            node: "ln1".to_owned(),
            x: None,
            y: None,
            w: None,
            h: None,
            rotate: Some(30.0),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property" && d.message.contains("line")),
        "expected tx.unsupported_property mentioning \"line\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

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
fn set_path_anchors_unsupported_on_rect_and_polygon() {
    for (src, node, kind) in [
        (RECT_GEOM_DOC, "rect", "rect"),
        (POLY_DOC, "poly", "polygon"),
    ] {
        let doc = parse(src);
        let tx = Transaction {
            ops: vec![Op::SetPathAnchors {
                node: node.to_owned(),
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

// ── SetTextOverflow tests ─────────────────────────────────────────────────

#[test]
fn set_text_overflow_on_text_accepted() {
    let doc = parse(TEXT_CODE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextOverflow {
            node_id: "body".to_owned(),
            overflow: "visible".to_owned(),
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
    assert_eq!(result.affected_node_ids, vec!["body".to_owned()]);
    assert!(
        result.source_after.contains("overflow=\"visible\""),
        "source_after should set overflow=\"visible\": {}",
        result.source_after
    );
}

#[test]
fn set_text_overflow_on_code_accepted() {
    let doc = parse(TEXT_CODE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextOverflow {
            node_id: "snip".to_owned(),
            overflow: "clip".to_owned(),
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
    assert_eq!(result.affected_node_ids, vec!["snip".to_owned()]);
    assert!(
        result.source_after.contains("overflow=\"clip\""),
        "source_after should set overflow=\"clip\": {}",
        result.source_after
    );
}

#[test]
fn set_text_overflow_invalid_value_rejected() {
    let doc = parse(TEXT_CODE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextOverflow {
            node_id: "body".to_owned(),
            overflow: "wrap".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_value" && d.message.contains("wrap")),
        "expected tx.invalid_value naming \"wrap\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_text_overflow_wrong_node_type_rejected() {
    let doc = parse(THREE_RECTS_DOC); // rects, no overflow field
    let tx = Transaction {
        ops: vec![Op::SetTextOverflow {
            node_id: "r1".to_owned(),
            overflow: "visible".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.wrong_node_type"),
        "expected tx.wrong_node_type; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn set_text_overflow_missing_node_rejected() {
    let doc = parse(TEXT_CODE_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextOverflow {
            node_id: "nope".to_owned(),
            overflow: "fit".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");
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
