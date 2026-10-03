//! `Group` / `Ungroup` application, plus the common-parent finder and
//! ungroup splice helper they use.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, Document, GroupNode, Node, ResolvedToken, translate_node};

use super::super::layout::places_in_flow;
use super::super::space::{container_chain, parent_frame, resolved_tokens};
use super::super::{find_node_any_shared, record_affected};
use super::finders::find_container_children_mut;

/// Find which page directly contains (at the top level of `page.children`) ALL
/// of the ids in `node_ids`. Returns `(page_index, sorted_indices)` where
/// `sorted_indices` is the list of positions within `page.children` in
/// ascending order, or `None` if the ids are not all siblings on one page.
///
/// We walk each page's *direct* children only — a flat O(pages × ids) scan.
/// Nesting is handled by a second pass that descends into containers.
fn find_common_parent_children_mut<'doc>(
    doc: &'doc mut Document,
    node_ids: &[String],
) -> Option<&'doc mut Vec<Node>> {
    // Phase 1 (shared scan): find which page + container has ALL ids as direct
    // children.  We search each page's full subtree of containers.
    struct Hit {
        page_index: usize,
        /// If `None` the parent is the page itself; otherwise it's the container id.
        container_id: Option<String>,
    }

    let hit: Option<Hit> = 'outer: {
        for (pi, page) in doc.body.pages.iter().enumerate() {
            // Check if all are direct children of this page.
            if node_ids
                .iter()
                .all(|id| page.children.iter().any(|n| n.id() == Some(id.as_str())))
            {
                break 'outer Some(Hit {
                    page_index: pi,
                    container_id: None,
                });
            }
            // Walk containers within this page.
            if let Some(cid) = find_container_with_all_children(&page.children, node_ids) {
                break 'outer Some(Hit {
                    page_index: pi,
                    container_id: Some(cid),
                });
            }
        }
        None
    };

    let Hit {
        page_index,
        container_id,
    } = hit?;

    // Phase 2 (exclusive borrow): return a mutable ref to the right vec.
    match container_id {
        None => doc.body.pages.get_mut(page_index).map(|p| &mut p.children),
        Some(cid) => find_container_children_mut(doc, &cid),
    }
}

/// Walk `children` recursively and return the id of the first container whose
/// *direct* children include all ids in `node_ids`. Returns `None` if no such
/// container exists in this subtree.
fn find_container_with_all_children(children: &[Node], node_ids: &[String]) -> Option<String> {
    for node in children {
        let (container_id, grandchildren) = match node {
            Node::Frame(f) => (f.id.as_str(), f.children.as_slice()),
            Node::Group(g) => (g.id.as_str(), g.children.as_slice()),
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
            | Node::Table(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => continue,
        };
        if node_ids
            .iter()
            .all(|id| grandchildren.iter().any(|n| n.id() == Some(id.as_str())))
        {
            return Some(container_id.to_owned());
        }
        if let Some(found) = find_container_with_all_children(grandchildren, node_ids) {
            return Some(found);
        }
    }
    None
}

pub(in crate::engine) fn apply_group(
    node_ids: &[String],
    group_id: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    // Require at least one id.
    if node_ids.is_empty() {
        diagnostics.push(Diagnostic::error(
            "tx.invalid_parent",
            "group requires at least one node id".to_owned(),
            None,
            None,
        ));
        return;
    }

    // Phase 1: locate the common parent children vec (shared-then-exclusive
    // two-phase, handled inside find_common_parent_children_mut).
    let children = match find_common_parent_children_mut(doc, node_ids) {
        Some(c) => c,
        None => {
            diagnostics.push(Diagnostic::error(
                "tx.invalid_parent",
                "group requires all nodes to share a parent".to_owned(),
                None,
                None,
            ));
            return;
        }
    };

    // Phase 2: collect the indices of the named nodes within this children vec,
    // in ascending order so we can determine insert position and remove cleanly.
    let mut indices: Vec<usize> = node_ids
        .iter()
        .filter_map(|id| children.iter().position(|n| n.id() == Some(id.as_str())))
        .collect();

    // All ids must resolve (filter_map would silently drop missing ones).
    if indices.len() != node_ids.len() {
        diagnostics.push(Diagnostic::error(
            "tx.invalid_parent",
            "group requires all nodes to share a parent".to_owned(),
            None,
            None,
        ));
        return;
    }

    indices.sort_unstable();

    // Insert position = index of the first (lowest) member.
    // indices is non-empty: node_ids is non-empty and all ids resolved (guarded
    // by the length check above), so .first() will always be Some.
    let Some(&insert_at) = indices.first() else {
        return; // unreachable: guarded by node_ids.is_empty() check above
    };

    // Extract the nodes in their original relative order (lowest index first).
    // indices is already sorted ascending, so this produces source-order children.
    // All indices came from `.position()` on the same `children` slice and the
    // slice has not been mutated since — `.get()` returns Some for all of them.
    let group_children: Vec<Node> = indices
        .iter()
        .filter_map(|&i| children.get(i).cloned())
        .collect();

    // Remove from back to front to keep earlier indices stable.
    for &i in indices.iter().rev() {
        children.remove(i);
    }

    // `insert_at` is the lowest removed index, so no removal shifts it.
    let insert_at = insert_at.min(children.len());

    // Build the group node with all fields at defaults (None / empty).
    // x/y stay unset: the group origin is 0, so the children keep their
    // coordinates and their page position.
    let group_node = Node::Group(GroupNode {
        id: group_id.to_owned(),
        name: None,
        role: None,
        x: None,
        y: None,
        w: None,
        h: None,
        layout_item: Default::default(),
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        blend_mode: None,
        shadow: None,
        filter: None,
        mask: None,
        blur: None,
        style: None,
        semantic_role: None,
        intensity: None,
        layer_priority: None,
        symmetry_count: None,
        symmetry_cx: None,
        symmetry_cy: None,
        symmetry_start_angle: None,
        symmetry_mode: None,
        anchor: None,
        anchor_zone: None,
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        children: group_children,
        protected_regions: Vec::new(),
        editable_param_ids: Vec::new(),
        source_span: None,
        unknown_props: BTreeMap::new(),
    });

    children.insert(insert_at, group_node);
    record_affected(group_id, affected);
    // Post-validation catches group_id collision (id.duplicate).
}

pub(in crate::engine) fn apply_ungroup(
    group_id: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    // Phase 1 (shared scan): verify the node exists (on a page or a master)
    // and is a group, and compute the shift each child needs to keep its
    // page position.
    struct GroupInfo {
        /// Per child: the px shift into the group's parent space, if any.
        shifts: Vec<Option<(f64, f64)>>,
        /// A child leaves the group but the group origin does not resolve.
        unresolved: bool,
    }

    let resolved = resolved_tokens(doc);
    let info: Option<Result<GroupInfo, &'static str>> =
        find_node_any_shared(doc, group_id).map(|node| match node {
            Node::Group(g) => {
                let parent = parent_frame(doc, group_id);
                // The group's own link ends its container chain.
                let offset = container_chain(doc, group_id, &resolved)
                    .and_then(|chain| chain.last().and_then(|link| link.offset));
                let mut unresolved = false;
                let shifts = g
                    .children
                    .iter()
                    .map(|child| {
                        // A flow slot of the new parent frame: layout places
                        // the child.
                        if parent.is_some_and(|f| places_in_flow(f, child)) {
                            return None;
                        }
                        match offset {
                            Some((dx, dy)) if dx == 0.0 && dy == 0.0 => None,
                            Some(shift) => Some(shift),
                            None => {
                                unresolved = true;
                                None
                            }
                        }
                    })
                    .collect();
                Ok(GroupInfo { shifts, unresolved })
            }
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Frame(_)
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
            | Node::Unknown(_) => Err("not a group"),
        });

    let info = match info {
        None => {
            diagnostics.push(Diagnostic::error(
                "tx.unknown_node",
                format!("node {:?} not found in document", group_id),
                None,
                Some(group_id.to_owned()),
            ));
            return;
        }
        Some(Err(reason)) => {
            diagnostics.push(Diagnostic::error(
                "tx.unsupported_property",
                format!("ungroup: {:?} is {}", group_id, reason),
                None,
                Some(group_id.to_owned()),
            ));
            return;
        }
        Some(Ok(info)) => info,
    };

    if info.unresolved {
        diagnostics.push(Diagnostic::advisory(
            "tx.coordinate_unresolved",
            format!(
                "ungroup: children of group {group_id:?} keep their x/y unchanged: the \
                 group's px origin does not resolve, or a layout frame places the group; \
                 check the children's positions and set x/y with set_geometry"
            ),
            None,
            Some(group_id.to_owned()),
        ));
    }

    // Phase 2 (exclusive borrow): remove the group from its page or master
    // and splice its children in place.
    let splice = Splice {
        group_id,
        shifts: &info.shifts,
        resolved: &resolved,
    };
    let lists = doc
        .body
        .pages
        .iter_mut()
        .map(|p| &mut p.children)
        .chain(doc.masters.iter_mut().map(|m| &mut m.children));
    for children in lists {
        if splice_ungroup(children, &splice) {
            record_affected(group_id, affected);
            return;
        }
    }
}

/// What [`splice_ungroup`] removes and how it moves each child.
struct Splice<'a> {
    group_id: &'a str,
    /// Per child: the px shift into the group's parent space, if any.
    shifts: &'a [Option<(f64, f64)>],
    resolved: &'a BTreeMap<String, ResolvedToken>,
}

/// Walk `children` to find the group with `group_id`, remove it, and insert
/// its children at the same index. Returns `true` if the group was found and
/// spliced, `false` otherwise (to continue recursion).
fn splice_ungroup(children: &mut Vec<Node>, splice: &Splice<'_>) -> bool {
    let group_id = splice.group_id;
    // Check direct children first.
    if let Some(i) = children.iter().position(|n| n.id() == Some(group_id)) {
        // We confirmed it's a group in the shared-scan phase; use .get() for
        // checked access — the match arm handles the unreachable-but-safe case.
        let group_children = match children.get(i) {
            Some(Node::Group(g)) => g.children.clone(),
            // unreachable under normal flow
            Some(Node::Rect(_))
            | Some(Node::Ellipse(_))
            | Some(Node::Line(_))
            | Some(Node::Text(_))
            | Some(Node::Code(_))
            | Some(Node::Frame(_))
            | Some(Node::Image(_))
            | Some(Node::Polygon(_))
            | Some(Node::Polyline(_))
            | Some(Node::Path(_))
            | Some(Node::Instance(_))
            | Some(Node::Field(_))
            | Some(Node::Footnote(_))
            | Some(Node::Toc(_))
            | Some(Node::Table(_))
            | Some(Node::Shape(_))
            | Some(Node::Connector(_))
            | Some(Node::Pattern(_))
            | Some(Node::Chart(_))
            | Some(Node::Light(_))
            | Some(Node::Mesh(_))
            | Some(Node::Unknown(_))
            | None => return false,
        };
        children.remove(i);
        // Insert the group's children at the same position, in order.
        let moved = group_children
            .into_iter()
            .enumerate()
            .map(|(offset, mut child)| {
                if let Some(Some((dx, dy))) = splice.shifts.get(offset) {
                    translate_node(&mut child, *dx, *dy, splice.resolved);
                }
                child
            });
        children.splice(i..i, moved);
        return true;
    }
    // Descend into nested containers, table cells, and unknown nodes.
    for child in children.iter_mut() {
        let found = match child {
            Node::Frame(f) => splice_ungroup(&mut f.children, splice),
            Node::Group(g) => splice_ungroup(&mut g.children, splice),
            Node::Table(t) => t
                .rows
                .iter_mut()
                .flat_map(|r| r.cells.iter_mut())
                .any(|c| splice_ungroup(&mut c.children, splice)),
            Node::Unknown(u) => splice_ungroup(&mut u.children, splice),
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
        };
        if found {
            return true;
        }
    }
    false
}
