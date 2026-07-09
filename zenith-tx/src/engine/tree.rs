//! Shared tree-walk helpers and small utilities used across the engine.
//!
//! These functions traverse the document node tree (shared and mutable),
//! extract per-node identity/kind, and record affected ids. They are pure and
//! contain no per-op business logic.

use zenith_core::{Dimension, Node, Unit};

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

/// Walk the document tree and return a mutable reference to the node with
/// the given `id`, or `None` if not found.
///
/// Two-phase approach: shared scan first (to find the page index), then a
/// single targeted mutable borrow. This pattern avoids the borrow-checker
/// conflict that would arise if we tried to return a mutable reference from
/// within an `&mut`-iterating for loop.
pub(super) fn find_node_any_mut<'doc>(
    doc: &'doc mut zenith_core::Document,
    id: &str,
) -> Option<&'doc mut Node> {
    // Phase 1: find which page or master hosts the id (shared borrow only).
    enum Host {
        Page(usize),
        Master(usize),
    }
    let host = doc
        .body
        .pages
        .iter()
        .enumerate()
        .find_map(|(pi, page)| {
            page.children
                .iter()
                .any(|n| subtree_contains(n, id))
                .then_some(Host::Page(pi))
        })
        .or_else(|| {
            doc.masters.iter().enumerate().find_map(|(mi, master)| {
                master
                    .children
                    .iter()
                    .any(|n| subtree_contains(n, id))
                    .then_some(Host::Master(mi))
            })
        });

    // Phase 2: act on the found host with an exclusive borrow.
    match host {
        None => None,
        Some(Host::Page(pi)) => match doc.body.pages.get_mut(pi) {
            None => None,
            Some(page) => find_in_children_any_mut(&mut page.children, id),
        },
        Some(Host::Master(mi)) => match doc.masters.get_mut(mi) {
            None => None,
            Some(master) => find_in_children_any_mut(&mut master.children, id),
        },
    }
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

/// Shared-borrow document walk: find a node with `id` on any page or master.
pub(super) fn find_node_any_shared<'doc>(
    doc: &'doc zenith_core::Document,
    id: &str,
) -> Option<&'doc Node> {
    for page in &doc.body.pages {
        if let Some(found) = find_node_shared(&page.children, id) {
            return Some(found);
        }
    }
    for master in &doc.masters {
        if let Some(found) = find_node_shared(&master.children, id) {
            return Some(found);
        }
    }
    None
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

/// Append `id` to `affected` only if it is not already present.
/// Uses a linear scan to maintain deterministic first-seen insertion order
/// without HashMap (which has non-deterministic iteration).
pub(super) fn record_affected(id: &str, affected: &mut Vec<String>) {
    if !affected.iter().any(|s| s == id) {
        affected.push(id.to_owned());
    }
}
