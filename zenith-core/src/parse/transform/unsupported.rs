//! Capture of child KDL nodes that a node kind does not consume.
//!
//! The per-kind [`child_policy`] table is DERIVED from what each node kind's
//! transform actually reads out of its KDL children block (see the sibling
//! `leaf`, `special`, `container`, `chart`, and `pattern` modules). Only child
//! node NAMES that a transform genuinely consumes are recognized; anything else
//! authored inside the block is dropped at parse time, so it is captured here
//! for a `node.unsupported_child` Warning from validation.
//!
//! Called from the single funnel [`super::node::transform_node`] on every node,
//! against that node's OWN children. Container recursion (frame/group/table
//! cells/pattern motif) threads the same sink so nested nodes are checked in the
//! exact position the transform descends into them — a dropped subtree is never
//! walked, so only the outermost discarded child is reported.

use kdl::KdlNode;

use crate::ast::UnsupportedChild;

use super::helpers::{node_span, optional_string_prop};

/// How a node kind treats its authored child KDL nodes.
enum ChildPolicy {
    /// Children are arbitrary renderable nodes (or already handled metadata).
    /// An unrecognized child KIND becomes `Node::Unknown` and yields the
    /// existing `node.unknown_kind` diagnostic, so nothing is flagged here.
    Container,
    /// The exact set of child node names the kind's transform consumes. Any
    /// child whose name is not listed is dropped, hence flagged.
    Allowed(&'static [&'static str]),
    /// `pattern`: the FIRST child is the motif (any kind); every subsequent
    /// child is dropped, hence flagged.
    PatternMotif,
}

/// Return how the given node kind treats its children.
///
/// Derived per kind from its transform:
/// - Leaves read no children at all → `Allowed(&[])` (any child is dropped).
/// - Content kinds read a fixed set of child node names → `Allowed(names)`.
/// - `pattern` consumes only its first child → `PatternMotif`.
/// - `group`/`frame` (and any unknown kind) take arbitrary nodes → `Container`.
fn child_policy(kind: &str) -> ChildPolicy {
    match kind {
        // Leaf kinds: their transforms never read `node.children()`.
        "rect" | "ellipse" | "line" | "image" | "field" | "toc" | "light" | "mesh" => {
            ChildPolicy::Allowed(&[])
        }
        // Text bearers.
        "text" => ChildPolicy::Allowed(&["block", "kern-pair", "span"]),
        // `code` carries its source in a `content` child, with optional
        // `kern-pair` children. A `span` here is NOT consumed (that is `text`'s
        // shape), so it is discarded and must be reported.
        "code" => ChildPolicy::Allowed(&["content", "kern-pair"]),
        "shape" | "connector" | "footnote" => ChildPolicy::Allowed(&["span"]),
        // Vertex / anchor bearers.
        "polygon" | "polyline" => ChildPolicy::Allowed(&["point"]),
        "path" => ChildPolicy::Allowed(&["anchor", "subpath"]),
        // Structured content.
        "table" => ChildPolicy::Allowed(&["column", "row"]),
        "chart" => ChildPolicy::Allowed(&["series", "categories", "label-colors", "slice-colors"]),
        "instance" => ChildPolicy::Allowed(&["override"]),
        // Motif bearer.
        "pattern" => ChildPolicy::PatternMotif,
        // `group` and `frame` take arbitrary renderable nodes; an unknown kind
        // likewise recurses via `transform_children`. Neither is flagged here.
        _ => ChildPolicy::Container,
    }
}

/// Record every child of `node` that its kind does not consume into `sink`.
///
/// Reads only the raw KDL (child names, the parent id, and spans); it never
/// inspects the transformed AST, so it stays a pure classification pass.
pub(super) fn collect_unsupported_children(node: &KdlNode, sink: &mut Vec<UnsupportedChild>) {
    match child_policy(node.name().value()) {
        ChildPolicy::Container => {}
        ChildPolicy::Allowed(allowed) => {
            if let Some(doc) = node.children() {
                for child in doc.nodes() {
                    if !allowed.contains(&child.name().value()) {
                        push_unsupported(node, child, sink);
                    }
                }
            }
        }
        ChildPolicy::PatternMotif => {
            if let Some(doc) = node.children() {
                // The first child is the consumed motif; the rest are dropped.
                for child in doc.nodes().iter().skip(1) {
                    push_unsupported(node, child, sink);
                }
            }
        }
    }
}

/// Push one [`UnsupportedChild`] record describing `child` dropped under `parent`.
fn push_unsupported(parent: &KdlNode, child: &KdlNode, sink: &mut Vec<UnsupportedChild>) {
    sink.push(UnsupportedChild {
        parent_id: optional_string_prop(parent, "id").map(str::to_owned),
        parent_kind: parent.name().value().to_owned(),
        child_kind: child.name().value().to_owned(),
        source_span: node_span(child),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A representative child name that each `Allowed` kind consumes, so a
    /// well-authored document never trips the classifier.
    fn is_allowed(kind: &str, child: &str) -> bool {
        match child_policy(kind) {
            ChildPolicy::Container => true,
            ChildPolicy::PatternMotif => false,
            ChildPolicy::Allowed(allowed) => allowed.contains(&child),
        }
    }

    #[test]
    fn leaf_kinds_consume_no_children() {
        for kind in [
            "rect", "ellipse", "line", "image", "field", "toc", "light", "mesh",
        ] {
            match child_policy(kind) {
                ChildPolicy::Allowed(allowed) => assert!(
                    allowed.is_empty(),
                    "{kind} must have an empty child allowlist"
                ),
                _ => panic!("{kind} must be an Allowed(&[]) leaf"),
            }
        }
    }

    #[test]
    fn content_kinds_recognize_their_children() {
        assert!(is_allowed("text", "span"));
        assert!(is_allowed("text", "block"));
        assert!(is_allowed("text", "kern-pair"));
        assert!(is_allowed("shape", "span"));
        assert!(is_allowed("footnote", "span"));
        assert!(is_allowed("connector", "span"));
        assert!(is_allowed("polygon", "point"));
        assert!(is_allowed("polyline", "point"));
        assert!(is_allowed("path", "anchor"));
        assert!(is_allowed("path", "subpath"));
        assert!(is_allowed("table", "column"));
        assert!(is_allowed("table", "row"));
        assert!(is_allowed("chart", "series"));
        assert!(is_allowed("chart", "categories"));
        assert!(is_allowed("chart", "label-colors"));
        assert!(is_allowed("chart", "slice-colors"));
        assert!(is_allowed("instance", "override"));
        assert!(is_allowed("code", "content"));
    }

    #[test]
    fn containers_and_unknown_kinds_flag_nothing() {
        assert!(matches!(child_policy("group"), ChildPolicy::Container));
        assert!(matches!(child_policy("frame"), ChildPolicy::Container));
        assert!(matches!(
            child_policy("no-such-kind"),
            ChildPolicy::Container
        ));
    }

    #[test]
    fn pattern_is_motif_positional() {
        assert!(matches!(child_policy("pattern"), ChildPolicy::PatternMotif));
    }

    #[test]
    fn unrecognized_child_is_not_allowed() {
        assert!(!is_allowed("ellipse", "text"));
        assert!(!is_allowed("text", "rect"));
        assert!(!is_allowed("polygon", "anchor"));
        assert!(!is_allowed("path", "point"));
        assert!(!is_allowed("instance", "span"));
    }
}
