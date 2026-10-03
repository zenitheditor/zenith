//! `Reparent` application: move a node into another container and keep its
//! page position.

use zenith_core::{Diagnostic, Document, Node, translate_node};

use crate::op::Position;

use super::super::layout::places_in_flow;
use super::super::space::{
    container_chain, parent_chain, parent_frame, resolved_tokens, shift_between,
};
use super::super::{find_node_any_shared, record_affected, subtree_contains};
use super::finders::{find_container_children_mut, remove_node_by_id, resolve_position};

/// How a reparent rewrites the moved node's coordinates.
enum Conversion {
    /// The coordinates stay as authored.
    Keep,
    /// Add `(dx, dy)` px: the old space origin minus the new one.
    Shift(f64, f64),
    /// The page position is not known. The string names the container
    /// whose origin or layout placement does not resolve.
    Unresolved(String),
}

/// Decide the conversion before any mutation.
///
/// - Into a flow slot of a `row` / `column` / `grid` frame: keep (the frame
///   places the node).
/// - Out of a flow slot: unresolved (the old frame placed the node).
/// - Otherwise shift by the origin difference below the shared ancestor.
///   An unresolved origin on either side gives unresolved.
fn conversion(doc: &Document, node_id: &str, new_parent: &str) -> Conversion {
    let Some(node) = find_node_any_shared(doc, node_id) else {
        return Conversion::Keep;
    };
    if let Some(Node::Frame(target)) = find_node_any_shared(doc, new_parent)
        && places_in_flow(target, node)
    {
        return Conversion::Keep;
    }
    if let Some(old) = parent_frame(doc, node_id)
        && places_in_flow(old, node)
    {
        return Conversion::Unresolved(old.id.clone());
    }
    let resolved = resolved_tokens(doc);
    let (Some(from), Some(to)) = (
        parent_chain(doc, node_id, &resolved),
        container_chain(doc, new_parent, &resolved),
    ) else {
        // A missing container is reported when the insert runs.
        return Conversion::Keep;
    };
    match shift_between(&from, &to) {
        Err(container) => Conversion::Unresolved(container),
        Ok((dx, dy)) if dx == 0.0 && dy == 0.0 => Conversion::Keep,
        Ok((dx, dy)) => Conversion::Shift(dx, dy),
    }
}

/// The top-level list that holds a node.
#[derive(Clone, Copy)]
enum Host {
    Page(usize),
    Master(usize),
}

/// The page or master whose subtree holds node `id`.
fn host_of(doc: &Document, id: &str) -> Option<Host> {
    let holds = |children: &[Node]| children.iter().any(|n| subtree_contains(n, id));
    doc.body
        .pages
        .iter()
        .position(|p| holds(&p.children))
        .map(Host::Page)
        .or_else(|| {
            doc.masters
                .iter()
                .position(|m| holds(&m.children))
                .map(Host::Master)
        })
}

/// The top-level children of `host`.
fn host_children(doc: &mut Document, host: Host) -> Option<&mut Vec<Node>> {
    match host {
        Host::Page(i) => doc.body.pages.get_mut(i).map(|p| &mut p.children),
        Host::Master(i) => doc.masters.get_mut(i).map(|m| &mut m.children),
    }
}

pub(in crate::engine) fn apply_reparent(
    node_id: &str,
    new_parent: &str,
    position: &Position,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    // Phase 1 (shared scan): find the page or master that holds the node,
    // and run the cycle check without a mutable borrow.
    let Some(host) = host_of(doc, node_id) else {
        diagnostics.push(Diagnostic::error(
            "tx.unknown_node",
            format!("node {:?} not found in document", node_id),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    };

    // Cycle check: new_parent must not be node itself or a descendant of node.
    if find_node_any_shared(doc, node_id).is_some_and(|n| subtree_contains(n, new_parent)) {
        diagnostics.push(Diagnostic::error(
            "tx.invalid_parent",
            format!(
                "cannot reparent {:?} into {:?}: new_parent is within \
                 the node's own subtree",
                node_id, new_parent
            ),
            None,
            Some(new_parent.to_owned()),
        ));
        return;
    }

    let conversion = conversion(doc, node_id, new_parent);
    let resolved = resolved_tokens(doc);

    // Phase 2 (exclusive borrows): remove then re-insert.
    // Step 2a — remove the node from its current parent.
    let Some(mut node) = host_children(doc, host).and_then(|c| remove_node_by_id(c, node_id))
    else {
        // Unexpected: the shared scan found it but remove didn't.
        diagnostics.push(Diagnostic::error(
            "tx.unknown_node",
            format!("node {:?} disappeared during reparent", node_id),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    };

    // Step 2b — locate the new parent's children vec.
    // `find_container_children_mut` handles page ids AND nested container ids.
    let new_children = match find_container_children_mut(doc, new_parent) {
        Some(c) => c,
        None => {
            // new_parent is not a container — roll back by re-inserting the node
            // at the end of its original page (best-effort; the transaction will
            // be rejected by the error diagnostic anyway).
            if let Some(children) = host_children(doc, host) {
                children.push(node);
            }
            diagnostics.push(Diagnostic::error(
                "tx.invalid_parent",
                format!(
                    "no container with id {:?} (new_parent must be a page, group, or frame)",
                    new_parent
                ),
                None,
                Some(new_parent.to_owned()),
            ));
            return;
        }
    };

    // Step 2c — resolve the insertion index, convert, and insert.
    let idx = match resolve_position(position, new_children, new_parent, diagnostics) {
        Some(i) => i,
        None => {
            // resolve_position already pushed a diagnostic; roll back.
            if let Some(children) = host_children(doc, host) {
                children.push(node);
            }
            return;
        }
    };

    match conversion {
        Conversion::Keep => {}
        Conversion::Shift(dx, dy) => translate_node(&mut node, dx, dy, &resolved),
        Conversion::Unresolved(container) => diagnostics.push(Diagnostic::advisory(
            "tx.coordinate_unresolved",
            format!(
                "reparent: node {node_id:?} keeps its x/y unchanged: the px origin of \
                 container {container:?} does not resolve, or a layout frame places it; \
                 check the node's position and set x/y with set_geometry"
            ),
            None,
            Some(node_id.to_owned()),
        )),
    }
    new_children.insert(idx, node);
    record_affected(node_id, affected);
}
