//! `AddNode` / `AddPath` / `RemoveNode` application: build a node from a `.zen`
//! fragment or typed payload, locate the parent container, and insert/remove.

use zenith_core::{
    AnchorKind, Diagnostic, Document, KdlAdapter, KdlSource, Node, PathAnchor as CorePathAnchor,
    PathNode, PathSubpath,
};

use crate::op::{OpPathAnchor, OpPathSubpath, Position};

use super::super::{px, record_affected};
use super::finders::{find_container_children_mut, remove_node_by_id, resolve_position};

/// Construct a single [`Node`] from a `.zen` node fragment by wrapping it in a
/// minimal synthetic document and parsing it through the canonical KDL parser.
///
/// Reusing the parser means every node kind, nested children (for group/frame),
/// tokens, and properties are supported with no per-field mapping. The wrapper's
/// `tokens`/`styles` blocks are left to their AST defaults (empty) — the real
/// candidate document, which carries the real tokens/assets, is what
/// post-validation actually checks.
///
/// Returns `Err` with a human-readable message if the fragment does not parse or
/// does not contain exactly one top-level node.
fn build_node_from_fragment(fragment: &str) -> Result<Node, String> {
    let synthetic = format!(
        "zenith version=1 {{\n  document id=\"__tx_doc\" {{\n    page id=\"__tx_page\" w=(px)1 h=(px)1 {{\n{fragment}\n    }}\n  }}\n}}\n"
    );
    let doc = KdlAdapter
        .parse(synthetic.as_bytes())
        .map_err(|e| format!("failed to parse node fragment: {e}"))?;
    let mut page = doc
        .body
        .pages
        .into_iter()
        .next()
        .ok_or_else(|| "synthetic document produced no page".to_owned())?;
    if page.children.len() != 1 {
        return Err(format!(
            "expected exactly one node in fragment, found {}",
            page.children.len()
        ));
    }
    Ok(page.children.remove(0))
}

fn path_anchor_from_op(anchor: &OpPathAnchor) -> CorePathAnchor {
    CorePathAnchor {
        x: Some(px(anchor.x)),
        y: Some(px(anchor.y)),
        kind: anchor.kind.as_deref().map(AnchorKind::from_kind_str),
        in_x: anchor.in_x.map(px),
        in_y: anchor.in_y.map(px),
        out_x: anchor.out_x.map(px),
        out_y: anchor.out_y.map(px),
    }
}

fn path_subpath_from_op(subpath: &OpPathSubpath) -> PathSubpath {
    PathSubpath {
        closed: subpath.closed,
        anchors: subpath.anchors.iter().map(path_anchor_from_op).collect(),
    }
}

fn empty_path_node(id: &str) -> PathNode {
    PathNode {
        id: id.to_owned(),
        name: None,
        role: None,
        closed: None,
        fill: None,
        stroke: None,
        stroke_width: None,
        stroke_alignment: None,
        stroke_linejoin: None,
        stroke_linecap: None,
        stroke_miter_limit: None,
        fill_rule: None,
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        style: None,
        anchors: Vec::new(),
        subpaths: Vec::new(),
        source_span: None,
        unknown_props: Default::default(),
    }
}

fn build_path_node(
    id: &str,
    closed: Option<bool>,
    anchors: &[OpPathAnchor],
    subpaths: &[OpPathSubpath],
) -> Result<Node, String> {
    let has_direct = !anchors.is_empty();
    let has_compound = !subpaths.is_empty();
    if has_direct && has_compound {
        return Err("path payload cannot mix direct anchors with subpaths".to_owned());
    }
    if !has_direct && !has_compound {
        return Err("path payload must include direct anchors or subpaths".to_owned());
    }
    if has_compound && closed.is_some() {
        return Err("closed is invalid when subpaths are present".to_owned());
    }

    let mut path = empty_path_node(id);
    if has_direct {
        path.closed = closed;
        path.anchors = anchors.iter().map(path_anchor_from_op).collect();
    } else {
        path.subpaths = subpaths.iter().map(path_subpath_from_op).collect();
    }

    Ok(Node::Path(path))
}

fn insert_node(
    parent: &str,
    position: &Position,
    node: Node,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let children = match find_container_children_mut(doc, parent) {
        Some(c) => c,
        None => {
            diagnostics.push(Diagnostic::error(
                "tx.invalid_parent",
                format!(
                    "no container with id {:?} (parent must be a page, master, group, or frame)",
                    parent
                ),
                None,
                Some(parent.to_owned()),
            ));
            return;
        }
    };

    let idx = match resolve_position(position, children, parent, diagnostics) {
        Some(i) => i,
        None => return,
    };

    let new_id = &node.id().map(|s| s.to_owned());
    children.insert(idx, node);
    if let Some(id) = new_id {
        record_affected(id, affected);
    }
}

pub(in crate::engine) struct AddPathSpec<'a> {
    pub(in crate::engine) parent: &'a str,
    pub(in crate::engine) id: &'a str,
    pub(in crate::engine) position: &'a Position,
    pub(in crate::engine) closed: Option<bool>,
    pub(in crate::engine) anchors: &'a [OpPathAnchor],
    pub(in crate::engine) subpaths: &'a [OpPathSubpath],
}

pub(in crate::engine) fn apply_add_node(
    parent: &str,
    position: &Position,
    source: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    // 1. Build the node from the `.zen` fragment.
    let node = match build_node_from_fragment(source) {
        Ok(n) => n,
        Err(e) => {
            diagnostics.push(Diagnostic::error(
                "tx.invalid_node_spec",
                format!("could not construct node from source fragment: {e}"),
                None,
                None,
            ));
            return;
        }
    };

    insert_node(parent, position, node, doc, diagnostics, affected);
    // Post-validation handles duplicate-id / missing-geometry / unknown-token / etc.
}

pub(in crate::engine) fn apply_add_path(
    spec: AddPathSpec<'_>,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let node = match build_path_node(spec.id, spec.closed, spec.anchors, spec.subpaths) {
        Ok(n) => n,
        Err(e) => {
            diagnostics.push(Diagnostic::error(
                "tx.invalid_node_spec",
                format!("could not construct path node: {e}"),
                None,
                Some(spec.id.to_owned()),
            ));
            return;
        }
    };

    insert_node(spec.parent, spec.position, node, doc, diagnostics, affected);
    // Post-validation handles duplicate-id / invalid geometry / unknown anchor kind / etc.
}

pub(in crate::engine) fn apply_remove_node(
    node_id: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    for page in doc.body.pages.iter_mut() {
        if remove_node_by_id(&mut page.children, node_id).is_some() {
            record_affected(node_id, affected);
            return;
        }
    }
    for master in doc.masters.iter_mut() {
        if remove_node_by_id(&mut master.children, node_id).is_some() {
            record_affected(node_id, affected);
            return;
        }
    }
    diagnostics.push(Diagnostic::error(
        "tx.unknown_node",
        format!("no node with id {:?}", node_id),
        None,
        Some(node_id.to_owned()),
    ));
}
