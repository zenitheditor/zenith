//! Page-order ops: `ReorderPages` (a full permutation), `MovePage` (one page
//! to a new place), and `SwapPages` (two pages trade places).

use zenith_core::{Diagnostic, Document, Page};

use super::super::record_affected;
use crate::op::Position;

pub(in crate::engine) fn apply_reorder_pages(
    order: &[String],
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    // The current page ids in document order.
    let current: Vec<String> = doc.body.pages.iter().map(|p| p.id.clone()).collect();

    // `order` must be a permutation of `current`: same length, no duplicates,
    // and every requested id must exist exactly once. We verify via sorted
    // comparison (deterministic, no HashMap).
    let mut order_sorted: Vec<&String> = order.iter().collect();
    order_sorted.sort();
    let dup = order_sorted.windows(2).any(|w| w[0] == w[1]);
    let mut current_sorted: Vec<&String> = current.iter().collect();
    current_sorted.sort();

    if dup || order_sorted != current_sorted {
        diagnostics.push(Diagnostic::error(
            "tx.invalid_value",
            format!(
                "reorder_pages: order {:?} is not a permutation of the existing \
                 page ids {:?}. To move one page, use move_page. To swap two, use swap_pages.",
                order, current
            ),
            None,
            None,
        ));
        return;
    }

    // Rebuild the page vec in the requested order. Each id resolves to exactly
    // one page (guaranteed by the permutation check above). We drain the old
    // pages into a lookup-by-index without HashMap: for each target id, find and
    // take its page from a slot-tracking vec.
    let mut slots: Vec<Option<Page>> = doc.body.pages.drain(..).map(Some).collect();
    let mut reordered: Vec<Page> = Vec::with_capacity(order.len());
    for id in order {
        // Find the first remaining slot whose page id matches. The permutation
        // check guarantees a match exists for every id.
        if let Some(slot) = slots
            .iter_mut()
            .find(|s| s.as_ref().map(|p| p.id.as_str()) == Some(id.as_str()))
            && let Some(page) = slot.take()
        {
            reordered.push(page);
        }
    }
    doc.body.pages = reordered;

    // Record every page id as affected (the whole list was restructured).
    for id in order {
        record_affected(id, affected);
    }
}

/// The index of page `id`, or a `tx.unknown_node` Error naming `op`.
fn page_index(
    doc: &Document,
    id: &str,
    op: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<usize> {
    let found = doc.body.pages.iter().position(|p| p.id == id);
    if found.is_none() {
        let ids: Vec<&str> = doc.body.pages.iter().map(|p| p.id.as_str()).collect();
        diagnostics.push(Diagnostic::error(
            "tx.unknown_node",
            format!("{op}: page {id:?} not found. The pages are {ids:?}."),
            None,
            Some(id.to_owned()),
        ));
    }
    found
}

pub(in crate::engine) fn apply_move_page(
    page_id: &str,
    position: &Position,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let Some(from) = page_index(doc, page_id, "move_page", diagnostics) else {
        return;
    };
    // The index in the page list without the moved page.
    let rest = doc.body.pages.len() - 1;
    let to = match position {
        Position::First => 0,
        Position::Last => rest,
        Position::Index { index } => {
            if *index > rest {
                diagnostics.push(Diagnostic::error(
                    "tx.out_of_range",
                    format!("move_page: index {index} is out of range. The last index is {rest}."),
                    None,
                    Some(page_id.to_owned()),
                ));
                return;
            }
            *index
        }
        Position::Before { id } | Position::After { id } => {
            if id == page_id {
                diagnostics.push(Diagnostic::error(
                    "tx.invalid_value",
                    format!("move_page: page {page_id:?} cannot move next to itself"),
                    None,
                    Some(page_id.to_owned()),
                ));
                return;
            }
            let Some(other) = page_index(doc, id, "move_page", diagnostics) else {
                return;
            };
            // Index of the other page once the moved page is out of the list.
            let other = if other > from { other - 1 } else { other };
            match position {
                Position::After { .. } => other + 1,
                Position::First
                | Position::Last
                | Position::Index { .. }
                | Position::Before { .. } => other,
            }
        }
    };
    if to == from {
        diagnostics.push(Diagnostic::advisory(
            "tx.noop",
            format!("move_page: page {page_id:?} is already at index {from}"),
            None,
            Some(page_id.to_owned()),
        ));
        return;
    }
    let page = doc.body.pages.remove(from);
    doc.body.pages.insert(to, page);
    record_affected(page_id, affected);
}

pub(in crate::engine) fn apply_swap_pages(
    a: &str,
    b: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    if a == b {
        diagnostics.push(Diagnostic::error(
            "tx.invalid_value",
            format!("swap_pages: a and b are both {a:?}. Name two different pages."),
            None,
            Some(a.to_owned()),
        ));
        return;
    }
    let Some(ia) = page_index(doc, a, "swap_pages", diagnostics) else {
        return;
    };
    let Some(ib) = page_index(doc, b, "swap_pages", diagnostics) else {
        return;
    };
    doc.body.pages.swap(ia, ib);
    record_affected(a, affected);
    record_affected(b, affected);
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource};

    fn doc(ids: &[&str]) -> Document {
        let pages: String = ids
            .iter()
            .map(|id| format!("    page id=\"{id}\" w=(px)10 h=(px)10 {{ }}\n"))
            .collect();
        let src = format!("zenith version=1 {{\n  document id=\"d\" {{\n{pages}  }}\n}}\n");
        KdlAdapter.parse(src.as_bytes()).expect("fixture parses")
    }

    fn order(doc: &Document) -> Vec<&str> {
        doc.body.pages.iter().map(|p| p.id.as_str()).collect()
    }

    fn mv(ids: &[&str], page: &str, position: Position) -> (Vec<String>, Vec<Diagnostic>) {
        let mut d = doc(ids);
        let mut diags = Vec::new();
        let mut affected = Vec::new();
        apply_move_page(page, &position, &mut d, &mut diags, &mut affected);
        (order(&d).into_iter().map(str::to_owned).collect(), diags)
    }

    #[test]
    fn move_after_a_later_page() {
        let (out, diags) = mv(
            &["a", "b", "c", "d"],
            "a",
            Position::After { id: "c".into() },
        );
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(out, ["b", "c", "a", "d"]);
    }

    #[test]
    fn move_before_an_earlier_page() {
        let (out, _) = mv(
            &["a", "b", "c", "d"],
            "d",
            Position::Before { id: "b".into() },
        );
        assert_eq!(out, ["a", "d", "b", "c"]);
    }

    #[test]
    fn move_to_index_first_and_last() {
        assert_eq!(
            mv(&["a", "b", "c"], "a", Position::Index { index: 2 }).0,
            ["b", "c", "a"]
        );
        assert_eq!(
            mv(&["a", "b", "c"], "c", Position::First).0,
            ["c", "a", "b"]
        );
        assert_eq!(mv(&["a", "b", "c"], "a", Position::Last).0, ["b", "c", "a"]);
    }

    #[test]
    fn move_errors_and_noop() {
        let (_, diags) = mv(&["a", "b"], "a", Position::Index { index: 2 });
        assert_eq!(diags[0].code, "tx.out_of_range");
        let (_, diags) = mv(&["a", "b"], "a", Position::After { id: "a".into() });
        assert_eq!(diags[0].code, "tx.invalid_value");
        let (_, diags) = mv(&["a", "b"], "zz", Position::First);
        assert_eq!(diags[0].code, "tx.unknown_node");
        assert!(
            diags[0].message.contains("[\"a\", \"b\"]"),
            "{}",
            diags[0].message
        );
        let (out, diags) = mv(&["a", "b"], "a", Position::Before { id: "b".into() });
        assert_eq!(diags[0].code, "tx.noop");
        assert_eq!(out, ["a", "b"]);
    }

    #[test]
    fn swap_trades_two_places() {
        let mut d = doc(&["a", "b", "c", "d"]);
        let mut diags = Vec::new();
        let mut affected = Vec::new();
        apply_swap_pages("b", "d", &mut d, &mut diags, &mut affected);
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(order(&d), ["a", "d", "c", "b"]);
        assert_eq!(affected, ["b", "d"]);
    }

    #[test]
    fn swap_rejects_one_page_twice_and_unknown_pages() {
        let mut d = doc(&["a", "b"]);
        let mut diags = Vec::new();
        let mut affected = Vec::new();
        apply_swap_pages("a", "a", &mut d, &mut diags, &mut affected);
        apply_swap_pages("a", "zz", &mut d, &mut diags, &mut affected);
        let codes: Vec<&str> = diags.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, ["tx.invalid_value", "tx.unknown_node"]);
        assert_eq!(order(&d), ["a", "b"]);
    }
}
