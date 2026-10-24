//! The page order before and after a tx that adds, removes, or moves pages.

use serde::Serialize;
use zenith_core::{Document, KdlAdapter, KdlSource};
use zenith_tx::{Op, TxResult};

/// Page ids in document order, before and after the edit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PageOrder {
    /// The page order of the input document.
    pub before: Vec<String>,
    /// The page order the edit writes. It equals `before` for a rejected edit.
    pub after: Vec<String>,
}

/// The page ids of `doc` in order.
fn ids(doc: &Document) -> Vec<String> {
    doc.body.pages.iter().map(|p| p.id.clone()).collect()
}

/// The page order before and after `result`, or `None` when no op in `ops`
/// changes the page order. `None` also when the edited source does not
/// parse, which the engine never produces.
pub(super) fn page_order(ops: &[Op], before: &Document, result: &TxResult) -> Option<PageOrder> {
    if !ops.iter().any(Op::changes_page_order) {
        return None;
    }
    let before_ids = ids(before);
    let after = if result.source_after == result.source_before {
        before_ids.clone()
    } else {
        ids(&KdlAdapter.parse(result.source_after.as_bytes()).ok()?)
    };
    Some(PageOrder {
        before: before_ids,
        after,
    })
}

/// `pages: a, b, c -> a, c, b`, or `pages: a, b (unchanged)`.
pub(super) fn page_order_line(order: &PageOrder) -> String {
    let before = order.before.join(", ");
    if order.before == order.after {
        format!("pages: {before} (unchanged)")
    } else {
        format!("pages: {before} -> {}", order.after.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(before: &[&str], after: &[&str]) -> PageOrder {
        PageOrder {
            before: before.iter().map(|s| (*s).to_owned()).collect(),
            after: after.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    #[test]
    fn line_shows_before_and_after() {
        assert_eq!(
            page_order_line(&order(&["a", "b", "c"], &["a", "c", "b"])),
            "pages: a, b, c -> a, c, b"
        );
        assert_eq!(
            page_order_line(&order(&["a", "b"], &["a", "b"])),
            "pages: a, b (unchanged)"
        );
    }
}
