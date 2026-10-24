//! The "did you mean" hint on `tx.unknown_node`.
//!
//! Many ops emit `tx.unknown_node`. One pass after each op adds the hint, so
//! no emit site repeats it.

use std::collections::BTreeSet;

use zenith_core::{Diagnostic, Document, nearest_names};

use super::structure::collect_doc_ids;

/// The most ids a hint names.
const HINT_LIMIT: usize = 3;

/// Append `(did you mean: a, b, c?)` to every `tx.unknown_node` in
/// `diagnostics` that names a subject with close ids in `doc`.
pub(super) fn add_unknown_node_hints<'d>(
    doc: &Document,
    diagnostics: impl Iterator<Item = &'d mut Diagnostic>,
) {
    let mut ids: Option<BTreeSet<String>> = None;
    for d in diagnostics {
        if d.code != "tx.unknown_node" {
            continue;
        }
        let Some(subject) = d.subject_id.as_deref() else {
            continue;
        };
        let ids = ids.get_or_insert_with(|| reachable_ids(doc));
        // A third of the id length in edits, at least two.
        let budget = (subject.chars().count() / 3).max(2);
        let near = nearest_names(subject, ids.iter().map(String::as_str), budget, HINT_LIMIT);
        if !near.is_empty() {
            let hint = format!(" (did you mean: {}?)", near.join(", "));
            d.message.push_str(&hint);
        }
    }
}

/// Every id the node resolver reaches: pages, masters, component
/// definitions, and the nodes inside them.
fn reachable_ids(doc: &Document) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    collect_doc_ids(doc, &mut ids);
    ids.extend(doc.masters.iter().map(|m| m.id.clone()));
    ids.extend(doc.components.iter().map(|c| c.id.clone()));
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource};

    const DOC: &str = r##"zenith version=1 {
  project id="proj" name="T"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc" title="T" {
    page id="p5" w=(px)100 h=(px)100 {
      rect id="p5.kicker.5b" x=(px)0 y=(px)0 w=(px)5 h=(px)5
      rect id="p5.title.5b" x=(px)0 y=(px)0 w=(px)5 h=(px)5
    }
  }
}"##;

    fn hinted(subject: &str) -> String {
        let doc = KdlAdapter.parse(DOC.as_bytes()).expect("parses");
        let mut d = vec![Diagnostic::error(
            "tx.unknown_node",
            format!("node {subject:?} not found in document"),
            None,
            Some(subject.to_owned()),
        )];
        add_unknown_node_hints(&doc, d.iter_mut());
        d.remove(0).message
    }

    #[test]
    fn a_near_id_is_suggested() {
        let message = hinted("p5b.kicker.5b");
        assert!(
            message.ends_with("(did you mean: p5.kicker.5b?)"),
            "{message}"
        );
    }

    #[test]
    fn a_far_id_gets_no_hint() {
        let message = hinted("completely.unrelated.name");
        assert!(!message.contains("did you mean"), "{message}");
    }

    #[test]
    fn other_codes_are_left_alone() {
        let doc = KdlAdapter.parse(DOC.as_bytes()).expect("parses");
        let mut d = [Diagnostic::error(
            "tx.noop",
            "x".to_owned(),
            None,
            Some("p5b.kicker.5b".to_owned()),
        )];
        add_unknown_node_hints(&doc, d.iter_mut());
        assert_eq!(d[0].message, "x");
    }
}
