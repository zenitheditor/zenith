//! Node lookup over a parsed document: where a node sits (its page or
//! master, its ancestors, its siblings) and the ids in use.

use std::collections::BTreeSet;

use zenith_core::{Document, Node};

/// The top-level list that holds a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Root {
    /// The 0-based page index.
    Page(usize),
    /// The 0-based master index.
    Master(usize),
}

/// A node and where it sits.
pub(crate) struct Located<'d> {
    pub(crate) node: &'d Node,
    /// Containers from the page (or master) down to the parent, outermost
    /// first.
    pub(crate) ancestors: Vec<&'d Node>,
    pub(crate) root: Root,
    /// The child list that holds the node.
    pub(crate) siblings: &'d [Node],
}

impl<'d> Located<'d> {
    /// The direct parent node, `None` at the top of a page or master.
    pub(crate) fn parent(&self) -> Option<&'d Node> {
        self.ancestors.last().copied()
    }

    /// The id of the container that holds the node: the parent node, else
    /// the page or master.
    pub(crate) fn container_id(&self, doc: &'d Document) -> Option<&'d str> {
        match self.parent() {
            Some(parent) => parent.id(),
            None => match self.root {
                Root::Page(i) => doc.body.pages.get(i).map(|p| p.id.as_str()),
                Root::Master(i) => doc.masters.get(i).map(|m| m.id.as_str()),
            },
        }
    }

    /// The nearest node, itself first, then its ancestors from the parent
    /// up, for which `test` holds.
    pub(crate) fn self_or_ancestor(&self, test: impl Fn(&Node) -> bool) -> Option<&'d Node> {
        std::iter::once(self.node)
            .chain(self.ancestors.iter().rev().copied())
            .find(|n| test(n))
    }
}

/// The child lists of `node`: the children of a frame, group, or unknown
/// node, and the children of every table cell.
pub(crate) fn child_lists(node: &Node) -> Vec<&[Node]> {
    match node {
        Node::Frame(n) => vec![n.children.as_slice()],
        Node::Group(n) => vec![n.children.as_slice()],
        Node::Unknown(n) => vec![n.children.as_slice()],
        Node::Table(t) => t
            .rows
            .iter()
            .flat_map(|r| &r.cells)
            .map(|c| c.children.as_slice())
            .collect(),
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
        | Node::Mesh(_) => Vec::new(),
    }
}

/// The node with id `id` in `doc`'s pages, then its masters.
pub(crate) fn locate<'d>(doc: &'d Document, id: &str) -> Option<Located<'d>> {
    let pages = doc
        .body
        .pages
        .iter()
        .enumerate()
        .map(|(i, p)| (Root::Page(i), p.children.as_slice()));
    let masters = doc
        .masters
        .iter()
        .enumerate()
        .map(|(i, m)| (Root::Master(i), m.children.as_slice()));
    for (root, children) in pages.chain(masters) {
        let mut ancestors = Vec::new();
        if let Some(found) = search(children, id, root, &mut ancestors) {
            return Some(found);
        }
    }
    None
}

fn search<'d>(
    nodes: &'d [Node],
    id: &str,
    root: Root,
    ancestors: &mut Vec<&'d Node>,
) -> Option<Located<'d>> {
    for node in nodes {
        if node.id() == Some(id) {
            return Some(Located {
                node,
                ancestors: ancestors.clone(),
                root,
                siblings: nodes,
            });
        }
        ancestors.push(node);
        for list in child_lists(node) {
            if let Some(found) = search(list, id, root, ancestors) {
                return Some(found);
            }
        }
        ancestors.pop();
    }
    None
}

/// `true` when some node has id `id`.
pub(crate) fn exists(doc: &Document, id: &str) -> bool {
    locate(doc, id).is_some()
}

/// Every id in use: nodes (pages and masters, any depth), pages, masters,
/// and components.
pub(crate) fn all_ids(doc: &Document) -> BTreeSet<&str> {
    let mut ids = BTreeSet::new();
    for page in &doc.body.pages {
        ids.insert(page.id.as_str());
        collect(&page.children, &mut ids);
    }
    for master in &doc.masters {
        ids.insert(master.id.as_str());
        collect(&master.children, &mut ids);
    }
    for component in &doc.components {
        ids.insert(component.id.as_str());
        collect(&component.children, &mut ids);
    }
    ids
}

fn collect<'d>(nodes: &'d [Node], ids: &mut BTreeSet<&'d str>) {
    for node in nodes {
        if let Some(id) = node.id() {
            ids.insert(id);
        }
        for list in child_lists(node) {
            collect(list, ids);
        }
    }
}

/// `base` when free, else `base-2`, `base-3`, … the first free one.
pub(crate) fn unique_id(base: &str, taken: &BTreeSet<&str>) -> String {
    if !taken.contains(base) {
        return base.to_owned();
    }
    (2_u64..)
        .map(|n| format!("{base}{n}"))
        .find(|candidate| !taken.contains(candidate.as_str()))
        .unwrap_or_else(|| base.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource};

    const SRC: &str = r#"
zenith version=1 {
  document id="d" {
    page id="pg" w=(px)100 h=(px)100 {
      group id="g" {
        rect id="a" x=(px)0 y=(px)0 w=(px)5 h=(px)5
        rect id="b" x=(px)0 y=(px)0 w=(px)5 h=(px)5
      }
    }
  }
  masters {
    master id="m" {
      rect id="chrome" x=(px)0 y=(px)0 w=(px)5 h=(px)5
    }
  }
}
"#;

    #[test]
    fn locates_with_ancestors_and_root() {
        let doc = KdlAdapter.parse(SRC.as_bytes()).expect("parse");
        let b = locate(&doc, "b").expect("b");
        assert_eq!(b.root, Root::Page(0));
        assert_eq!(b.siblings.len(), 2);
        assert_eq!(b.parent().and_then(Node::id), Some("g"));
        assert_eq!(b.container_id(&doc), Some("g"));
        let g = locate(&doc, "g").expect("g");
        assert_eq!(g.container_id(&doc), Some("pg"));
        let chrome = locate(&doc, "chrome").expect("chrome");
        assert_eq!(chrome.root, Root::Master(0));
        assert!(locate(&doc, "zz").is_none());
    }

    #[test]
    fn unique_ids_skip_taken_ones() {
        let doc = KdlAdapter.parse(SRC.as_bytes()).expect("parse");
        let ids = all_ids(&doc);
        assert!(ids.contains("chrome") && ids.contains("pg") && ids.contains("m"));
        assert_eq!(unique_id("a", &ids), "a2");
        assert_eq!(unique_id("group", &ids), "group");
    }
}
