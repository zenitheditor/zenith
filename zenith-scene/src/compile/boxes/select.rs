//! [`selectable_id`]: the authored node a compiled box id selects.

use zenith_core::{Document, Node};

/// The id of the authored node a user selects for the compiled box id
/// `raw_id` on page `page_index` of `doc`, the document the boxes came from.
///
/// Rules, first match wins:
/// 1. A node of the page (any depth, table cells included) with id `raw_id`
///    selects itself.
/// 2. A master projection `<page-id>/<rest>` selects the node of the page's
///    master that `rest` resolves to by rules 1 and 3. The master node is
///    the authored node: an edit to it changes every page that uses it.
/// 3. Expanded content selects the node that expanded it: the page node
///    with the longest id `owner` such that `raw_id` starts with
///    `<owner>/`. Instance content `<instance-id>/<id>` selects the
///    instance. A pattern motif `<pattern-id>/<index>/<motif-id>` selects
///    the pattern.
///
/// `None` when nothing matches: a guide node (`role="guide"`) or a node
/// inside one, an id from another page, or an out-of-range page.
#[must_use]
pub fn selectable_id<'d>(doc: &'d Document, page_index: usize, raw_id: &str) -> Option<&'d str> {
    let page = doc.body.pages.get(page_index)?;
    if let Some(id) = exact(&page.children, raw_id) {
        return Some(id);
    }
    let projected = page.master.as_deref().and_then(|master_id| {
        let rest = raw_id.strip_prefix(page.id.as_str())?.strip_prefix('/')?;
        let master = doc.masters.iter().find(|m| m.id == master_id)?;
        exact(&master.children, rest).or_else(|| owner(&master.children, rest))
    });
    projected.or_else(|| owner(&page.children, raw_id))
}

/// The id of the node in `nodes` named `raw`.
fn exact<'d>(nodes: &'d [Node], raw: &str) -> Option<&'d str> {
    let mut found = None;
    walk(nodes, &mut |id| {
        if found.is_none() && id == raw {
            found = Some(id);
        }
    });
    found
}

/// The longest id `owner` in `nodes` such that `raw` starts with
/// `<owner>/`.
fn owner<'d>(nodes: &'d [Node], raw: &str) -> Option<&'d str> {
    let mut best: Option<&'d str> = None;
    walk(nodes, &mut |id| {
        let owns = raw
            .strip_prefix(id)
            .is_some_and(|rest| rest.starts_with('/'));
        if owns && best.is_none_or(|b| id.len() > b.len()) {
            best = Some(id);
        }
    });
    best
}

/// Visit the id of every node in `nodes` and their descendants, in source
/// order. Guide nodes and their subtrees are left out: they never compile.
fn walk<'d>(nodes: &'d [Node], visit: &mut impl FnMut(&'d str)) {
    for node in nodes {
        if node.role() == Some("guide") {
            continue;
        }
        if let Some(id) = node.id() {
            visit(id);
        }
        match node {
            Node::Frame(n) => walk(&n.children, visit),
            Node::Group(n) => walk(&n.children, visit),
            Node::Unknown(n) => walk(&n.children, visit),
            Node::Table(t) => {
                for cell in t.rows.iter().flat_map(|r| &r.cells) {
                    walk(&cell.children, visit);
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
            | Node::Toc(_)
            | Node::Footnote(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_) => {}
        }
    }
}
