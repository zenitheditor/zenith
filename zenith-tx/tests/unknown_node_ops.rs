//! Integration tests for UNIT 3 of the library-framework foundation: unknown
//! (library) nodes and the KNOWN nodes nested inside them are addressable
//! through the transaction layer.
//!
//! Principle exercised here:
//! - DESCEND/FIND/EDIT/REMOVE/REORDER/DUPLICATE into `unknown.children`: yes.
//! - The unknown node itself is targetable BY ID (find/remove).
//! - REPARENT INTO an unknown node: NO — it is not a valid container target.
//!
//! Note on status: a document containing an unknown node always emits a
//! `node.unknown_kind` *warning* during post-validation, so a successful op on
//! such a document yields `AcceptedWithWarnings` (never plain `Accepted`). The
//! assertions below check that no `Severity::Error` diagnostic is present rather
//! than demanding the bare `Accepted` status.

mod common;
use common::*;
use zenith_tx::{Op, Permissions, Transaction, TxStatus, run_transaction};

/// True when the transaction was not rejected (Accepted or AcceptedWithWarnings).
fn accepted(status: TxStatus) -> bool {
    matches!(status, TxStatus::Accepted | TxStatus::AcceptedWithWarnings)
}

// ── Edit a known node nested inside an unknown node ───────────────────────────

/// `set_geometry` targeting a `rect id="inner"` nested inside an unknown
/// `mystery` node must succeed and mutate the rect — proving the finders
/// descend into `unknown.children`.
#[test]
fn set_geometry_on_node_inside_unknown() {
    let doc = parse(UNKNOWN_WITH_INNER_DOC);
    let tx = Transaction {
        ops: vec![Op::SetGeometry {
            node: "inner".to_owned(),
            x: Some(25.0),
            y: None,
            w: Some(120.0),
            h: None,
            rotate: None,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert!(
        accepted(result.status),
        "expected non-rejected; diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["inner".to_owned()]);
    assert!(
        result.source_after.contains("x=(px)25"),
        "inner rect must have new x; got:\n{}",
        result.source_after
    );
    assert!(
        result.source_after.contains("w=(px)120"),
        "inner rect must have new w; got:\n{}",
        result.source_after
    );
    // The unknown parent survives unchanged.
    assert!(
        result.source_after.contains("id=\"lib1\""),
        "unknown parent must remain in source_after"
    );
}

// ── Remove a known node nested inside an unknown node ─────────────────────────

/// `remove_node` targeting `inner` removes it from the unknown node's children
/// while the unknown parent itself remains.
#[test]
fn remove_node_inside_unknown() {
    let doc = parse(UNKNOWN_WITH_INNER_DOC);
    let tx = Transaction {
        ops: vec![Op::RemoveNode {
            node: "inner".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert!(
        accepted(result.status),
        "expected non-rejected; diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["inner".to_owned()]);
    assert!(
        !result.source_after.contains("id=\"inner\""),
        "inner must be gone; got:\n{}",
        result.source_after
    );
    assert!(
        result.source_after.contains("id=\"lib1\""),
        "unknown parent must remain after removing its child"
    );
}

// ── Remove the unknown node itself by id ──────────────────────────────────────

/// `remove_node` targeting the UNKNOWN node's own id removes the whole unknown
/// subtree — proving id-targeting via `Node::id()` returning the unknown id.
#[test]
fn remove_unknown_node_by_id() {
    let doc = parse(UNKNOWN_WITH_INNER_DOC);
    let tx = Transaction {
        ops: vec![Op::RemoveNode {
            node: "lib1".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert!(
        accepted(result.status),
        "expected non-rejected; diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(result.affected_node_ids, vec!["lib1".to_owned()]);
    // The whole unknown subtree is gone: both the unknown node and its child.
    assert!(
        !result.source_after.contains("id=\"lib1\""),
        "unknown node must be gone; got:\n{}",
        result.source_after
    );
    assert!(
        !result.source_after.contains("id=\"inner\""),
        "the unknown node's child must be gone with it; got:\n{}",
        result.source_after
    );
}

// ── Reparent INTO an unknown node is rejected ─────────────────────────────────

/// An unknown node is NOT a valid container target. Reparenting a sibling rect
/// into the unknown node must be rejected with `tx.invalid_parent`, leaving the
/// document unchanged.
#[test]
fn reparent_into_unknown_rejected() {
    let doc = parse(UNKNOWN_PARENT_DOC);
    let tx = Transaction {
        ops: vec![Op::Reparent {
            node: "outer".to_owned(),
            new_parent: "lib1".to_owned(), // lib1 is an unknown node, not a container
            position: zenith_tx::Position::Last,
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_parent"),
        "expected tx.invalid_parent; got: {:?}",
        result.diagnostics
    );
    assert_eq!(
        result.source_after, result.source_before,
        "a rejected transaction must not mutate the document"
    );
}

// ── Group INTO an unknown node is rejected ────────────────────────────────────

/// Grouping nodes whose only common would-be parent is unknown is also rejected:
/// `inner` lives inside the unknown node, `outer` is a page sibling, so they do
/// not share a valid container parent.
#[test]
fn group_across_unknown_boundary_rejected() {
    let doc = parse(UNKNOWN_PARENT_DOC);
    let tx = Transaction {
        ops: vec![Op::Group {
            node_ids: vec!["inner".to_owned(), "outer".to_owned()],
            group_id: "grp-bad".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(result.status, TxStatus::Rejected);
    assert_eq!(
        result.source_after, result.source_before,
        "a rejected group must not mutate the document"
    );
}

// ── Reorder a known node inside an unknown node ───────────────────────────────

/// `move_forward` on a node nested inside an unknown node descends correctly.
/// With a single child the reorder is a no-op (advisory), but it must NOT be a
/// hard error — proving `reorder_in` reaches into `unknown.children`.
#[test]
fn reorder_inside_unknown_not_errored() {
    let doc = parse(UNKNOWN_WITH_INNER_DOC);
    let tx = Transaction {
        ops: vec![Op::MoveForward {
            node: "inner".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    // A single-child reorder yields a no-op advisory, not a rejection.
    assert!(
        accepted(result.status),
        "reorder inside unknown must not be rejected; diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result.source_after.contains("id=\"inner\""),
        "inner must still be present after a no-op reorder"
    );
}

// ── Duplicate a page whose subtree contains an unknown node ───────────────────

/// Duplicating a page that contains an unknown node with a known child must
/// re-suffix BOTH the unknown node's own id and its descendant ids, so the
/// cloned subtree has unique ids (no `id.duplicate`).
#[test]
fn duplicate_page_suffixes_unknown_subtree_ids() {
    let doc = parse(UNKNOWN_WITH_INNER_DOC);
    let tx = Transaction {
        ops: vec![Op::DuplicatePage {
            page: "pg1".to_owned(),
            new_id: "pg2".to_owned(),
            id_suffix: ".v2".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert!(
        accepted(result.status),
        "duplicate_page over an unknown subtree must succeed; diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        !result.diagnostics.iter().any(|d| d.code == "id.duplicate"),
        "cloned unknown subtree ids must be unique; got: {:?}",
        result.diagnostics
    );
    // The clone carries suffixed ids for both the unknown node and its child.
    assert!(
        result.source_after.contains("id=\"lib1.v2\""),
        "cloned unknown node must carry the suffixed id; got:\n{}",
        result.source_after
    );
    assert!(
        result.source_after.contains("id=\"inner.v2\""),
        "cloned child of unknown node must carry the suffixed id; got:\n{}",
        result.source_after
    );
    // The original ids remain exactly once each (only the source page carries them).
    assert_eq!(
        result.source_after.matches("id=\"lib1\"").count(),
        1,
        "source unknown node id must remain unique; got:\n{}",
        result.source_after
    );
    assert_eq!(
        result.source_after.matches("id=\"inner\"").count(),
        1,
        "source child id must remain unique; got:\n{}",
        result.source_after
    );
}
