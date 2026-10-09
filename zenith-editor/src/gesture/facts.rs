//! What a box node's authored attributes allow a gesture to do: each
//! `x` / `y` / `w` / `h` classified, its anchor, and its layout flow.

use zenith_core::{AnchorEdge, Document, Node, PropertyValue, parse_anchor_edge};
use zenith_tx::{FlowPlacement, layout_flow};

/// One of a box's four attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Axis {
    X,
    Y,
    W,
    H,
}

impl Axis {
    /// The attribute name.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Axis::X => "x",
            Axis::Y => "y",
            Axis::W => "w",
            Axis::H => "h",
        }
    }

    /// `true` for `w` and `h`.
    pub(crate) fn is_size(self) -> bool {
        match self {
            Axis::X | Axis::Y => false,
            Axis::W | Axis::H => true,
        }
    }

    fn slot(self) -> usize {
        match self {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::W => 2,
            Axis::H => 3,
        }
    }
}

/// What one attribute holds, as far as a gesture cares.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AxisValue {
    /// A px or pt value: `nudge_geometry` offsets it in its unit.
    Length,
    /// A token reference: offset only with `detach`.
    Token(String),
    /// Not written.
    Absent,
    /// A `hug` / `fill` size keyword.
    Keyword(&'static str),
    /// A value with no px conversion (pct, deg, literal, data ref).
    Unresolved(String),
}

/// How an anchor places the node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Anchoring {
    /// No anchor attribute.
    None,
    /// `anchor-sibling` with `anchor-edge`: the gap moves the node along
    /// `main`.
    Edge { main: Axis },
    /// Any other anchor (`anchor`, `anchor-zone`, `anchor-parent`, or a
    /// sibling without an edge).
    Other,
}

/// The gesture facts of a box node.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BoxFacts {
    values: [AxisValue; 4],
    pub(crate) anchoring: Anchoring,
    /// An absent `x` / `y` counts as px 0 (group, instance).
    pub(crate) origin_defaults: bool,
    /// The layout frame that places the node in flow.
    pub(crate) flow: Option<FlowPlacement>,
}

impl BoxFacts {
    /// What `axis` holds.
    pub(crate) fn value(&self, axis: Axis) -> &AxisValue {
        self.values.get(axis.slot()).unwrap_or(&AxisValue::Absent)
    }

    /// `true` when an anchor supplies `axis` (it is absent and anchored).
    pub(crate) fn anchor_supplies(&self, axis: Axis) -> bool {
        !axis.is_size()
            && *self.value(axis) == AxisValue::Absent
            && self.anchoring != Anchoring::None
    }
}

/// The gesture facts of `node` (id `id` in `doc`), or `None` for a kind
/// without a box.
pub(crate) fn box_facts(doc: &Document, node: &Node, id: &str) -> Option<BoxFacts> {
    let (values, anchoring) = if let Node::Instance(i) = node {
        let v = |d: Option<&zenith_core::Dimension>| match d {
            Some(d) => classify(&PropertyValue::Dimension(d.clone())),
            None => AxisValue::Absent,
        };
        (
            [
                v(i.x.as_ref()),
                v(i.y.as_ref()),
                v(i.w.as_ref()),
                v(i.h.as_ref()),
            ],
            Anchoring::None,
        )
    } else {
        let view = node.box_view()?;
        let v = |p: Option<&PropertyValue>| p.map_or(AxisValue::Absent, classify);
        let anchoring = if view.anchored {
            let parts = node.anchor_view().map(|a| {
                (
                    a.anchor_sibling.is_some(),
                    a.anchor_edge.and_then(parse_anchor_edge),
                )
            });
            match parts {
                Some((true, Some(AnchorEdge::Above | AnchorEdge::Below))) => {
                    Anchoring::Edge { main: Axis::Y }
                }
                Some((true, Some(AnchorEdge::Before | AnchorEdge::After))) => {
                    Anchoring::Edge { main: Axis::X }
                }
                Some(
                    (true, None)
                    | (
                        false,
                        Some(
                            AnchorEdge::Above
                            | AnchorEdge::Below
                            | AnchorEdge::Before
                            | AnchorEdge::After,
                        )
                        | None,
                    ),
                )
                | None => Anchoring::Other,
            }
        } else {
            Anchoring::None
        };
        ([v(view.x), v(view.y), v(view.w), v(view.h)], anchoring)
    };
    let mut values = values;
    if let Some(item) = node.layout_item() {
        for (slot, keyword) in [(2, item.w_keyword), (3, item.h_keyword)] {
            if let (Some(k), Some(value)) = (keyword, values.get_mut(slot)) {
                *value = AxisValue::Keyword(k.as_str());
            }
        }
    }
    Some(BoxFacts {
        values,
        anchoring,
        origin_defaults: matches!(node, Node::Group(_) | Node::Instance(_)),
        flow: layout_flow(doc, id),
    })
}

fn classify(p: &PropertyValue) -> AxisValue {
    match p {
        PropertyValue::Dimension(d) => match zenith_core::dim_to_px(d.value, &d.unit) {
            Some(_) => AxisValue::Length,
            None => AxisValue::Unresolved(d.to_kdl_string()),
        },
        PropertyValue::TokenRef(id) => AxisValue::Token(id.clone()),
        PropertyValue::Literal(s) => AxisValue::Unresolved(format!("{s:?}")),
        PropertyValue::DataRef(s) => AxisValue::Unresolved(format!("(data){s:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{KdlAdapter, KdlSource};

    #[test]
    fn classifies_values_anchors_and_flow() {
        let src = r##"zenith version=1 {
  tokens format="zenith-token-v1" {
    token id="s.x" type="dimension" value=(px)10
  }
  document id="d" {
    page id="p" w=(px)200 h=(px)200 {
      rect id="a" x=(token)"s.x" y=(pt)4 w=(pct)50 h=(px)5
      rect id="b" anchor-sibling="a" anchor-edge="below" w=(px)5 h=(px)5
      rect id="c" anchor="top-left" w=(px)5 h=(px)5
      frame id="f" x=(px)0 y=(px)100 w=(px)100 h=(px)50 layout="row" {
        rect id="d1" w="hug" h=(px)5
      }
    }
  }
}"##;
        let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
        let page = doc.body.pages.first().expect("page");
        let facts = |id: &str| {
            let node = crate::doc::tree::locate(&doc, id).expect("node").node;
            box_facts(&doc, node, id).expect("box")
        };
        let a = facts("a");
        assert_eq!(*a.value(Axis::X), AxisValue::Token("s.x".to_owned()));
        assert_eq!(*a.value(Axis::Y), AxisValue::Length);
        assert!(matches!(a.value(Axis::W), AxisValue::Unresolved(_)));
        assert_eq!(a.anchoring, Anchoring::None);
        let b = facts("b");
        assert_eq!(b.anchoring, Anchoring::Edge { main: Axis::Y });
        assert!(b.anchor_supplies(Axis::Y) && b.anchor_supplies(Axis::X));
        assert_eq!(facts("c").anchoring, Anchoring::Other);
        let d1 = facts("d1");
        assert_eq!(*d1.value(Axis::W), AxisValue::Keyword("hug"));
        assert_eq!(d1.flow.map(|f| f.mode), Some("row"));
        assert!(page.children.len() >= 4);
    }
}
