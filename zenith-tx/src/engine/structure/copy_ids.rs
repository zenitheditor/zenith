//! Pick the id suffix for a `duplicate_node` subtree copy.

use std::collections::{BTreeMap, BTreeSet};

use zenith_core::{Document, Node};

/// The suffix `duplicate_node` appends to every descendant id of the copy.
///
/// The base suffix is what `new_id` adds to `node_id` (`box` to `box-copy`
/// gives `-copy`). A `new_id` that does not start with `node_id` gives
/// `.{new_id}`. When a descendant id plus that suffix already exists in the
/// document, the suffix gets a number (`-copy2`, `-copy3`, ...). Duplicating a
/// node twice therefore always gives distinct descendant ids.
///
/// Returns `None` only when no free suffix exists, which cannot happen for a
/// finite document.
pub(super) fn pick_suffix(
    doc: &Document,
    source: &Node,
    node_id: &str,
    new_id: &str,
) -> Option<String> {
    let base = match new_id.strip_prefix(node_id) {
        Some(rest) if !rest.is_empty() => rest.to_owned(),
        Some(_) | None => format!(".{new_id}"),
    };
    let mut descendants = Vec::new();
    collect_descendant_ids(source, &mut descendants);
    if descendants.is_empty() {
        return Some(base);
    }
    let mut taken = BTreeSet::new();
    collect_doc_ids(doc, &mut taken);
    taken.insert(new_id.to_owned());
    (1..=taken.len().saturating_add(1)).find_map(|n| {
        let suffix = if n == 1 {
            base.clone()
        } else {
            format!("{base}{n}")
        };
        descendants
            .iter()
            .all(|id| !taken.contains(&format!("{id}{suffix}")))
            .then_some(suffix)
    })
}

/// The node lists directly under `node`, including table cells.
fn child_lists(node: &Node) -> Vec<&[Node]> {
    match node {
        Node::Frame(f) => vec![f.children.as_slice()],
        Node::Group(g) => vec![g.children.as_slice()],
        Node::Table(t) => t
            .rows
            .iter()
            .flat_map(|row| row.cells.iter().map(|cell| cell.children.as_slice()))
            .collect(),
        Node::Unknown(u) => vec![u.children.as_slice()],
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
        | Node::Mesh(_) => Vec::new(),
    }
}

fn collect_descendant_ids(node: &Node, out: &mut Vec<String>) {
    for list in child_lists(node) {
        for child in list {
            if let Some(id) = child.id() {
                out.push(id.to_owned());
            }
            collect_descendant_ids(child, out);
        }
    }
}

fn collect_node_ids(nodes: &[Node], out: &mut BTreeSet<String>) {
    for node in nodes {
        if let Some(id) = node.id() {
            out.insert(id.to_owned());
        }
        for list in child_lists(node) {
            collect_node_ids(list, out);
        }
    }
}

/// Every page, node, safe-zone, and fold id in the document.
fn collect_doc_ids(doc: &Document, out: &mut BTreeSet<String>) {
    for page in &doc.body.pages {
        out.insert(page.id.clone());
        out.extend(page.safe_zones.iter().map(|z| z.id.clone()));
        out.extend(page.folds.iter().map(|f| f.id.clone()));
        collect_node_ids(&page.children, out);
    }
    for master in &doc.masters {
        collect_node_ids(&master.children, out);
    }
}

/// Old id to new id for the source node and every descendant of its subtree.
/// The source maps to `new_id`, each descendant to its id plus `suffix`.
pub(super) fn subtree_id_map(
    source: &Node,
    node_id: &str,
    new_id: &str,
    suffix: &str,
) -> BTreeMap<String, String> {
    let mut descendants = Vec::new();
    collect_descendant_ids(source, &mut descendants);
    let mut map: BTreeMap<String, String> = descendants
        .into_iter()
        .map(|id| {
            let new = format!("{id}{suffix}");
            (id, new)
        })
        .collect();
    map.insert(node_id.to_owned(), new_id.to_owned());
    map
}
