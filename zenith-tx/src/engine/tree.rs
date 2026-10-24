//! Shared tree-walk helpers and small utilities used across the engine.
//!
//! These functions traverse the document node tree (shared and mutable),
//! extract per-node identity/kind, and record affected ids. They are pure and
//! contain no per-op business logic.

use zenith_core::{Dimension, Document, Node, Unit};

// ── Shared tree-walk helpers ──────────────────────────────────────────────────

/// Returns true if `node` is, or transitively contains, a node with `id`.
pub(super) fn subtree_contains(node: &Node, id: &str) -> bool {
    if node.id() == Some(id) {
        return true;
    }
    match node {
        Node::Frame(f) => f.children.iter().any(|c| subtree_contains(c, id)),
        Node::Group(g) => g.children.iter().any(|c| subtree_contains(c, id)),
        Node::Table(t) => t.rows.iter().any(|r| {
            r.cells
                .iter()
                .any(|c| c.children.iter().any(|ch| subtree_contains(ch, id)))
        }),
        Node::Unknown(u) => u.children.iter().any(|c| subtree_contains(c, id)),
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Line(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Image(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Footnote(_)
        | Node::Toc(_)
        | Node::Shape(_)
        | Node::Connector(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_) => false,
    }
}

// ── Root lists ────────────────────────────────────────────────────────────────

/// The top-level child lists that hold addressable nodes, with the id of
/// their owner, in lookup order: pages, then masters, then component
/// definitions.
///
/// This order is the one addressing rule of the engine. A node id resolves
/// to the first list that holds it. Master and component child ids are
/// local to their owner, so a page node wins over a master or component
/// child with the same id. An edit to a component child changes every
/// instance of the component.
pub(crate) fn root_entries(doc: &Document) -> impl Iterator<Item = (&str, &[Node])> {
    let pages = doc
        .body
        .pages
        .iter()
        .map(|p| (p.id.as_str(), p.children.as_slice()));
    let masters = doc
        .masters
        .iter()
        .map(|m| (m.id.as_str(), m.children.as_slice()));
    let components = doc
        .components
        .iter()
        .map(|c| (c.id.as_str(), c.children.as_slice()));
    pages.chain(masters).chain(components)
}

/// The mutable form of [`root_entries`], in the same order.
pub(crate) fn root_entries_mut(doc: &mut Document) -> impl Iterator<Item = (&str, &mut Vec<Node>)> {
    let pages = doc
        .body
        .pages
        .iter_mut()
        .map(|p| (p.id.as_str(), &mut p.children));
    let masters = doc
        .masters
        .iter_mut()
        .map(|m| (m.id.as_str(), &mut m.children));
    let components = doc
        .components
        .iter_mut()
        .map(|c| (c.id.as_str(), &mut c.children));
    pages.chain(masters).chain(components)
}

/// The child lists of [`root_entries`] without their owner ids.
pub(crate) fn root_lists(doc: &Document) -> impl Iterator<Item = &[Node]> {
    root_entries(doc).map(|(_, children)| children)
}

/// The child lists of [`root_entries_mut`] without their owner ids.
pub(crate) fn root_lists_mut(doc: &mut Document) -> impl Iterator<Item = &mut Vec<Node>> {
    root_entries_mut(doc).map(|(_, children)| children)
}

/// The index in [`root_lists`] of the first list whose subtree holds `id`.
pub(crate) fn root_index_of(doc: &Document, id: &str) -> Option<usize> {
    root_lists(doc).position(|children| children.iter().any(|n| subtree_contains(n, id)))
}

/// Walk the document tree and return a mutable reference to the node with
/// the given `id`, or `None` if not found. The lookup order is the one of
/// [`root_entries`].
///
/// Two-phase approach: shared scan first (to find the root list), then a
/// single targeted mutable borrow. This pattern avoids the borrow-checker
/// conflict that would arise if we tried to return a mutable reference from
/// within an `&mut`-iterating for loop.
pub(super) fn find_node_any_mut<'doc>(
    doc: &'doc mut zenith_core::Document,
    id: &str,
) -> Option<&'doc mut Node> {
    let index = root_index_of(doc, id)?;
    let children = root_lists_mut(doc).nth(index)?;
    find_in_children_any_mut(children, id)
}

/// Descend into a children slice and return a mutable reference to the node
/// with `id`. Returns `None` if the id is not present in this subtree.
///
/// Two-phase: shared scan to find the index, then exclusive borrow to act.
///
/// No recursion-depth guard (accepted v0 limit, consistent with
/// `reorder_in` and `subtree_contains`).
fn find_in_children_any_mut<'a>(children: &'a mut [Node], id: &str) -> Option<&'a mut Node> {
    // Phase 1: find the index and how to reach it.
    // `Direct(i)` — id matches children[i] itself.
    // `Descend(i)` — id lives somewhere inside the container at children[i].
    enum Hit {
        Direct(usize),
        Descend(usize),
    }

    let hit = children.iter().enumerate().find_map(|(i, node)| {
        if node.id() == Some(id) {
            return Some(Hit::Direct(i));
        }
        match node {
            Node::Frame(f) if f.children.iter().any(|c| subtree_contains(c, id)) => {
                Some(Hit::Descend(i))
            }
            Node::Group(g) if g.children.iter().any(|c| subtree_contains(c, id)) => {
                Some(Hit::Descend(i))
            }
            Node::Table(t)
                if t.rows.iter().any(|r| {
                    r.cells
                        .iter()
                        .any(|c| c.children.iter().any(|ch| subtree_contains(ch, id)))
                }) =>
            {
                Some(Hit::Descend(i))
            }
            Node::Unknown(u) if u.children.iter().any(|c| subtree_contains(c, id)) => {
                Some(Hit::Descend(i))
            }
            Node::Frame(_)
            | Node::Group(_)
            | Node::Table(_)
            | Node::Unknown(_)
            | Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Footnote(_)
            | Node::Toc(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_) => None,
        }
    });

    // Phase 2: take the exclusive borrow we deferred.
    match hit {
        None => None,
        Some(Hit::Direct(i)) => children.get_mut(i),
        Some(Hit::Descend(i)) => match children.get_mut(i) {
            Some(Node::Frame(f)) => find_in_children_any_mut(&mut f.children, id),
            Some(Node::Group(g)) => find_in_children_any_mut(&mut g.children, id),
            Some(Node::Table(t)) => {
                for row in &mut t.rows {
                    for cell in &mut row.cells {
                        if let Some(found) = find_in_children_any_mut(&mut cell.children, id) {
                            return Some(found);
                        }
                    }
                }
                None
            }
            Some(Node::Unknown(u)) => find_in_children_any_mut(&mut u.children, id),
            // unreachable: phase-1 confirmed a container at i
            Some(Node::Rect(_))
            | Some(Node::Ellipse(_))
            | Some(Node::Line(_))
            | Some(Node::Text(_))
            | Some(Node::Code(_))
            | Some(Node::Image(_))
            | Some(Node::Polygon(_))
            | Some(Node::Polyline(_))
            | Some(Node::Path(_))
            | Some(Node::Instance(_))
            | Some(Node::Field(_))
            | Some(Node::Footnote(_))
            | Some(Node::Toc(_))
            | Some(Node::Shape(_))
            | Some(Node::Connector(_))
            | Some(Node::Pattern(_))
            | Some(Node::Chart(_))
            | Some(Node::Light(_))
            | Some(Node::Mesh(_))
            | None => None,
        },
    }
}

/// Shared-borrow document walk: find a node with `id` on any page, master,
/// or component definition, in the order of [`root_entries`].
pub(crate) fn find_node_any_shared<'doc>(
    doc: &'doc zenith_core::Document,
    id: &str,
) -> Option<&'doc Node> {
    root_lists(doc).find_map(|children| find_node_shared(children, id))
}

/// Shared-borrow tree walk: find a node with `id` anywhere in `children`.
pub(super) fn find_node_shared<'a>(children: &'a [Node], id: &str) -> Option<&'a Node> {
    for node in children {
        if node.id() == Some(id) {
            return Some(node);
        }
        match node {
            Node::Frame(f) => {
                if let Some(found) = find_node_shared(&f.children, id) {
                    return Some(found);
                }
            }
            Node::Group(g) => {
                if let Some(found) = find_node_shared(&g.children, id) {
                    return Some(found);
                }
            }
            Node::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        if let Some(found) = find_node_shared(&cell.children, id) {
                            return Some(found);
                        }
                    }
                }
            }
            Node::Unknown(u) => {
                if let Some(found) = find_node_shared(&u.children, id) {
                    return Some(found);
                }
            }
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Footnote(_)
            | Node::Toc(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_) => {}
        }
    }
    None
}

/// Construct a [`Dimension`] with the `(px)` unit from a raw `f64` value.
pub(super) fn px(v: f64) -> Dimension {
    Dimension {
        value: v,
        unit: Unit::Px,
    }
}

// ── Utility ───────────────────────────────────────────────────────────────────

/// Clear `role="placeholder"`: the text of the node was replaced, so it no
/// longer holds sample content. Any other role stays.
pub(super) fn clear_placeholder(role: &mut Option<String>) {
    if role.as_deref() == Some(zenith_core::ast::PLACEHOLDER_ROLE) {
        *role = None;
    }
}

/// Append `id` to `affected` only if it is not already present.
/// Uses a linear scan to maintain deterministic first-seen insertion order
/// without HashMap (which has non-deterministic iteration).
pub(super) fn record_affected(id: &str, affected: &mut Vec<String>) {
    if !affected.iter().any(|s| s == id) {
        affected.push(id.to_owned());
    }
}
