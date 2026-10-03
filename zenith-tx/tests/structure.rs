mod common;
use common::*;
use zenith_tx::{Op, Permissions, Position, Transaction, TxStatus, run_transaction};

// ── MoveForward: a moves after b ──────────────────────────────────────────

#[test]
fn move_forward_reorders() {
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::MoveForward {
            node: "a".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["a".to_owned()]);

    // In source_after, "b" should appear before "a" (a is now last).
    let pos_a = result
        .source_after
        .find("id=\"a\"")
        .expect("a in source_after");
    let pos_b = result
        .source_after
        .find("id=\"b\"")
        .expect("b in source_after");
    assert!(pos_b < pos_a, "b should appear before a in source_after");

    // source_before has a before b.
    let pb_a = result
        .source_before
        .find("id=\"a\"")
        .expect("a in source_before");
    let pb_b = result
        .source_before
        .find("id=\"b\"")
        .expect("b in source_before");
    assert!(pb_a < pb_b, "a should appear before b in source_before");
}

// ── MoveForward: reorder among group siblings ─────────────────────────────

#[test]
fn tx_move_forward_reorders_nested_child() {
    // Two rects (a then b) nested inside a group. MoveForward on "a"
    // should reorder them so b appears before a in source_after.
    let doc = parse(GROUP_TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::MoveForward {
            node: "a".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["a".to_owned()]);

    // In source_after, "b" should appear before "a".
    let pos_a = result
        .source_after
        .find("id=\"a\"")
        .expect("a in source_after");
    let pos_b = result
        .source_after
        .find("id=\"b\"")
        .expect("b in source_after");
    assert!(pos_b < pos_a, "b should appear before a in source_after");

    // source_before has a before b.
    let pb_a = result
        .source_before
        .find("id=\"a\"")
        .expect("a in source_before");
    let pb_b = result
        .source_before
        .find("id=\"b\"")
        .expect("b in source_before");
    assert!(pb_a < pb_b, "a should appear before b in source_before");
}

// ── MoveBackward tests ────────────────────────────────────────────────────

#[test]
fn move_backward_reorders() {
    // Doc: a (bottom) then b (top). MoveBackward on b → b moves before a.
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::MoveBackward {
            node: "b".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["b".to_owned()]);

    // In source_after, b should appear before a.
    let pos_a = result
        .source_after
        .find("id=\"a\"")
        .expect("a in source_after");
    let pos_b = result
        .source_after
        .find("id=\"b\"")
        .expect("b in source_after");
    assert!(pos_b < pos_a, "b should appear before a in source_after");
}

#[test]
fn move_backward_already_at_back_noop() {
    // Doc: a (bottom) then b. MoveBackward on "a" → already at back → noop advisory.
    let doc = parse(TWO_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::MoveBackward {
            node: "a".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert!(
        result.affected_node_ids.is_empty(),
        "affected must be empty for noop; got: {:?}",
        result.affected_node_ids
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.noop" && d.message.contains("back")),
        "expected tx.noop advisory mentioning \"back\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn move_backward_nested_child() {
    // Group with x (bottom) then y (top). MoveBackward on y → recursion into
    // group, y swaps with x.
    let doc = parse(GROUP_TWO_RECT_BACKWARD_DOC);
    let tx = Transaction {
        ops: vec![Op::MoveBackward {
            node: "y".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["y".to_owned()]);

    // In source_after, y should appear before x.
    let pos_x = result
        .source_after
        .find("id=\"x\"")
        .expect("x in source_after");
    let pos_y = result
        .source_after
        .find("id=\"y\"")
        .expect("y in source_after");
    assert!(pos_y < pos_x, "y should appear before x in source_after");
}

// ── MoveToFront tests ─────────────────────────────────────────────────────

#[test]
fn move_to_front_moves_to_top() {
    // THREE_RECT_DOC: a (0), b (1), c (2). MoveToFront on "a" → order becomes b, c, a.
    let doc = parse(THREE_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::MoveToFront {
            node: "a".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["a".to_owned()]);

    // In source_after: b appears before c, c appears before a.
    let pos_a = result
        .source_after
        .find("id=\"a\"")
        .expect("a in source_after");
    let pos_b = result
        .source_after
        .find("id=\"b\"")
        .expect("b in source_after");
    let pos_c = result
        .source_after
        .find("id=\"c\"")
        .expect("c in source_after");
    assert!(pos_b < pos_c, "b should appear before c in source_after");
    assert!(pos_c < pos_a, "c should appear before a in source_after");
}

#[test]
fn move_to_front_already_front_noop() {
    // THREE_RECT_DOC: c is already the last (topmost). MoveToFront on "c" → noop.
    let doc = parse(THREE_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::MoveToFront {
            node: "c".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert!(
        result.affected_node_ids.is_empty(),
        "affected must be empty for noop; got: {:?}",
        result.affected_node_ids
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.noop" && d.message.contains("front")),
        "expected tx.noop advisory mentioning \"front\"; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── MoveToBack tests ──────────────────────────────────────────────────────

#[test]
fn move_to_back_moves_to_bottom() {
    // THREE_RECT_DOC: a (0), b (1), c (2). MoveToBack on "c" → order becomes c, a, b.
    let doc = parse(THREE_RECT_DOC);
    let tx = Transaction {
        ops: vec![Op::MoveToBack {
            node: "c".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Accepted);
    assert_eq!(result.affected_node_ids, vec!["c".to_owned()]);

    // In source_after: c appears before a, a appears before b.
    let pos_a = result
        .source_after
        .find("id=\"a\"")
        .expect("a in source_after");
    let pos_b = result
        .source_after
        .find("id=\"b\"")
        .expect("b in source_after");
    let pos_c = result
        .source_after
        .find("id=\"c\"")
        .expect("c in source_after");
    assert!(pos_c < pos_a, "c should appear before a in source_after");
    assert!(pos_a < pos_b, "a should appear before b in source_after");
}

// ── Group tests ───────────────────────────────────────────────────────────

/// Group two sibling rects → parent now has one group containing both,
/// inserted at the position of the first (r1's original index = 0).
#[test]
fn group_two_sibling_rects() {
    let doc = parse(TWO_SIBLING_RECTS);
    let tx = Transaction {
        ops: vec![Op::Group {
            node_ids: vec!["r1".to_owned(), "r2".to_owned()],
            group_id: "grp-new".to_owned(),
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
    assert!(
        result.affected_node_ids.contains(&"grp-new".to_owned()),
        "grp-new must be in affected_node_ids"
    );
    // The page should now contain exactly one top-level node: the group.
    assert!(
        result.source_after.contains("id=\"grp-new\""),
        "source_after must contain the new group id"
    );
    // r1 and r2 are inside the group, not at the page level.
    // Both ids should still appear in the source (as group children).
    assert!(
        result.source_after.contains("id=\"r1\""),
        "r1 must appear inside the group"
    );
    assert!(
        result.source_after.contains("id=\"r2\""),
        "r2 must appear inside the group"
    );
    // r1 must appear before r2 in source_after (relative order preserved).
    let pos_r1 = result
        .source_after
        .find("id=\"r1\"")
        .expect("r1 in source_after");
    let pos_r2 = result
        .source_after
        .find("id=\"r2\"")
        .expect("r2 in source_after");
    assert!(pos_r1 < pos_r2, "r1 must precede r2 inside the group");
    // The group must appear before both rects in source_after (group wraps them).
    let pos_grp = result
        .source_after
        .find("id=\"grp-new\"")
        .expect("grp-new in source_after");
    assert!(pos_grp < pos_r1, "group node must open before its children");
}

/// Attempting to group nodes that do not share a parent → tx.invalid_parent.
#[test]
fn group_non_siblings_rejected() {
    let doc = parse(PAGE_WITH_GROUP);
    // r1 is inside grp1, r3 is a top-level sibling of grp1 → different parents.
    let tx = Transaction {
        ops: vec![Op::Group {
            node_ids: vec!["r1".to_owned(), "r3".to_owned()],
            group_id: "grp-bad".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_parent"),
        "expected tx.invalid_parent; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── Ungroup tests ─────────────────────────────────────────────────────────

/// Ungroup a group → its children move up to the parent in order, group gone.
#[test]
fn ungroup_splices_children_in_place() {
    let doc = parse(PAGE_WITH_GROUP);
    let tx = Transaction {
        ops: vec![Op::Ungroup {
            group_id: "grp1".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    // AcceptedWithWarnings is fine; exact status depends on post-validate.
    assert_ne!(
        result.status,
        TxStatus::Rejected,
        "ungroup must not be rejected; diagnostics: {:?}",
        result.diagnostics
    );
    // The group id should no longer appear in source_after.
    assert!(
        !result.source_after.contains("id=\"grp1\""),
        "group grp1 must be gone from source_after;\n{}",
        result.source_after
    );
    // r1 and r2 must still be present (now at page level).
    assert!(
        result.source_after.contains("id=\"r1\""),
        "r1 must appear in source_after"
    );
    assert!(
        result.source_after.contains("id=\"r2\""),
        "r2 must appear in source_after"
    );
    // r1 must appear before r2 (order preserved).
    let pos_r1 = result
        .source_after
        .find("id=\"r1\"")
        .expect("r1 in source_after");
    let pos_r2 = result
        .source_after
        .find("id=\"r2\"")
        .expect("r2 in source_after");
    assert!(pos_r1 < pos_r2, "r1 must precede r2 after ungroup");
    // r3 must still be present.
    assert!(
        result.source_after.contains("id=\"r3\""),
        "r3 must remain in source_after"
    );
}

/// Ungrouping a node that is not a group → tx.unsupported_property.
#[test]
fn ungroup_non_group_rejected() {
    let doc = parse(PAGE_WITH_GROUP);
    let tx = Transaction {
        ops: vec![Op::Ungroup {
            group_id: "r1".to_owned(), // r1 is a rect, not a group
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property"),
        "expected tx.unsupported_property; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

/// Ungrouping a group with a non-zero x/y shifts each child by the group
/// origin, so the child keeps its page position.
#[test]
fn ungroup_with_offset_shifts_children() {
    let doc = parse(PAGE_WITH_OFFSET_GROUP);
    let tx = Transaction {
        ops: vec![Op::Ungroup {
            group_id: "grp1".to_owned(),
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
    assert!(
        !result.source_after.contains("id=\"grp1\""),
        "group must be dissolved"
    );
    assert_eq!(extract_px_attr(&result.source_after, "r1", "x"), Some(50.0));
    assert_eq!(extract_px_attr(&result.source_after, "r1", "y"), Some(20.0));
}

// ── Reparent tests ────────────────────────────────────────────────────────

/// Move a top-level rect into an existing group.
#[test]
fn reparent_rect_into_group() {
    let doc = parse(PAGE_WITH_GROUP);
    // r3 is a top-level rect; move it into grp1.
    let tx = Transaction {
        ops: vec![Op::Reparent {
            node: "r3".to_owned(),
            new_parent: "grp1".to_owned(),
            position: Position::Last,
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
    assert!(
        result.affected_node_ids.contains(&"r3".to_owned()),
        "r3 must be in affected_node_ids"
    );
    // r3 must still be present somewhere in the output.
    assert!(
        result.source_after.contains("id=\"r3\""),
        "r3 must appear in source_after"
    );
    // grp1 must contain r3 (grp1 opens before r3 in the serialised form).
    let pos_grp = result
        .source_after
        .find("id=\"grp1\"")
        .expect("grp1 in source_after");
    let pos_r3 = result
        .source_after
        .find("id=\"r3\"")
        .expect("r3 in source_after");
    assert!(
        pos_grp < pos_r3,
        "r3 must appear after grp1 opens (inside it)"
    );
}

/// Reparent into a non-container (a rect) → tx.invalid_parent.
#[test]
fn reparent_into_non_container_rejected() {
    let doc = parse(PAGE_WITH_GROUP);
    let tx = Transaction {
        ops: vec![Op::Reparent {
            node: "r3".to_owned(),
            new_parent: "r1".to_owned(), // r1 is a rect, not a container
            position: Position::Last,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_parent"),
        "expected tx.invalid_parent; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

/// Reparent a group into its own child group → cycle → tx.invalid_parent.
#[test]
fn reparent_into_own_subtree_rejected() {
    let doc = parse(NESTED_GROUPS);
    // Try to move `outer` into `inner` (inner is a descendant of outer).
    let tx = Transaction {
        ops: vec![Op::Reparent {
            node: "outer".to_owned(),
            new_parent: "inner".to_owned(),
            position: Position::Last,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction must not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_parent"),
        "expected tx.invalid_parent (cycle); got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

// ── AddPage / DeletePage / ReorderPages tests ─────────────────────────────

#[test]
fn add_page_append() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::AddPage {
            id: "pg3".to_owned(),
            w: "(px)800".to_owned(),
            h: "(px)600".to_owned(),
            background: Some("color.bg".to_owned()),
            index: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        page_id_order(&result.source_after),
        vec!["pg1", "pg2", "pg3"],
        "new page must be appended last"
    );
    assert_eq!(result.affected_node_ids, vec!["pg3".to_owned()]);
}

#[test]
fn add_page_at_index() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::AddPage {
            id: "pg.mid".to_owned(),
            w: "(px)800".to_owned(),
            h: "(px)600".to_owned(),
            background: None,
            index: Some(1),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        page_id_order(&result.source_after),
        vec!["pg1", "pg.mid", "pg2"],
        "new page must be inserted at index 1"
    );
}

#[test]
fn add_page_duplicate_id_rejected() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::AddPage {
            id: "pg1".to_owned(),
            w: "(px)800".to_owned(),
            h: "(px)600".to_owned(),
            background: None,
            index: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.duplicate_id"),
        "expected tx.duplicate_id; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn add_page_out_of_range_index_rejected() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::AddPage {
            id: "pg3".to_owned(),
            w: "(px)800".to_owned(),
            h: "(px)600".to_owned(),
            background: None,
            index: Some(5),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.out_of_range"),
        "expected tx.out_of_range; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn add_page_invalid_dimension_rejected() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::AddPage {
            id: "pg3".to_owned(),
            w: "not-a-dim".to_owned(),
            h: "(px)600".to_owned(),
            background: None,
            index: None,
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
        "expected tx.invalid_value; got: {:?}",
        result.diagnostics
    );
}

#[test]
fn delete_page_removes() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::DeletePage {
            page: "pg1".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        page_id_order(&result.source_after),
        vec!["pg2"],
        "pg1 must be removed"
    );
    assert_eq!(result.affected_node_ids, vec!["pg1".to_owned()]);
}

#[test]
fn delete_page_unknown_rejected() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::DeletePage {
            page: "nope".to_owned(),
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
fn reorder_pages_permutation() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::ReorderPages {
            order: vec!["pg2".to_owned(), "pg1".to_owned()],
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        page_id_order(&result.source_after),
        vec!["pg2", "pg1"],
        "pages must be reordered to match `order`"
    );
}

#[test]
fn reorder_pages_missing_id_rejected() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::ReorderPages {
            order: vec!["pg1".to_owned()],
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
        "expected tx.invalid_value; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}

#[test]
fn reorder_pages_extra_id_rejected() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::ReorderPages {
            order: vec!["pg1".to_owned(), "pg2".to_owned(), "pg3".to_owned()],
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
        "expected tx.invalid_value; got: {:?}",
        result.diagnostics
    );
}

#[test]
fn reorder_pages_duplicate_id_rejected() {
    let doc = parse(TWO_PAGE_STRUCT_DOC);
    let tx = Transaction {
        ops: vec![Op::ReorderPages {
            order: vec!["pg1".to_owned(), "pg1".to_owned()],
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
        "expected tx.invalid_value; got: {:?}",
        result.diagnostics
    );
}
