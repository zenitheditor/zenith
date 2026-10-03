//! The child space of a container: the translation it applies to the
//! coordinates of its children.

use std::collections::BTreeMap;

use crate::ast::value::{Dimension, PropertyValue, dim_to_px};
use crate::tokens::ResolvedToken;

use super::super::common::Node;
use super::super::container::{FrameNode, GroupNode};
use super::super::special::InstanceNode;
use super::resolve::resolve_geometry_px;

impl Node {
    /// The px origin this node adds to the coordinates of its children.
    ///
    /// `Some((dx, dy))` for a container whose children are placed relative
    /// to it (`frame`, `group`, `instance`). `None` for every kind without
    /// positioned children. Every walker that accumulates a page-space offset
    /// reads the container translation here.
    pub fn child_space(&self, resolved: &BTreeMap<String, ResolvedToken>) -> Option<(f64, f64)> {
        match self {
            Node::Frame(f) => Some(f.child_space(resolved)),
            Node::Group(g) => Some(g.child_space(resolved)),
            Node::Instance(i) => Some(i.child_space()),
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Path(_)
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
            | Node::Unknown(_) => None,
        }
    }

    /// The origin of this node's children in a list at `origin`: `origin`
    /// plus [`Node::child_space`], unchanged for a node without a
    /// child space.
    pub fn child_origin(
        &self,
        origin: (f64, f64),
        resolved: &BTreeMap<String, ResolvedToken>,
    ) -> (f64, f64) {
        match self.child_space(resolved) {
            Some((sx, sy)) => (origin.0 + sx, origin.1 + sy),
            None => origin,
        }
    }
}

/// The px `x` / `y` of a container; an absent or unresolvable axis counts as 0.
fn origin_px(
    x: Option<&PropertyValue>,
    y: Option<&PropertyValue>,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> (f64, f64) {
    (
        resolve_geometry_px(x, resolved).unwrap_or(0.0),
        resolve_geometry_px(y, resolved).unwrap_or(0.0),
    )
}

impl FrameNode {
    /// The px origin a frame adds to its children: its `x` / `y`, so a child
    /// counts from the frame's top-left. An absent or unresolvable axis counts
    /// as 0.
    ///
    /// Scene compilation reads the resolved origin: auto-layout lowering
    /// writes the `x` / `y` of every layout frame it places or anchors before
    /// compile runs. A plain frame without a px `x` / `y` does not render.
    /// Validation on authored geometry reads the authored `x` / `y` only.
    pub fn child_space(&self, resolved: &BTreeMap<String, ResolvedToken>) -> (f64, f64) {
        origin_px(self.x.as_ref(), self.y.as_ref(), resolved)
    }
}

impl GroupNode {
    /// The px origin a group adds to its children: its `x` / `y`. An absent
    /// or unresolvable axis counts as 0.
    pub fn child_space(&self, resolved: &BTreeMap<String, ResolvedToken>) -> (f64, f64) {
        origin_px(self.x.as_ref(), self.y.as_ref(), resolved)
    }
}

impl InstanceNode {
    /// The px origin an instance adds to its expanded subtree: its `x` / `y`.
    /// An absent or non-px axis counts as 0.
    pub fn child_space(&self) -> (f64, f64) {
        let px = |d: Option<&Dimension>| d.and_then(|d| dim_to_px(d.value, &d.unit)).unwrap_or(0.0);
        (px(self.x.as_ref()), px(self.y.as_ref()))
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::Document;
    use crate::parse::{KdlAdapter, KdlSource};

    use super::*;

    fn page_children(body: &str) -> Vec<Node> {
        let src = format!(
            r##"zenith version=1 {{
  project id="p" name="P"
  tokens format="zenith-token-v1" {{}}
  styles {{}}
  document id="d" title="D" {{
    page id="pg" w=(px)400 h=(px)300 {{
      {body}
    }}
  }}
}}"##
        );
        let doc: Document = KdlAdapter.parse(src.as_bytes()).expect("parse");
        doc.body.pages.into_iter().next().expect("page").children
    }

    #[test]
    fn group_translates_by_origin() {
        let nodes = page_children(r#"group id="g" x=(px)10 y=(pt)30 { }"#);
        let r = BTreeMap::new();
        assert_eq!(nodes[0].child_space(&r), Some((10.0, 40.0)));
    }

    #[test]
    fn group_without_origin_translates_by_zero() {
        let nodes = page_children(r#"group id="g" { }"#);
        assert_eq!(nodes[0].child_space(&BTreeMap::new()), Some((0.0, 0.0)));
    }

    #[test]
    fn instance_translates_by_origin() {
        let nodes = page_children(r#"instance id="i" component="c" x=(px)5 y=(px)7"#);
        assert_eq!(nodes[0].child_space(&BTreeMap::new()), Some((5.0, 7.0)));
    }

    #[test]
    fn frame_translates_by_origin() {
        let nodes = page_children(r#"frame id="f" x=(px)10 y=(pt)30 w=(px)100 h=(px)100 { }"#);
        assert_eq!(nodes[0].child_space(&BTreeMap::new()), Some((10.0, 40.0)));
    }

    #[test]
    fn frame_without_origin_translates_by_zero() {
        let nodes = page_children(r#"frame id="f" layout="column" { }"#);
        assert_eq!(nodes[0].child_space(&BTreeMap::new()), Some((0.0, 0.0)));
    }

    #[test]
    fn leaves_keep_parent_space() {
        let nodes =
            page_children(r##"rect id="r" x=(px)1 y=(px)2 w=(px)3 h=(px)4 fill="#000000""##);
        assert_eq!(nodes[0].child_space(&BTreeMap::new()), None);
    }
}
