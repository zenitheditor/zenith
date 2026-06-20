mod common;
use common::*;
use zenith_tx::{
    Op, OpPoint, OpSpan, Permissions, Position, Transaction, TxStatus, run_transaction,
};

// ── 2. from_json round-trip ───────────────────────────────────────────────

#[test]
fn from_json_round_trip() {
    let json = r#"{"ops":[{"op":"set_text_align","node":"label","align":"center"},{"op":"move_forward","node":"accent"}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![
                Op::SetTextAlign {
                    node: "label".to_owned(),
                    align: "center".to_owned(),
                },
                Op::MoveForward {
                    node: "accent".to_owned()
                },
            ],
            permissions: Permissions::default(),
        }
    );
}

// ── 4. Unknown node id → Rejected ────────────────────────────────────────

#[test]
fn unknown_node_rejected() {
    let doc = parse(TEXT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "does_not_exist".to_owned(),
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
            .any(|d| d.code == "tx.unknown_node"),
        "expected tx.unknown_node diagnostic"
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── 6. Invalid align value → tx.invalid_value, Rejected ──────────────────

#[test]
fn invalid_align_value_rejected() {
    let doc = parse(TEXT_DOC);
    let tx = Transaction {
        ops: vec![Op::SetTextAlign {
            node: "label".to_owned(),
            align: "middle".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_value"),
        "expected tx.invalid_value diagnostic"
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── from_json round-trip: new op variants ─────────────────────────────────

#[test]
fn from_json_new_ops_round_trip() {
    let json = r#"{"ops":[
            {"op":"set_fill","node":"r","fill":"c"},
            {"op":"set_visible","node":"r","visible":false},
            {"op":"set_locked","node":"r","locked":true},
            {"op":"set_stroke","node":"r","stroke":"s"},
            {"op":"set_stroke_width","node":"r","stroke_width":"sw"},
            {"op":"add_node","parent":"pg1","position":{"at":"after","id":"r"},"source":"rect id=\"r2\""},
            {"op":"remove_node","node":"r"}
        ]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![
                Op::SetFill {
                    node: "r".to_owned(),
                    fill: "c".to_owned(),
                },
                Op::SetVisible {
                    node: "r".to_owned(),
                    visible: false,
                },
                Op::SetLocked {
                    node: "r".to_owned(),
                    locked: true,
                },
                Op::SetStroke {
                    node: "r".to_owned(),
                    stroke: "s".to_owned(),
                },
                Op::SetStrokeWidth {
                    node: "r".to_owned(),
                    stroke_width: "sw".to_owned(),
                },
                Op::AddNode {
                    parent: "pg1".to_owned(),
                    position: Position::After { id: "r".to_owned() },
                    source: "rect id=\"r2\"".to_owned(),
                },
                Op::RemoveNode {
                    node: "r".to_owned(),
                },
            ],
            permissions: Permissions::default(),
        }
    );
}

#[test]
fn from_json_add_node_position_defaults_to_last() {
    // `position` omitted → serde default → Position::Last.
    let json = r#"{"ops":[{"op":"add_node","parent":"pg1","source":"rect id=\"r2\""}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![Op::AddNode {
                parent: "pg1".to_owned(),
                position: Position::Last,
                source: "rect id=\"r2\"".to_owned(),
            }],
            permissions: Permissions::default(),
        }
    );
}

// ── JSON round-trip: reshape ops ─────────────────────────────────────────

#[test]
fn from_json_reshape_ops_round_trip() {
    let json_geo = r#"{"ops":[{"op":"set_geometry","node":"r","x":10.0,"w":200.0}]}"#;
    let tx_geo = Transaction::from_json(json_geo).expect("parse set_geometry JSON");
    assert_eq!(
        tx_geo,
        Transaction {
            ops: vec![Op::SetGeometry {
                node: "r".to_owned(),
                x: Some(10.0),
                y: None,
                w: Some(200.0),
                h: None,
                rotate: None,
            }],
            permissions: Permissions::default(),
        }
    );

    let json_pts = r#"{"ops":[{"op":"set_points","node":"p","points":[{"x":0.0,"y":0.0},{"x":1.0,"y":1.0}]}]}"#;
    let tx_pts = Transaction::from_json(json_pts).expect("parse set_points JSON");
    assert_eq!(
        tx_pts,
        Transaction {
            ops: vec![Op::SetPoints {
                node: "p".to_owned(),
                points: vec![OpPoint { x: 0.0, y: 0.0 }, OpPoint { x: 1.0, y: 1.0 },],
            }],
            permissions: Permissions::default(),
        }
    );
}

// ── from_json round-trip: reorder ops ─────────────────────────────────────

#[test]
fn from_json_reorder_ops_round_trip() {
    let json = r#"{"ops":[
            {"op":"move_backward","node":"x"},
            {"op":"move_to_front","node":"x"},
            {"op":"move_to_back","node":"x"}
        ]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![
                Op::MoveBackward {
                    node: "x".to_owned()
                },
                Op::MoveToFront {
                    node: "x".to_owned()
                },
                Op::MoveToBack {
                    node: "x".to_owned()
                },
            ],
            permissions: Permissions::default(),
        }
    );
}

// ── from_json round-trip: crud ops ────────────────────────────────────────

#[test]
fn from_json_crud_ops_round_trip() {
    let json = r#"{"ops":[
            {"op":"add_node","parent":"pg1","source":"rect id=\"r\""},
            {"op":"add_node","parent":"pg1","position":{"at":"index","index":2},"source":"rect id=\"r2\""},
            {"op":"remove_node","node":"r"}
        ]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![
                Op::AddNode {
                    parent: "pg1".to_owned(),
                    position: Position::Last,
                    source: r#"rect id="r""#.to_owned(),
                },
                Op::AddNode {
                    parent: "pg1".to_owned(),
                    position: Position::Index { index: 2 },
                    source: r#"rect id="r2""#.to_owned(),
                },
                Op::RemoveNode {
                    node: "r".to_owned(),
                },
            ],
            permissions: Permissions::default(),
        }
    );
}

// ── from_json round-trip: set_opacity + replace_text ─────────────────────

#[test]
fn from_json_set_opacity_replace_text_round_trip() {
    let json = r#"{"ops":[
            {"op":"set_opacity","node":"box","opacity":0.75},
            {"op":"replace_text","node":"lbl","spans":[
                {"text":"Hello","fill":"color.brand","italic":true},
                {"text":" world","underline":false}
            ]}
        ]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![
                Op::SetOpacity {
                    node: "box".to_owned(),
                    opacity: 0.75,
                },
                Op::ReplaceText {
                    node: "lbl".to_owned(),
                    spans: vec![
                        OpSpan {
                            text: "Hello".to_owned(),
                            fill: Some("color.brand".to_owned()),
                            font_weight: None,
                            italic: Some(true),
                            underline: None,
                            strikethrough: None,
                            vertical_align: None,
                            footnote_ref: None,
                        },
                        OpSpan {
                            text: " world".to_owned(),
                            fill: None,
                            font_weight: None,
                            italic: None,
                            underline: Some(false),
                            strikethrough: None,
                            vertical_align: None,
                            footnote_ref: None,
                        },
                    ],
                },
            ],
            permissions: Permissions::default(),
        }
    );
}

// ── Extend the ops serde round-trip to include duplicate_node ─────────────

#[test]
fn from_json_duplicate_node_round_trip() {
    let json = r#"{"ops":[{"op":"duplicate_node","node":"orig","new_id":"orig-copy"}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![Op::DuplicateNode {
                node: "orig".to_owned(),
                new_id: "orig-copy".to_owned(),
            }],
            permissions: Permissions::default(),
        }
    );
}

/// Serde round-trip for the duplicate_page op.
#[test]
fn from_json_duplicate_page_round_trip() {
    let json =
        r#"{"ops":[{"op":"duplicate_page","page":"page.x","new_id":"page.x2","id_suffix":".v2"}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![Op::DuplicatePage {
                page: "page.x".to_owned(),
                new_id: "page.x2".to_owned(),
                id_suffix: ".v2".to_owned(),
            }],
            permissions: Permissions::default(),
        }
    );
}

#[test]
fn from_json_add_page_round_trip() {
    let json =
        r#"{"ops":[{"op":"add_page","id":"page.new","w":"(px)1800","h":"(px)1200","index":1}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![Op::AddPage {
                id: "page.new".to_owned(),
                w: "(px)1800".to_owned(),
                h: "(px)1200".to_owned(),
                background: None,
                index: Some(1),
            }],
            permissions: Permissions::default(),
        }
    );
}

#[test]
fn from_json_reorder_pages_round_trip() {
    let json = r#"{"ops":[{"op":"reorder_pages","order":["b","a"]}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![Op::ReorderPages {
                order: vec!["b".to_owned(), "a".to_owned()],
            }],
            permissions: Permissions::default(),
        }
    );
}

// ── Serde round-trip: group / ungroup / reparent ──────────────────────────

#[test]
fn from_json_group_ungroup_reparent_round_trip() {
    let json = r#"{"ops":[
            {"op":"group","node_ids":["r1","r2"],"group_id":"grp-new"},
            {"op":"ungroup","group_id":"grp1"},
            {"op":"reparent","node":"r3","new_parent":"grp1","position":{"at":"first"}}
        ]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![
                Op::Group {
                    node_ids: vec!["r1".to_owned(), "r2".to_owned()],
                    group_id: "grp-new".to_owned(),
                },
                Op::Ungroup {
                    group_id: "grp1".to_owned(),
                },
                Op::Reparent {
                    node: "r3".to_owned(),
                    new_parent: "grp1".to_owned(),
                    position: Position::First,
                },
            ],
            permissions: Permissions::default(),
        }
    );
}

// ── serde round-trip: align_nodes with default anchor ────────────────────

#[test]
fn from_json_align_nodes_round_trip() {
    // anchor is omitted → should deserialize to "selection" via serde default.
    let json = r#"{"ops":[{"op":"align_nodes","node_ids":["r1","r2"],"align":"left"}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![Op::AlignNodes {
                node_ids: vec!["r1".to_owned(), "r2".to_owned()],
                align: "left".to_owned(),
                anchor: "selection".to_owned(),
            }],
            permissions: Permissions::default(),
        }
    );
}

// ── serde round-trips for the new ops ─────────────────────────────────────

#[test]
fn from_json_set_text_overflow_round_trip() {
    let json = r#"{"ops":[{"op":"set_text_overflow","node_id":"body","overflow":"visible"}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![Op::SetTextOverflow {
                node_id: "body".to_owned(),
                overflow: "visible".to_owned(),
            }],
            permissions: Permissions::default(),
        }
    );
}

#[test]
fn from_json_distribute_nodes_round_trip() {
    let json =
        r#"{"ops":[{"op":"distribute_nodes","node_ids":["p1","p2","p3"],"axis":"horizontal"}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![Op::DistributeNodes {
                node_ids: vec!["p1".to_owned(), "p2".to_owned(), "p3".to_owned()],
                axis: "horizontal".to_owned(),
            }],
            permissions: Permissions::default(),
        }
    );
}

#[test]
fn from_json_align_dimension_anchor_round_trip() {
    let json = r#"{"ops":[{"op":"align_nodes","node_ids":["a","b","caption"],"align":"left","anchor":"(px)120"}]}"#;
    let tx = Transaction::from_json(json).expect("parse JSON");
    assert_eq!(
        tx,
        Transaction {
            ops: vec![Op::AlignNodes {
                node_ids: vec!["a".to_owned(), "b".to_owned(), "caption".to_owned()],
                align: "left".to_owned(),
                anchor: "(px)120".to_owned(),
            }],
            permissions: Permissions::default(),
        }
    );
}

// ── LOCKED-NODE enforcement ───────────────────────────────────────────────

#[test]
fn set_geometry_on_locked_node_rejected() {
    let doc = parse(TWO_RECT_DOC);

    // Two ops in one tx: lock the node, then try to move it. The lock check
    // reads the candidate state, so the earlier set_locked locks "a" for the
    // later set_geometry — which must therefore be rejected.
    let tx = Transaction {
        ops: vec![
            Op::SetLocked {
                node: "a".to_owned(),
                locked: true,
            },
            Op::SetGeometry {
                node: "a".to_owned(),
                x: Some(50.0),
                y: None,
                w: None,
                h: None,
                rotate: None,
            },
        ],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "node.locked" && d.subject_id.as_deref() == Some("a")),
        "expected a node.locked diagnostic naming 'a', got: {:?}",
        result.diagnostics
    );
    // Rejected ⇒ document unchanged.
    assert_eq!(result.source_before, result.source_after);
}

#[test]
fn set_geometry_on_locked_node_allowed_with_permission() {
    let doc = parse(TWO_RECT_DOC);

    let tx = Transaction {
        ops: vec![
            Op::SetLocked {
                node: "a".to_owned(),
                locked: true,
            },
            Op::SetGeometry {
                node: "a".to_owned(),
                x: Some(50.0),
                y: None,
                w: None,
                h: None,
                rotate: None,
            },
        ],
        permissions: Permissions {
            allow_locked: true,
            allow_raw_visual_literals: false,
        },
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert!(
        !result.diagnostics.iter().any(|d| d.code == "node.locked"),
        "no node.locked diagnostic expected with allow_locked, got: {:?}",
        result.diagnostics
    );
    // Geometry changed: x moved to 50.
    assert!(
        result.source_after.contains("(px)50"),
        "source_after should reflect the moved geometry: {}",
        result.source_after
    );
    assert_ne!(result.source_before, result.source_after);
}

#[test]
fn set_locked_can_unlock_a_locked_node() {
    let doc = parse(TWO_RECT_DOC);

    // Lock then unlock in one tx: set_locked is exempt from the lock guard,
    // so the unlock must be allowed even though the node is locked when it runs.
    let tx = Transaction {
        ops: vec![
            Op::SetLocked {
                node: "a".to_owned(),
                locked: true,
            },
            Op::SetLocked {
                node: "a".to_owned(),
                locked: false,
            },
        ],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert!(
        !result.diagnostics.iter().any(|d| d.code == "node.locked"),
        "set_locked must be exempt from the lock guard, got: {:?}",
        result.diagnostics
    );
}
