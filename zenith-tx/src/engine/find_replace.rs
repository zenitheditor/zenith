//! `FindReplaceText`: literal find-and-replace across text spans, shape
//! labels, and instance override texts.
//!
//! A scoped run edits one text node, shape, or instance. On an instance it
//! edits each override text, and adds a text override for a component text
//! child whose own text matches, so only that instance changes. A
//! document-wide run walks pages, masters, and component definitions. It
//! edits a component text child in the definition, so every instance
//! without its own text override changes.

use zenith_core::{Diagnostic, Document, InstanceNode, Node, Override, TextSpan};

use super::instance_override::{component_children, empty_override};
use super::{
    clear_placeholder, find_node_any_mut, find_node_any_shared, record_affected, root_lists_mut,
};

/// Replace all occurrences of `find` in every span's `text` of `spans`,
/// returning `true` if any span was modified.
///
/// Only `TextSpan::text` is mutated; all formatting fields are preserved.
/// Shared by text nodes, shape labels, and override spans.
fn replace_in_spans(spans: &mut [TextSpan], find: &str, replace: &str) -> bool {
    let mut changed = false;
    for span in spans {
        if span.text.contains(find) {
            span.text = span.text.replace(find, replace);
            changed = true;
        }
    }
    changed
}

/// `true` when any span text of `spans` contains `find`.
fn spans_contain(spans: &[TextSpan], find: &str) -> bool {
    spans.iter().any(|s| s.text.contains(find))
}

/// Replace `find` in every override text of `inst`. `true` when any changed.
fn replace_in_overrides(inst: &mut InstanceNode, find: &str, replace: &str) -> bool {
    let mut changed = false;
    for ov in &mut inst.overrides {
        if let Some(spans) = ov.spans.as_mut() {
            changed |= replace_in_spans(spans, find, replace);
        }
    }
    changed
}

/// `true` when any override text of `inst` contains `find`.
fn overrides_contain(inst: &InstanceNode, find: &str) -> bool {
    inst.overrides
        .iter()
        .any(|ov| ov.spans.as_deref().is_some_and(|s| spans_contain(s, find)))
}

/// The new text overrides a scoped run adds to `inst`: one per component
/// text child that has no text override on `inst` and whose text contains
/// `find`. Each holds the child's spans with `find` replaced.
fn new_text_overrides(
    doc: &Document,
    inst: &InstanceNode,
    find: &str,
    replace: &str,
) -> Vec<(String, Vec<TextSpan>)> {
    let Some(children) = component_children(doc, inst) else {
        return Vec::new();
    };
    let mut texts = Vec::new();
    collect_component_texts(children, &mut texts);
    texts
        .into_iter()
        .filter(|(id, spans)| {
            let overridden = inst
                .overrides
                .iter()
                .any(|ov| ov.ref_id == *id && ov.spans.is_some());
            !overridden && spans_contain(spans, find)
        })
        .map(|(id, mut spans)| {
            replace_in_spans(&mut spans, find, replace);
            (id, spans)
        })
        .collect()
}

/// Push `(id, spans)` for every text node an override can reach below
/// `nodes`: the descent of the scene's override walk (frames and groups).
fn collect_component_texts(nodes: &[Node], out: &mut Vec<(String, Vec<TextSpan>)>) {
    for node in nodes {
        match node {
            Node::Text(t) => out.push((t.id.clone(), t.spans.clone())),
            Node::Frame(f) => collect_component_texts(&f.children, out),
            Node::Group(g) => collect_component_texts(&g.children, out),
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Code(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Footnote(_)
            | Node::Toc(_)
            | Node::Table(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => {}
        }
    }
}

/// The `tx.noop` advisory of a scoped run that found nothing.
fn scoped_noop(find: &str, node_id: &str) -> Diagnostic {
    Diagnostic::advisory(
        "tx.noop",
        format!("find_replace_text: {find:?} not found in node {node_id:?}; document is unchanged"),
        None,
        Some(node_id.to_owned()),
    )
}

/// Apply a literal find-and-replace across one or all text nodes in `doc`.
///
/// See [`crate::op::Op::FindReplaceText`] for the full specification.
pub(super) fn apply_find_replace_text(
    find: &str,
    replace: &str,
    node: Option<&str>,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    // Eager: empty find string is invalid.
    if find.is_empty() {
        diagnostics.push(Diagnostic::error(
            "tx.invalid_value",
            "find string must be non-empty",
            None,
            None,
        ));
        return;
    }
    match node {
        Some(node_id) => scoped(find, replace, node_id, doc, diagnostics, affected),
        None => document_wide(find, replace, doc, diagnostics, affected),
    }
}

/// Scoped mode: one text node, shape, or instance.
fn scoped(
    find: &str,
    replace: &str,
    node_id: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    // Phase 1 (shared): the overrides an instance scope adds.
    let added = match find_node_any_shared(doc, node_id) {
        Some(Node::Instance(inst)) => new_text_overrides(doc, inst, find, replace),
        Some(_) | None => Vec::new(),
    };
    // Phase 2 (exclusive): edit the node.
    match find_node_any_mut(doc, node_id) {
        None => {
            diagnostics.push(Diagnostic::error(
                "tx.unknown_node",
                format!("node {:?} not found in document", node_id),
                None,
                Some(node_id.to_owned()),
            ));
        }
        Some(Node::Text(text_node)) => {
            if replace_in_spans(&mut text_node.spans, find, replace) {
                clear_placeholder(&mut text_node.role);
                record_affected(node_id, affected);
            } else {
                diagnostics.push(scoped_noop(find, node_id));
            }
        }
        Some(Node::Shape(shape_node)) => {
            if replace_in_spans(&mut shape_node.spans, find, replace) {
                clear_placeholder(&mut shape_node.role);
                record_affected(node_id, affected);
            } else {
                diagnostics.push(scoped_noop(find, node_id));
            }
        }
        Some(Node::Instance(inst)) => {
            let mut changed = replace_in_overrides(inst, find, replace);
            for (ref_id, spans) in added {
                changed = true;
                match inst.overrides.iter_mut().find(|ov| ov.ref_id == ref_id) {
                    Some(ov) => ov.spans = Some(spans),
                    None => inst.overrides.push(Override {
                        spans: Some(spans),
                        ..empty_override(&ref_id)
                    }),
                }
            }
            if changed {
                record_affected(node_id, affected);
            } else {
                diagnostics.push(scoped_noop(find, node_id));
            }
        }
        Some(other) => {
            let kind = other.kind_str();
            diagnostics.push(Diagnostic::error(
                "tx.wrong_node_type",
                format!(
                    "find_replace_text requires a text, shape, or instance node but {:?} is a {}",
                    node_id, kind
                ),
                None,
                Some(node_id.to_owned()),
            ));
        }
    }
}

/// The ids a document-wide walk changed and the locked ids it skipped.
struct Walk<'a> {
    find: &'a str,
    replace: &'a str,
    changed: Vec<String>,
    skipped: Vec<String>,
}

impl Walk<'_> {
    /// Edit every text target below `nodes`, in document order.
    fn nodes(&mut self, nodes: &mut [Node]) {
        for node in nodes {
            match node {
                Node::Text(t) => {
                    if t.locked == Some(true) {
                        self.skipped.push(t.id.clone());
                    } else if replace_in_spans(&mut t.spans, self.find, self.replace) {
                        clear_placeholder(&mut t.role);
                        self.changed.push(t.id.clone());
                    }
                }
                // A shape owns its label spans directly (not as child Nodes),
                // so it is a leaf here.
                Node::Shape(s) => {
                    if s.locked == Some(true) {
                        self.skipped.push(s.id.clone());
                    } else if replace_in_spans(&mut s.spans, self.find, self.replace) {
                        clear_placeholder(&mut s.role);
                        self.changed.push(s.id.clone());
                    }
                }
                Node::Instance(inst) => {
                    if !overrides_contain(inst, self.find) {
                        continue;
                    }
                    if inst.locked == Some(true) {
                        self.skipped.push(inst.id.clone());
                    } else if replace_in_overrides(inst, self.find, self.replace) {
                        self.changed.push(inst.id.clone());
                    }
                }
                Node::Frame(f) => self.nodes(&mut f.children),
                Node::Group(g) => self.nodes(&mut g.children),
                Node::Table(t) => {
                    for row in &mut t.rows {
                        for cell in &mut row.cells {
                            self.nodes(&mut cell.children);
                        }
                    }
                }
                Node::Rect(_)
                | Node::Ellipse(_)
                | Node::Line(_)
                | Node::Code(_)
                | Node::Image(_)
                | Node::Polygon(_)
                | Node::Polyline(_)
                | Node::Path(_)
                | Node::Field(_)
                | Node::Toc(_)
                | Node::Footnote(_)
                | Node::Connector(_)
                | Node::Pattern(_)
                | Node::Chart(_)
                | Node::Light(_)
                | Node::Mesh(_)
                | Node::Unknown(_) => {}
            }
        }
    }
}

/// Document-wide mode: pages, then masters, then component definitions.
fn document_wide(
    find: &str,
    replace: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let mut walk = Walk {
        find,
        replace,
        changed: Vec::new(),
        skipped: Vec::new(),
    };
    for children in root_lists_mut(doc) {
        walk.nodes(children);
    }
    for id in &walk.changed {
        record_affected(id, affected);
    }

    // Sort skipped list for determinism, then emit a warning if any.
    walk.skipped.sort();
    walk.skipped.dedup();
    if !walk.skipped.is_empty() {
        diagnostics.push(Diagnostic::warning(
            "tx.locked_skipped",
            format!(
                "find_replace_text: skipped locked node(s): {}",
                walk.skipped.join(", ")
            ),
            None,
            None,
        ));
    }

    // If this op changed nothing AND nothing was skipped, emit noop advisory.
    if walk.changed.is_empty() && walk.skipped.is_empty() {
        diagnostics.push(Diagnostic::advisory(
            "tx.noop",
            format!(
                "find_replace_text: {:?} not found in any text node, shape label, or \
                 instance override; document is unchanged",
                find
            ),
            None,
            None,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource};

    const SRC: &str = r##"zenith version=1 {
  project id="proj" name="T"
  tokens format="zenith-token-v1" { }
  styles { }
  components {
    component id="card" {
      text id="card.title" x=(px)0 y=(px)0 w=(px)100 h=(px)20 { span "Title" }
      group id="card.inner" {
        text id="card.body" x=(px)0 y=(px)30 w=(px)100 h=(px)20 { span "Body text" }
      }
    }
  }
  document id="d" {
    page id="p" w=(px)400 h=(px)300 {
      instance id="c1" component="card" {
        override ref="card.title" { span "Fast setup" }
      }
      instance id="c2" component="card"
      text id="t" x=(px)0 y=(px)200 w=(px)100 h=(px)20 { span "Fast lane" }
    }
  }
}"##;

    fn run(
        find: &str,
        replace: &str,
        node: Option<&str>,
    ) -> (Document, Vec<Diagnostic>, Vec<String>) {
        let mut doc = KdlAdapter.parse(SRC.as_bytes()).expect("fixture parses");
        let mut diags = Vec::new();
        let mut affected = Vec::new();
        apply_find_replace_text(find, replace, node, &mut doc, &mut diags, &mut affected);
        (doc, diags, affected)
    }

    fn instance<'d>(doc: &'d Document, id: &str) -> &'d InstanceNode {
        match find_node_any_shared(doc, id) {
            Some(Node::Instance(inst)) => inst,
            other => panic!("{id} is not an instance: {other:?}"),
        }
    }

    fn override_text(inst: &InstanceNode, ref_id: &str) -> Option<String> {
        inst.overrides
            .iter()
            .find(|ov| ov.ref_id == ref_id)
            .and_then(|ov| ov.spans.as_ref())
            .map(|s| s.iter().map(|sp| sp.text.as_str()).collect())
    }

    #[test]
    fn document_wide_edits_override_spans_and_page_text() {
        let (doc, diags, affected) = run("Fast", "Quick", None);
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(affected, ["c1", "t"]);
        assert_eq!(
            override_text(instance(&doc, "c1"), "card.title").as_deref(),
            Some("Quick setup")
        );
    }

    #[test]
    fn document_wide_edits_the_component_definition() {
        let (doc, _, affected) = run("Body", "Copy", None);
        assert_eq!(affected, ["card.body"]);
        match find_node_any_shared(&doc, "card.body") {
            Some(Node::Text(t)) => assert_eq!(t.spans[0].text, "Copy text"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn instance_scope_adds_an_override_for_one_instance() {
        let (doc, diags, affected) = run("Body", "Copy", Some("c2"));
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(affected, ["c2"]);
        assert_eq!(
            override_text(instance(&doc, "c2"), "card.body").as_deref(),
            Some("Copy text")
        );
        // The definition and the other instance keep their text.
        assert_eq!(override_text(instance(&doc, "c1"), "card.body"), None);
        match find_node_any_shared(&doc, "card.body") {
            Some(Node::Text(t)) => assert_eq!(t.spans[0].text, "Body text"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn instance_scope_with_no_match_is_a_noop() {
        let (_, diags, affected) = run("zzz", "y", Some("c1"));
        assert!(affected.is_empty());
        assert_eq!(diags[0].code, "tx.noop");
    }

    #[test]
    fn component_child_scope_edits_the_definition() {
        let (_, diags, affected) = run("Title", "Head", Some("card.title"));
        assert!(diags.is_empty(), "{diags:?}");
        assert_eq!(affected, ["card.title"]);
    }
}
