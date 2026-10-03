//! The id tree of a document for `zenith tx`: parent, children, and
//! descendants of every node with an id.

use std::collections::BTreeMap;

use zenith_core::{Document, Node};

/// Parent and child ids of every node with an id. A page or master id is
/// the parent of its top-level nodes. A node without an id passes its
/// children to its nearest ancestor with an id.
#[derive(Default)]
pub(super) struct Tree {
    pub(super) parent: BTreeMap<String, String>,
    children: BTreeMap<String, Vec<String>>,
}

impl Tree {
    pub(super) fn of(doc: &Document) -> Self {
        let mut tree = Tree::default();
        for page in &doc.body.pages {
            tree.add(&page.id, &page.children);
        }
        for master in &doc.masters {
            tree.add(&master.id, &master.children);
        }
        tree
    }

    fn add(&mut self, parent: &str, nodes: &[Node]) {
        for node in nodes {
            let key = match node.id() {
                Some(id) => {
                    self.parent.insert(id.to_owned(), parent.to_owned());
                    self.children
                        .entry(parent.to_owned())
                        .or_default()
                        .push(id.to_owned());
                    id
                }
                None => parent,
            };
            match node {
                Node::Frame(f) => self.add(key, &f.children),
                Node::Group(g) => self.add(key, &g.children),
                Node::Unknown(u) => self.add(key, &u.children),
                Node::Table(t) => {
                    for cell in t.rows.iter().flat_map(|r| r.cells.iter()) {
                        self.add(key, &cell.children);
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
    }

    /// The other children of `id`'s parent.
    pub(super) fn siblings<'t>(&'t self, id: &'t str) -> impl Iterator<Item = &'t str> + 't {
        self.parent
            .get(id)
            .and_then(|p| self.children.get(p))
            .into_iter()
            .flatten()
            .map(String::as_str)
            .filter(move |s| *s != id)
    }

    /// Every descendant id of `id`, depth first in document order.
    pub(super) fn descendants<'t>(&'t self, id: &str) -> Vec<&'t str> {
        let mut out = Vec::new();
        let mut stack: Vec<&str> = self
            .children
            .get(id)
            .into_iter()
            .flatten()
            .rev()
            .map(String::as_str)
            .collect();
        while let Some(next) = stack.pop() {
            out.push(next);
            if let Some(kids) = self.children.get(next) {
                stack.extend(kids.iter().rev().map(String::as_str));
            }
        }
        out
    }

    /// The ancestor ids of `id`, nearest first (pages and masters included).
    pub(super) fn ancestors<'t>(&'t self, id: &'t str) -> impl Iterator<Item = &'t str> + 't {
        std::iter::successors(self.parent.get(id).map(String::as_str), move |p| {
            self.parent.get(*p).map(String::as_str)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource};

    const DOC: &str = r##"zenith version=1 {
  project id="proj" name="T"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc" title="T" {
    page id="pg" w=(px)100 h=(px)100 {
      frame id="f" x=(px)0 y=(px)0 w=(px)50 h=(px)50 {
        rect id="f.a" x=(px)0 y=(px)0 w=(px)5 h=(px)5
        group id="f.g" {
          rect id="f.g.a" x=(px)0 y=(px)0 w=(px)5 h=(px)5
        }
        rect id="f.b" x=(px)0 y=(px)0 w=(px)5 h=(px)5
      }
    }
  }
}"##;

    #[test]
    fn descendants_are_depth_first_in_document_order() {
        let doc = KdlAdapter.parse(DOC.as_bytes()).expect("parses");
        let tree = Tree::of(&doc);
        assert_eq!(tree.descendants("f"), ["f.a", "f.g", "f.g.a", "f.b"]);
        assert!(tree.descendants("f.a").is_empty());
        let up: Vec<&str> = tree.ancestors("f.g.a").collect();
        assert_eq!(up, ["f.g", "f", "pg"]);
    }
}
