//! The ids a `duplicate_page` or `duplicate_node` op creates.

use std::collections::BTreeSet;

use zenith_core::{Document, KdlAdapter, KdlSource};
use zenith_tx::{Op, TxResult};

use super::tree::ordered_ids;

/// The most new ids the human line names.
const SHOWN: usize = 5;

/// The ids in the edited document that the input document lacks, in document
/// order. `None` when no op in `ops` duplicates nodes. `None` also when the
/// edited source does not parse, which the engine never produces. A rejected
/// edit creates nothing.
pub(super) fn created_ids(ops: &[Op], before: &Document, result: &TxResult) -> Option<Vec<String>> {
    if !ops
        .iter()
        .any(|op| matches!(op, Op::DuplicatePage { .. } | Op::DuplicateNode { .. }))
    {
        return None;
    }
    if result.source_after == result.source_before {
        return Some(Vec::new());
    }
    let after = KdlAdapter.parse(result.source_after.as_bytes()).ok()?;
    let known: BTreeSet<String> = ordered_ids(before).into_iter().collect();
    Some(
        ordered_ids(&after)
            .into_iter()
            .filter(|id| !known.contains(id))
            .collect(),
    )
}

/// `created: a, b, c ... (+9 more)`, or `created: (none)`.
pub(super) fn created_line(ids: &[String]) -> String {
    if ids.is_empty() {
        return "created: (none)".to_owned();
    }
    let shown = ids
        .iter()
        .take(SHOWN)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    match ids.len().saturating_sub(SHOWN) {
        0 => format!("created: {shown}"),
        more => format!("created: {shown} ... (+{more} more)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("n{i}")).collect()
    }

    #[test]
    fn line_names_up_to_five_ids() {
        assert_eq!(created_line(&ids(3)), "created: n0, n1, n2");
        assert_eq!(created_line(&ids(5)), "created: n0, n1, n2, n3, n4");
    }

    #[test]
    fn line_counts_the_rest() {
        assert_eq!(
            created_line(&ids(14)),
            "created: n0, n1, n2, n3, n4 ... (+9 more)"
        );
    }

    #[test]
    fn line_for_no_ids() {
        assert_eq!(created_line(&[]), "created: (none)");
    }
}
