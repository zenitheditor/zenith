//! Editing metadata accessors on [`Node`]: display `name`, `locked`, and own
//! `rotate`.
//!
//! Editors read these for every node kind. One exhaustive match per field
//! keeps a new variant a compile error here only.

use super::Node;
use crate::ast::value::Dimension;

impl Node {
    /// Authored display `name`. `None` when omitted or the kind has no
    /// `name` (`unknown`).
    pub fn name(&self) -> Option<&str> {
        match self {
            Node::Rect(n) => n.name.as_deref(),
            Node::Ellipse(n) => n.name.as_deref(),
            Node::Line(n) => n.name.as_deref(),
            Node::Text(n) => n.name.as_deref(),
            Node::Code(n) => n.name.as_deref(),
            Node::Frame(n) => n.name.as_deref(),
            Node::Group(n) => n.name.as_deref(),
            Node::Image(n) => n.name.as_deref(),
            Node::Polygon(n) => n.name.as_deref(),
            Node::Polyline(n) => n.name.as_deref(),
            Node::Path(n) => n.name.as_deref(),
            Node::Instance(n) => n.name.as_deref(),
            Node::Field(n) => n.name.as_deref(),
            Node::Toc(n) => n.name.as_deref(),
            Node::Footnote(n) => n.name.as_deref(),
            Node::Table(n) => n.name.as_deref(),
            Node::Shape(n) => n.name.as_deref(),
            Node::Connector(n) => n.name.as_deref(),
            Node::Pattern(n) => n.name.as_deref(),
            Node::Chart(n) => n.name.as_deref(),
            Node::Light(n) => n.name.as_deref(),
            Node::Mesh(n) => n.name.as_deref(),
            Node::Unknown(_) => None,
        }
    }

    /// Authored `locked` flag. `None` when omitted or the kind has no
    /// `locked` (`footnote`, `unknown`).
    pub fn locked(&self) -> Option<bool> {
        match self {
            Node::Rect(n) => n.locked,
            Node::Ellipse(n) => n.locked,
            Node::Line(n) => n.locked,
            Node::Text(n) => n.locked,
            Node::Code(n) => n.locked,
            Node::Frame(n) => n.locked,
            Node::Group(n) => n.locked,
            Node::Image(n) => n.locked,
            Node::Polygon(n) => n.locked,
            Node::Polyline(n) => n.locked,
            Node::Path(n) => n.locked,
            Node::Instance(n) => n.locked,
            Node::Field(n) => n.locked,
            Node::Toc(n) => n.locked,
            Node::Table(n) => n.locked,
            Node::Shape(n) => n.locked,
            Node::Connector(n) => n.locked,
            Node::Pattern(n) => n.locked,
            Node::Chart(n) => n.locked,
            Node::Light(n) => n.locked,
            Node::Mesh(n) => n.locked,
            Node::Footnote(_) | Node::Unknown(_) => None,
        }
    }

    /// `true` when the node itself has `locked=#true`.
    pub fn is_locked(&self) -> bool {
        self.locked() == Some(true)
    }

    /// Authored own `rotate`. `None` when omitted or the kind has no
    /// `rotate` (line, instance, field, toc, footnote, light, mesh,
    /// unknown).
    pub fn rotate(&self) -> Option<&Dimension> {
        match self {
            Node::Rect(n) => n.rotate.as_ref(),
            Node::Ellipse(n) => n.rotate.as_ref(),
            Node::Text(n) => n.rotate.as_ref(),
            Node::Code(n) => n.rotate.as_ref(),
            Node::Frame(n) => n.rotate.as_ref(),
            Node::Group(n) => n.rotate.as_ref(),
            Node::Image(n) => n.rotate.as_ref(),
            Node::Polygon(n) => n.rotate.as_ref(),
            Node::Polyline(n) => n.rotate.as_ref(),
            Node::Path(n) => n.rotate.as_ref(),
            Node::Table(n) => n.rotate.as_ref(),
            Node::Shape(n) => n.rotate.as_ref(),
            Node::Connector(n) => n.rotate.as_ref(),
            Node::Pattern(n) => n.rotate.as_ref(),
            Node::Chart(n) => n.rotate.as_ref(),
            Node::Line(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Toc(_)
            | Node::Footnote(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => None,
        }
    }

    /// `true` when the kind carries a `rotate` attribute, set or not.
    pub fn takes_rotate(&self) -> bool {
        match self {
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Frame(_)
            | Node::Group(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
            | Node::Table(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_) => true,
            Node::Line(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Toc(_)
            | Node::Footnote(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::Node;
    use crate::parse::{KdlAdapter, KdlSource};

    fn children(src: &str) -> Vec<Node> {
        let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
        doc.body.pages.into_iter().next().expect("page").children
    }

    #[test]
    fn name_locked_rotate_read_back() {
        let nodes = children(
            r#"
zenith version=1 {
  document id="d" {
    page id="pg" w=(px)100 h=(px)100 {
      rect id="r" name="Card" locked=#true rotate=(deg)15
      line id="l" x1=(px)0 y1=(px)0 x2=(px)5 y2=(px)5
      sparkle
    }
  }
}
"#,
        );
        let [r, l, u] = nodes.as_slice() else {
            panic!("three nodes");
        };
        assert_eq!(r.name(), Some("Card"));
        assert!(r.is_locked());
        assert_eq!(r.rotate().map(|d| d.value), Some(15.0));
        assert!(r.takes_rotate());
        assert_eq!(l.name(), None);
        assert!(!l.is_locked());
        assert!(!l.takes_rotate());
        assert_eq!(l.rotate(), None);
        assert_eq!(u.locked(), None);
        assert_eq!(u.name(), None);
    }
}
