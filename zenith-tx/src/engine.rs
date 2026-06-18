//! Transaction engine: [`run_transaction`] and all per-op application logic.
//!
//! This module is pure: it performs no file I/O and does not mutate the input
//! document (it works on a clone). Dry-run vs. apply is the caller's concern.

use zenith_core::{Diagnostic, Document, KdlAdapter, KdlSource, Node, Severity, validate};

use crate::op::{Op, Transaction};
use crate::result::{TxError, TxResult, TxStatus};

// ── Valid align values ────────────────────────────────────────────────────────

const VALID_ALIGNS: &[&str] = &["start", "center", "end", "justify"];

// ── Public entry point ────────────────────────────────────────────────────────

/// Apply `tx` to `doc` and return a structured [`TxResult`].
///
/// The function is **pure**: `doc` is never mutated (a clone is used for the
/// candidate), and no I/O is performed. Both dry-run and apply callers receive
/// the same result shape; the caller decides whether to persist `source_after`.
pub fn run_transaction(doc: &Document, tx: &Transaction) -> Result<TxResult, TxError> {
    let adapter = KdlAdapter;

    // 1. Format the original document → source_before.
    let source_before_bytes = adapter.format(doc).map_err(|e| TxError {
        message: format!("failed to format source document: {e}"),
    })?;
    let source_before = String::from_utf8(source_before_bytes).map_err(|e| TxError {
        message: format!("source_before is not valid UTF-8: {e}"),
    })?;

    // 2. Clone the document into a mutable candidate.
    let mut candidate = doc.clone();

    // 3. Apply each op in order, collecting diagnostics and affected ids.
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut affected: Vec<String> = Vec::new(); // insertion-order, de-duplicated

    for op in &tx.ops {
        apply_op(op, &mut candidate, &mut diagnostics, &mut affected);
    }

    // 4. Post-apply validation.
    let report = validate(&candidate);
    diagnostics.extend(report.diagnostics);

    // 5. Determine status and source_after.
    let has_errors = diagnostics.iter().any(|d| d.severity == Severity::Error);
    let has_warnings = diagnostics.iter().any(|d| d.severity == Severity::Warning);

    let (status, source_after) = if has_errors {
        // Rejected — discard candidate, source_after == source_before.
        (TxStatus::Rejected, source_before.clone())
    } else {
        let after_bytes = adapter.format(&candidate).map_err(|e| TxError {
            message: format!("failed to format candidate document: {e}"),
        })?;
        let after = String::from_utf8(after_bytes).map_err(|e| TxError {
            message: format!("source_after is not valid UTF-8: {e}"),
        })?;
        let status = if has_warnings {
            TxStatus::AcceptedWithWarnings
        } else {
            TxStatus::Accepted
        };
        (status, after)
    };

    Ok(TxResult {
        status,
        diagnostics,
        source_before,
        source_after,
        affected_node_ids: affected,
    })
}

// ── Per-op dispatch ───────────────────────────────────────────────────────────

fn apply_op(
    op: &Op,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    match op {
        Op::SetTextAlign {
            node: node_id,
            align,
        } => {
            apply_set_text_align(node_id, align, doc, diagnostics, affected);
        }
        Op::MoveForward { node: node_id } => {
            apply_move_forward(node_id, doc, diagnostics, affected);
        }
    }
}

// ── SetTextAlign ──────────────────────────────────────────────────────────────

fn apply_set_text_align(
    node_id: &str,
    align: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    // Validate align value before touching the tree.
    if !VALID_ALIGNS.contains(&align) {
        diagnostics.push(Diagnostic::error(
            "tx.invalid_value",
            format!(
                "invalid align value {:?}; must be one of: {}",
                align,
                VALID_ALIGNS.join(", ")
            ),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    }

    // Walk the tree looking for `node_id`.
    match find_node_mut(doc, node_id) {
        FindResult::NotFound => {
            diagnostics.push(Diagnostic::error(
                "tx.unknown_node",
                format!("node {:?} not found in document", node_id),
                None,
                Some(node_id.to_owned()),
            ));
        }
        FindResult::WrongType { kind } => {
            diagnostics.push(Diagnostic::error(
                "tx.wrong_node_type",
                format!(
                    "set_text_align requires a text node but {:?} is a {}",
                    node_id, kind
                ),
                None,
                Some(node_id.to_owned()),
            ));
        }
        FindResult::TextNode(text_node) => {
            text_node.align = Some(align.to_owned());
            record_affected(node_id, affected);
        }
    }
}

// ── MoveForward ───────────────────────────────────────────────────────────────

fn apply_move_forward(
    node_id: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    match find_sibling_index(doc, node_id) {
        SiblingResult::NotFound => {
            diagnostics.push(Diagnostic::error(
                "tx.unknown_node",
                format!("node {:?} not found in document", node_id),
                None,
                Some(node_id.to_owned()),
            ));
        }
        SiblingResult::Found {
            page_index,
            sibling_index,
            sibling_count,
        } => {
            if sibling_index + 1 >= sibling_count {
                // Already last (front) — no-op; emit an advisory.
                diagnostics.push(Diagnostic::advisory(
                    "tx.noop",
                    format!("node {:?} is already at the front of its parent", node_id),
                    None,
                    Some(node_id.to_owned()),
                ));
            } else {
                // Swap with the next sibling.
                // `page_index` came from `find_sibling_index` which scanned
                // the same `doc.body.pages`; no intervening mutation, so
                // `.get_mut()` will always be `Some`. We use it instead of
                // the indexing operator so the engine can never panic.
                if let Some(page) = doc.body.pages.get_mut(page_index) {
                    page.children.swap(sibling_index, sibling_index + 1);
                    record_affected(node_id, affected);
                }
            }
        }
    }
}

// ── Tree walk helpers ─────────────────────────────────────────────────────────

/// Result of a node lookup for mutation.
enum FindResult<'a> {
    NotFound,
    WrongType { kind: &'static str },
    TextNode(&'a mut zenith_core::TextNode),
}

/// Walk the document tree and return a mutable reference to a `TextNode` with
/// the given id, or indicate not-found / wrong-type.
///
/// Two-phase approach: shared scan first (to find the page index), then a
/// single targeted mutable borrow. This pattern avoids the borrow-checker
/// conflict that would arise if we tried to return a mutable reference from
/// within an `&mut`-iterating for loop.
fn find_node_mut<'doc>(doc: &'doc mut Document, id: &str) -> FindResult<'doc> {
    // Phase 1: find which page (shared borrow only).
    let page_index = doc.body.pages.iter().enumerate().find_map(|(pi, page)| {
        let found = page.children.iter().any(|node| match node {
            Node::Text(t) => t.id == id,
            Node::Rect(r) => r.id == id,
            Node::Ellipse(e) => e.id == id,
            Node::Line(l) => l.id == id,
            Node::Unknown(_) => false,
        });
        if found { Some(pi) } else { None }
    });

    // Phase 2: act on the found page with an exclusive borrow.
    // `pi` came from iterating the same `doc.body.pages`; no intervening
    // mutation, so `.get_mut()` will always be `Some`. We use it instead of
    // the indexing operator so the engine can never panic.
    match page_index {
        None => FindResult::NotFound,
        Some(pi) => match doc.body.pages.get_mut(pi) {
            None => FindResult::NotFound,
            Some(page) => {
                find_in_children_mut(&mut page.children, id).unwrap_or(FindResult::NotFound)
            }
        },
    }
}

fn find_in_children_mut<'a>(children: &'a mut [Node], id: &str) -> Option<FindResult<'a>> {
    // Two-phase: first find the index (shared borrow), then mutate (exclusive
    // borrow). This avoids a simultaneous shared + mutable borrow of `children`.

    // Phase 1: find the index and record what kind of node it is.
    enum Hit {
        Text(usize),
        WrongType(&'static str),
    }

    let hit = children
        .iter()
        .enumerate()
        .find_map(|(i, node)| match node {
            Node::Text(t) if t.id == id => Some(Hit::Text(i)),
            Node::Rect(r) if r.id == id => Some(Hit::WrongType("rect")),
            Node::Ellipse(e) if e.id == id => Some(Hit::WrongType("ellipse")),
            Node::Line(l) if l.id == id => Some(Hit::WrongType("line")),
            // All other variants without a matching id, and Unknown: skip.
            _ => None,
        });

    // Phase 2: act on the hit (if any).
    match hit {
        None => None,
        Some(Hit::WrongType(kind)) => Some(FindResult::WrongType { kind }),
        Some(Hit::Text(i)) => {
            // SAFETY: `i` came from the same `children` slice above; it is
            // within bounds. We replace the shared borrow with an exclusive one.
            match &mut children[i] {
                Node::Text(t) => Some(FindResult::TextNode(t)),
                // Unreachable: we just confirmed it's Text in phase 1.
                _ => None,
            }
        }
    }
}

/// Result of a sibling-order lookup.
enum SiblingResult {
    NotFound,
    Found {
        page_index: usize,
        sibling_index: usize,
        sibling_count: usize,
    },
}

/// Find which page and sibling index a node occupies (for reordering).
fn find_sibling_index(doc: &Document, id: &str) -> SiblingResult {
    for (pi, page) in doc.body.pages.iter().enumerate() {
        for (si, node) in page.children.iter().enumerate() {
            let node_id = node_id_of(node);
            if node_id == Some(id) {
                return SiblingResult::Found {
                    page_index: pi,
                    sibling_index: si,
                    sibling_count: page.children.len(),
                };
            }
        }
    }
    SiblingResult::NotFound
}

/// Extract the stable id string from any [`Node`] variant, if it has one.
fn node_id_of(node: &Node) -> Option<&str> {
    match node {
        Node::Rect(r) => Some(&r.id),
        Node::Ellipse(e) => Some(&e.id),
        Node::Line(l) => Some(&l.id),
        Node::Text(t) => Some(&t.id),
        Node::Unknown(_) => None,
    }
}

// ── Utility ───────────────────────────────────────────────────────────────────

/// Append `id` to `affected` only if it is not already present.
/// Uses a linear scan to maintain deterministic first-seen insertion order
/// without HashMap (which has non-deterministic iteration).
fn record_affected(id: &str, affected: &mut Vec<String>) {
    if !affected.iter().any(|s| s == id) {
        affected.push(id.to_owned());
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::Transaction;
    use zenith_core::{KdlAdapter, KdlSource};

    /// Minimal valid document with a `text` node (align `start`) and a `rect`.
    fn parse(src: &str) -> Document {
        KdlAdapter
            .parse(src.as_bytes())
            .expect("test doc must parse")
    }

    // ── Test documents ────────────────────────────────────────────────────────

    const TEXT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      text id="label" x=(px)10 y=(px)10 w=(px)200 h=(px)40 align="start" {
        span "Hello"
      }
    }
  }
}"##;

    const TWO_RECT_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="a" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      rect id="b" x=(px)0 y=(px)0 w=(px)100 h=(px)100
    }
  }
}"##;

    const MIXED_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      rect id="box1" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      text id="lbl" x=(px)10 y=(px)10 w=(px)200 h=(px)40 {
        span "Hi"
      }
    }
  }
}"##;

    const ELLIPSE_DOC: &str = r##"zenith version=1 {
  project id="proj" name="Test"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc1" title="T" {
    page id="pg1" w=(px)400 h=(px)300 {
      ellipse id="dot" x=(px)0 y=(px)0 w=(px)100 h=(px)100
      text id="lbl" x=(px)10 y=(px)10 w=(px)200 h=(px)40 {
        span "Hi"
      }
    }
  }
}"##;

    // ── 1. SetTextAlign: accepted, affected ids, source diff ──────────────────

    #[test]
    fn set_text_align_accepted() {
        let doc = parse(TEXT_DOC);
        let tx = Transaction {
            ops: vec![Op::SetTextAlign {
                node: "label".to_owned(),
                align: "center".to_owned(),
            }],
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
            }
        );
    }

    // ── 3. MoveForward: a moves after b ──────────────────────────────────────

    #[test]
    fn move_forward_reorders() {
        let doc = parse(TWO_RECT_DOC);
        let tx = Transaction {
            ops: vec![Op::MoveForward {
                node: "a".to_owned(),
            }],
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

    // ── 4. Unknown node id → Rejected ────────────────────────────────────────

    #[test]
    fn unknown_node_rejected() {
        let doc = parse(TEXT_DOC);
        let tx = Transaction {
            ops: vec![Op::SetTextAlign {
                node: "does_not_exist".to_owned(),
                align: "center".to_owned(),
            }],
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

    // ── 5. SetTextAlign on a rect → wrong_node_type, Rejected ────────────────

    #[test]
    fn set_text_align_wrong_node_type() {
        let doc = parse(MIXED_DOC);
        let tx = Transaction {
            ops: vec![Op::SetTextAlign {
                node: "box1".to_owned(),
                align: "center".to_owned(),
            }],
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

    // ── 6. Invalid align value → tx.invalid_value, Rejected ──────────────────

    #[test]
    fn invalid_align_value_rejected() {
        let doc = parse(TEXT_DOC);
        let tx = Transaction {
            ops: vec![Op::SetTextAlign {
                node: "label".to_owned(),
                align: "middle".to_owned(),
            }],
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
}
