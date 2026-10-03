//! Fold rigid subtrees of the `zenith tx` box delta into one entry.
//!
//! A reparent or a move shifts a whole subtree. Listing every descendant
//! floods the review. A node folds its subtree when it moved and every
//! descendant with a box moved by the same x/y delta with an unchanged size.
//! The node keeps its entry with `descendants: N`. A descendant that moved
//! differently, resized, appeared, or vanished keeps the subtree unfolded,
//! so it keeps its own entry. Only the topmost rigid node folds.

use std::collections::{BTreeMap, BTreeSet};

use crate::commands::inspect::NodeBox;

use super::boxes::{BoxDelta, BoxSides, SAME_PX, moved, resized};
use super::tree::Tree;

/// Fold rigid subtrees in `raw` (sorted by id) and keep the id order.
pub(super) fn collapse(raw: Vec<BoxDelta>, boxes: &BoxSides<'_>, tree: &Tree) -> Vec<BoxDelta> {
    let changed: BTreeSet<&str> = raw.iter().map(|d| d.id.as_str()).collect();
    let rigid: BTreeMap<&str, usize> = raw
        .iter()
        .filter_map(|d| rigid_count(d, boxes, tree, &changed).map(|n| (d.id.as_str(), n)))
        .collect();
    let folded: BTreeMap<String, usize> = rigid
        .iter()
        .filter(|(id, _)| !tree.ancestors(id).any(|a| rigid.contains_key(a)))
        .map(|(id, n)| ((*id).to_owned(), *n))
        .collect();
    let absorbed: BTreeSet<&str> = folded.keys().flat_map(|id| tree.descendants(id)).collect();
    raw.into_iter()
        .filter(|d| !absorbed.contains(d.id.as_str()))
        .map(|mut d| {
            d.descendants = folded.get(&d.id).copied().unwrap_or(0);
            d
        })
        .collect()
}

/// The number of changed descendants `d` folds, or `None` when `d` did not
/// move, its subtree is not rigid, or no descendant changed.
fn rigid_count(
    d: &BoxDelta,
    boxes: &BoxSides<'_>,
    tree: &Tree,
    changed: &BTreeSet<&str>,
) -> Option<usize> {
    let (Some(a), Some(b)) = (d.before, d.after) else {
        return None;
    };
    if !moved(a, b) {
        return None;
    }
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let mut count = 0;
    for id in tree.descendants(&d.id) {
        match (boxes.before.get(id), boxes.after.get(id)) {
            (None, None) => {}
            (Some(old), Some(new)) if same_shift(*old, *new, dx, dy) => {
                if changed.contains(id) {
                    count += 1;
                }
            }
            (Some(_), Some(_)) | (Some(_), None) | (None, Some(_)) => return None,
        }
    }
    (count > 0).then_some(count)
}

/// `true` when `new` is `old` shifted by (`dx`, `dy`) with the same size.
fn same_shift(old: NodeBox, new: NodeBox, dx: f64, dy: f64) -> bool {
    (new.x - old.x - dx).abs() <= SAME_PX
        && (new.y - old.y - dy).abs() <= SAME_PX
        && !resized(old, new)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::tx::boxes::{PageBoxes, box_deltas};
    use zenith_core::{KdlAdapter, KdlSource};

    /// `card` holds `card.title` and `card.body` (`card.body` holds
    /// `card.body.icon`); `other` is a sibling.
    const DOC: &str = r##"zenith version=1 {
  project id="proj" name="T"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc" title="T" {
    page id="pg" w=(px)400 h=(px)400 {
      frame id="card" x=(px)0 y=(px)0 w=(px)100 h=(px)100 {
        rect id="card.title" x=(px)0 y=(px)0 w=(px)10 h=(px)10
        frame id="card.body" x=(px)0 y=(px)20 w=(px)50 h=(px)50 {
          rect id="card.body.icon" x=(px)0 y=(px)0 w=(px)5 h=(px)5
        }
      }
      rect id="other" x=(px)200 y=(px)200 w=(px)10 h=(px)10
    }
  }
}"##;

    fn nb(x: f64, y: f64, w: f64, h: f64) -> NodeBox {
        NodeBox { x, y, w, h }
    }

    fn sides(entries: &[(&str, NodeBox, NodeBox)]) -> (PageBoxes, PageBoxes) {
        let before = entries
            .iter()
            .map(|(id, b, _)| ((*id).to_owned(), *b))
            .collect();
        let after = entries
            .iter()
            .map(|(id, _, a)| ((*id).to_owned(), *a))
            .collect();
        (before, after)
    }

    fn tree() -> Tree {
        Tree::of(&KdlAdapter.parse(DOC.as_bytes()).expect("parses"))
    }

    fn summary(deltas: &[BoxDelta]) -> Vec<(String, usize)> {
        deltas
            .iter()
            .map(|d| (d.id.clone(), d.descendants))
            .collect()
    }

    #[test]
    fn a_rigid_subtree_folds_into_its_root() {
        let (before, after) = sides(&[
            (
                "card",
                nb(0.0, 0.0, 100.0, 100.0),
                nb(50.0, 30.0, 100.0, 100.0),
            ),
            (
                "card.title",
                nb(0.0, 0.0, 10.0, 10.0),
                nb(50.0, 30.0, 10.0, 10.0),
            ),
            (
                "card.body",
                nb(0.0, 20.0, 50.0, 50.0),
                nb(50.0, 50.0, 50.0, 50.0),
            ),
            (
                "card.body.icon",
                nb(0.0, 20.0, 5.0, 5.0),
                nb(50.0, 50.0, 5.0, 5.0),
            ),
            (
                "other",
                nb(200.0, 200.0, 10.0, 10.0),
                nb(200.0, 200.0, 10.0, 10.0),
            ),
        ]);
        let deltas = box_deltas(&before, &after, &tree());
        assert_eq!(summary(&deltas), [("card".to_owned(), 3)]);
        let json = serde_json::to_string(&deltas).expect("serializes");
        assert!(json.contains("\"descendants\":3"), "{json}");
    }

    #[test]
    fn a_mixed_subtree_does_not_fold_at_the_root() {
        // card.body.icon resized: card is not rigid, but card.body still
        // is not either (its descendant resized). card.title moved rigidly
        // with card, yet a leaf has no descendants to fold.
        let (before, after) = sides(&[
            (
                "card",
                nb(0.0, 0.0, 100.0, 100.0),
                nb(50.0, 30.0, 100.0, 100.0),
            ),
            (
                "card.title",
                nb(0.0, 0.0, 10.0, 10.0),
                nb(50.0, 30.0, 10.0, 10.0),
            ),
            (
                "card.body",
                nb(0.0, 20.0, 50.0, 50.0),
                nb(50.0, 50.0, 50.0, 50.0),
            ),
            (
                "card.body.icon",
                nb(0.0, 20.0, 5.0, 5.0),
                nb(50.0, 50.0, 8.0, 5.0),
            ),
        ]);
        let deltas = box_deltas(&before, &after, &tree());
        assert_eq!(
            summary(&deltas),
            [
                ("card".to_owned(), 0),
                ("card.body".to_owned(), 0),
                ("card.body.icon".to_owned(), 0),
                ("card.title".to_owned(), 0),
            ]
        );
        let json = serde_json::to_string(&deltas).expect("serializes");
        assert!(!json.contains("descendants"), "{json}");
    }

    #[test]
    fn a_rigid_inner_subtree_folds_when_its_root_does_not() {
        // card.title moved differently from card, so card does not fold;
        // card.body moved rigidly with its icon, so it folds the icon.
        let (before, after) = sides(&[
            (
                "card",
                nb(0.0, 0.0, 100.0, 100.0),
                nb(50.0, 30.0, 100.0, 100.0),
            ),
            (
                "card.title",
                nb(0.0, 0.0, 10.0, 10.0),
                nb(60.0, 30.0, 10.0, 10.0),
            ),
            (
                "card.body",
                nb(0.0, 20.0, 50.0, 50.0),
                nb(50.0, 50.0, 50.0, 50.0),
            ),
            (
                "card.body.icon",
                nb(0.0, 20.0, 5.0, 5.0),
                nb(50.0, 50.0, 5.0, 5.0),
            ),
        ]);
        let deltas = box_deltas(&before, &after, &tree());
        assert_eq!(
            summary(&deltas),
            [
                ("card".to_owned(), 0),
                ("card.body".to_owned(), 1),
                ("card.title".to_owned(), 0),
            ]
        );
    }

    #[test]
    fn an_unchanged_descendant_blocks_the_fold() {
        let (before, after) = sides(&[
            (
                "card",
                nb(0.0, 0.0, 100.0, 100.0),
                nb(50.0, 30.0, 100.0, 100.0),
            ),
            (
                "card.title",
                nb(0.0, 0.0, 10.0, 10.0),
                nb(0.0, 0.0, 10.0, 10.0),
            ),
            (
                "card.body",
                nb(0.0, 20.0, 50.0, 50.0),
                nb(50.0, 50.0, 50.0, 50.0),
            ),
        ]);
        let deltas = box_deltas(&before, &after, &tree());
        assert_eq!(
            summary(&deltas),
            [("card".to_owned(), 0), ("card.body".to_owned(), 0)]
        );
    }
}
