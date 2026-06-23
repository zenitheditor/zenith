//! Integration tests for the `detach_pattern` transaction op.
//!
//! `detach_pattern` materializes a `pattern` node into an editable `group` of
//! native shapes whose instance positions match the live pattern exactly
//! (both go through `zenith_core::pattern_positions`).

mod common;
use common::*;
use zenith_core::Node;
use zenith_tx::{Op, Permissions, Transaction, TxStatus, run_transaction};

// ── Fixtures ──────────────────────────────────────────────────────────────────

/// A grid pattern over bounds (0,0,100,100) with spacing 50, whose motif is a
/// 10×10 ellipse at the origin. The grid yields 4 instances:
/// (0,0), (50,0), (0,50), (50,50) in row-major order.
const GRID_PATTERN_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" {
    token id="color.dot" type="color" value="#334155"
  }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      pattern id="dots" kind="grid" x=(px)0 y=(px)0 w=(px)100 h=(px)100 spacing=(px)50 fill=(token)"color.dot" {
        ellipse id="dot" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.dot"
      }
    }
  }
}"##;

/// A plain rect (no pattern) for the not-a-pattern rejection test.
const PLAIN_RECT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="box" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}"##;

// ── detach_pattern: grid → editable group ─────────────────────────────────────

/// Detaching a grid pattern replaces it with a group carrying the same id and
/// the pattern's bounds, containing exactly four ellipse clones with ids
/// `dots.0..dots.3` at the four grid instance positions.
#[test]
fn detach_pattern_grid_accepted() {
    let doc = parse(GRID_PATTERN_DOC);

    let tx = Transaction {
        ops: vec![Op::DetachPattern {
            node: "dots".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "expected Accepted; diagnostics: {:?}",
        result.diagnostics
    );
    assert_eq!(
        result.affected_node_ids,
        vec!["dots".to_owned()],
        "affected must contain the detached pattern id"
    );

    // Re-parse the resulting source and inspect the materialized group.
    let after_doc = parse(&result.source_after);
    let page = after_doc
        .body
        .pages
        .first()
        .expect("page pg1 must exist after detach");

    // The pattern node is gone; in its place is a group with the same id.
    let group = page
        .children
        .iter()
        .find_map(|n| match n {
            Node::Group(g) if g.id == "dots" => Some(g),
            _ => None,
        })
        .expect("a group with id 'dots' must replace the pattern");

    assert!(
        page.children.iter().all(|n| !matches!(n, Node::Pattern(_))),
        "no pattern node may remain after detach"
    );

    // Group bounds equal the pattern's bounds.
    assert_eq!(
        group.x.as_ref().map(|d| d.value),
        Some(0.0),
        "group x must equal pattern bounds x"
    );
    assert_eq!(
        group.y.as_ref().map(|d| d.value),
        Some(0.0),
        "group y must equal pattern bounds y"
    );
    assert_eq!(
        group.w.as_ref().map(|d| d.value),
        Some(100.0),
        "group w must equal pattern bounds w"
    );
    assert_eq!(
        group.h.as_ref().map(|d| d.value),
        Some(100.0),
        "group h must equal pattern bounds h"
    );

    // Exactly four child clones, ids dots.0..dots.3 in render order.
    assert_eq!(
        group.children.len(),
        4,
        "grid 100/50 must yield 4 instances"
    );

    // Read each child's (id, x, y).
    let positions: Vec<(String, f64, f64)> = group
        .children
        .iter()
        .map(|child| match child {
            Node::Ellipse(e) => (
                e.id.clone(),
                e.x.as_ref().map(|d| d.value).unwrap_or(f64::NAN),
                e.y.as_ref().map(|d| d.value).unwrap_or(f64::NAN),
            ),
            other => panic!("expected ellipse clone, got {other:?}"),
        })
        .collect();

    let expected = [
        ("dots.0", 0.0, 0.0),
        ("dots.1", 50.0, 0.0),
        ("dots.2", 0.0, 50.0),
        ("dots.3", 50.0, 50.0),
    ];
    for (i, (eid, ex, ey)) in expected.iter().enumerate() {
        let (gid, gx, gy) = &positions[i];
        assert_eq!(gid, eid, "child {i} id mismatch");
        assert_eq!(gx, ex, "child {i} x mismatch");
        assert_eq!(gy, ey, "child {i} y mismatch");
    }
}

/// The detached group re-validates cleanly: the four child ids are unique, so
/// no `id.duplicate` diagnostic is produced (status is Accepted, not Rejected).
#[test]
fn detach_pattern_grid_revalidates_clean() {
    let doc = parse(GRID_PATTERN_DOC);

    let tx = Transaction {
        ops: vec![Op::DetachPattern {
            node: "dots".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_ne!(
        result.status,
        TxStatus::Rejected,
        "detach must not be rejected; diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result.diagnostics.iter().all(|d| d.code != "id.duplicate"),
        "child ids must be unique (no id.duplicate); got: {:?}",
        result.diagnostics
    );
}

// ── detach_pattern: unknown node → Rejected ───────────────────────────────────

/// Detaching an id that does not exist → Rejected (tx.unknown_node).
#[test]
fn detach_pattern_unknown_node_rejected() {
    let doc = parse(GRID_PATTERN_DOC);

    let tx = Transaction {
        ops: vec![Op::DetachPattern {
            node: "does_not_exist".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Rejected,
        "expected Rejected; diagnostics: {:?}",
        result.diagnostics
    );
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

// ── detach_pattern: non-pattern node → Rejected ───────────────────────────────

/// Detaching a node that is not a pattern (a rect) → Rejected (tx.not_a_pattern).
#[test]
fn detach_pattern_not_a_pattern_rejected() {
    let doc = parse(PLAIN_RECT_DOC);

    let tx = Transaction {
        ops: vec![Op::DetachPattern {
            node: "box".to_owned(),
        }],
        permissions: Permissions::default(),
    };
    let result = run_transaction(&doc, &tx).expect("run_transaction should not error");

    assert_eq!(
        result.status,
        TxStatus::Rejected,
        "expected Rejected; diagnostics: {:?}",
        result.diagnostics
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.not_a_pattern"),
        "expected tx.not_a_pattern; got: {:?}",
        result.diagnostics
    );
    assert_eq!(result.source_after, result.source_before);
}
