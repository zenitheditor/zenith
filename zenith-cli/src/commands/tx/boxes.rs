//! Compiled page boxes for `zenith tx`: the moved/resized box delta, and the
//! `tx.page_box_changed` warning for ops that promise to keep page position.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use zenith_core::{Diagnostic, Document};
use zenith_tx::Op;

use crate::commands::inspect::{NodeBox, resolved_boxes};

use super::collapse::collapse;
use super::tree::Tree;

/// A box change at or under this many px counts as unchanged.
pub(super) const SAME_PX: f64 = 0.01;

/// Reflowed sibling ids named in one warning before the `+N more` tail.
const LISTED_SIBLINGS: usize = 5;

/// One node whose compiled page box differs between the input and the
/// result. A `None` side means the node has no box there (added or removed).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct BoxDelta {
    /// The node id.
    pub id: String,
    /// The page box before the transaction.
    pub before: Option<NodeBox>,
    /// The page box after the transaction.
    pub after: Option<NodeBox>,
    /// How many descendants moved by this node's x/y delta with their size
    /// unchanged. They are folded into this entry and have no entry of
    /// their own. JSON omits the key when it is 0.
    #[serde(skip_serializing_if = "is_zero")]
    pub descendants: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// Page-absolute box per node id over all pages. On a repeated id the
/// first page wins.
pub(super) type PageBoxes = BTreeMap<String, NodeBox>;

/// Compile `doc` and collect each node's page box.
pub(super) fn page_boxes(doc: &Document, project_dir: Option<&Path>) -> PageBoxes {
    let mut out = PageBoxes::new();
    for page in resolved_boxes(doc, project_dir) {
        for (id, info) in page {
            out.entry(id).or_insert(info.rect);
        }
    }
    out
}

/// `true` when the x or y of `a` and `b` differ by more than [`SAME_PX`].
pub(super) fn moved(a: NodeBox, b: NodeBox) -> bool {
    (a.x - b.x).abs() > SAME_PX || (a.y - b.y).abs() > SAME_PX
}

/// `true` when the w or h of `a` and `b` differ by more than [`SAME_PX`].
pub(super) fn resized(a: NodeBox, b: NodeBox) -> bool {
    (a.w - b.w).abs() > SAME_PX || (a.h - b.h).abs() > SAME_PX
}

pub(super) fn differs(a: NodeBox, b: NodeBox) -> bool {
    moved(a, b) || resized(a, b)
}

/// Every id whose box differs between `before` and `after`, sorted by id,
/// with rigid subtrees folded into their root (see [`collapse`]). `tree` is
/// the id tree of the result.
pub(super) fn box_deltas(before: &PageBoxes, after: &PageBoxes, tree: &Tree) -> Vec<BoxDelta> {
    let ids: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    let raw: Vec<BoxDelta> = ids
        .into_iter()
        .filter_map(|id| {
            let old = before.get(id).copied();
            let new = after.get(id).copied();
            let changed = match (old, new) {
                (Some(a), Some(b)) => differs(a, b),
                (None, None) => false,
                (Some(_), None) | (None, Some(_)) => true,
            };
            changed.then(|| BoxDelta {
                id: id.clone(),
                before: old,
                after: new,
                descendants: 0,
            })
        })
        .collect();
    collapse(raw, &BoxSides { before, after }, tree)
}

/// `(x,y wxh)` with each value rounded to 0.01 px.
pub(super) fn fmt_box(b: NodeBox) -> String {
    format!("({},{} {}x{})", px(b.x), px(b.y), px(b.w), px(b.h))
}

fn px(v: f64) -> String {
    // `+ 0.0` turns -0.0 into 0.0.
    let rounded = (v * 100.0).round() / 100.0 + 0.0;
    format!("{rounded}")
}

/// The compiled boxes of both sides of a transaction.
pub(super) struct BoxSides<'a> {
    pub before: &'a PageBoxes,
    pub after: &'a PageBoxes,
}

/// Push one `tx.page_box_changed` Warning per position-preserving subject
/// whose page box changed. A subject below an already reported subject is
/// not reported again. A `tx.flow_placed` for the same subject moves out of
/// `diagnostics` into the warning's cause. Ids missing on either side are
/// skipped. A subject that a later op in the transaction edits is skipped:
/// that op moves it on purpose. `after_tree` is the id tree of the result.
pub(super) fn page_box_warnings(
    ops: &[Op],
    before: &Document,
    after_tree: &Tree,
    boxes: &BoxSides<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let before_tree = Tree::of(before);
    let changed: BTreeSet<&str> = boxes
        .before
        .iter()
        .filter(|(id, old)| {
            boxes
                .after
                .get(id.as_str())
                .is_some_and(|new| differs(**old, *new))
        })
        .map(|(id, _)| id.as_str())
        .collect();
    for (index, op) in ops.iter().enumerate() {
        let Some(subjects) = op.position_preserving_subjects(before) else {
            continue;
        };
        let name = op_name(op);
        let subject_set: BTreeSet<&str> = subjects.iter().map(String::as_str).collect();
        // A later op that edits a subject moves it on purpose.
        let edited_later: BTreeSet<&str> = ops
            .iter()
            .skip(index + 1)
            .flat_map(Op::edited_node_ids)
            .collect();
        let mut covered: BTreeSet<&str> = BTreeSet::new();
        for id in &subjects {
            if edited_later.contains(id.as_str()) {
                continue;
            }
            if before_tree
                .parent
                .get(id)
                .is_some_and(|p| covered.contains(p.as_str()))
            {
                covered.insert(id.as_str());
                continue;
            }
            let (Some(old), Some(new)) = (boxes.before.get(id), boxes.after.get(id)) else {
                continue;
            };
            if !differs(*old, *new) {
                continue;
            }
            covered.insert(id.as_str());
            let reflowed: BTreeSet<&str> = before_tree
                .siblings(id)
                .chain(after_tree.siblings(id))
                .filter(|s| !subject_set.contains(s) && changed.contains(s))
                .collect();
            let mut warning = Diagnostic::warning(
                "tx.page_box_changed",
                warning_message(&name, id, *old, *new, &reflowed),
                None,
                Some(id.clone()),
            );
            if let Some(pos) = diagnostics
                .iter()
                .position(|d| d.code == "tx.flow_placed" && d.subject_id.as_deref() == Some(id))
            {
                let flow = diagnostics.remove(pos);
                warning = warning.with_cause(flow.message);
            }
            diagnostics.push(warning);
        }
    }
}

fn warning_message(
    op: &str,
    id: &str,
    old: NodeBox,
    new: NodeBox,
    reflowed: &BTreeSet<&str>,
) -> String {
    let mut msg = format!(
        "{op}: node {id:?} page box changed from {} to {}",
        fmt_box(old),
        fmt_box(new)
    );
    if !reflowed.is_empty() {
        let listed: Vec<&str> = reflowed.iter().take(LISTED_SIBLINGS).copied().collect();
        msg.push_str(&format!("; reflowed siblings: {}", listed.join(", ")));
        let rest = reflowed.len().saturating_sub(listed.len());
        if rest > 0 {
            msg.push_str(&format!(" +{rest} more"));
        }
    }
    msg.push_str(". Check the dry-run boxes before you apply.");
    msg
}

/// The JSON `op` tag of `op` (`reparent`, `group`, ...).
fn op_name(op: &Op) -> String {
    serde_json::to_value(op)
        .ok()
        .and_then(|v| v.get("op").and_then(|t| t.as_str()).map(str::to_owned))
        .unwrap_or_else(|| "tx".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nb(x: f64, y: f64, w: f64, h: f64) -> NodeBox {
        NodeBox { x, y, w, h }
    }

    #[test]
    fn deltas_skip_sub_tolerance_changes_and_sort_by_id() {
        let before: PageBoxes = [
            ("b".to_owned(), nb(0.0, 0.0, 10.0, 10.0)),
            ("a".to_owned(), nb(5.0, 5.0, 10.0, 10.0)),
            ("gone".to_owned(), nb(0.0, 0.0, 1.0, 1.0)),
        ]
        .into_iter()
        .collect();
        let after: PageBoxes = [
            ("b".to_owned(), nb(0.005, 0.0, 10.0, 10.0)),
            ("a".to_owned(), nb(5.0, 5.0, 12.0, 10.0)),
            ("new".to_owned(), nb(1.0, 1.0, 1.0, 1.0)),
        ]
        .into_iter()
        .collect();
        let ids: Vec<String> = box_deltas(&before, &after, &Tree::default())
            .into_iter()
            .map(|d| d.id)
            .collect();
        assert_eq!(ids, ["a", "gone", "new"]);
    }

    #[test]
    fn fmt_box_rounds_to_hundredths() {
        assert_eq!(
            fmt_box(nb(526.6666, 340.0, 219.0, -0.001)),
            "(526.67,340 219x0)"
        );
    }

    #[test]
    fn warning_message_lists_five_siblings_then_more() {
        let reflowed: BTreeSet<&str> = ["a", "b", "c", "d", "e", "f", "g"].into_iter().collect();
        let msg = warning_message(
            "reparent",
            "n",
            nb(0.0, 0.0, 1.0, 1.0),
            nb(2.0, 0.0, 1.0, 1.0),
            &reflowed,
        );
        assert!(
            msg.contains("reflowed siblings: a, b, c, d, e +2 more"),
            "{msg}"
        );
        assert!(msg.contains("(0,0 1x1) to (2,0 1x1)"), "{msg}");
    }
}
