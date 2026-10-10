//! Anchor followers: a gesture member whose `x` / `y` an anchor derives
//! from another member of the same gesture.
//!
//! The anchor carries such a member along with its target, so the member
//! gets no delta of its own on the axes the anchor supplies. A second delta
//! would move it twice (an edge anchor's gap would grow by the delta).

use std::collections::BTreeSet;

use zenith_core::Node;

use crate::doc::tree::Located;
use crate::geom::Pt;

/// How far an anchor chain counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reach {
    /// The direct target and, through it, every sibling it anchors to.
    Chain,
    /// The direct target only (a copy follows the copy of its target).
    Direct,
}

/// The authored axes an anchor carries along with another member.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Held {
    pub(crate) x: bool,
    pub(crate) y: bool,
    /// The index in `ids` of the member the anchor chain reaches.
    target: Option<usize>,
}

impl Held {
    /// `true` when the anchor carries at least one axis.
    pub(crate) fn any(self) -> bool {
        self.x || self.y
    }

    /// `p` with every held axis set to exactly 0.
    pub(crate) fn mask(self, p: Pt) -> Pt {
        (
            if self.x { 0.0 } else { p.0 },
            if self.y { 0.0 } else { p.1 },
        )
    }

    /// The gesture member the anchor chain reaches: an index into the
    /// `ids` passed to [`held`].
    pub(crate) fn target(self) -> Option<usize> {
        self.target
    }
}

/// The axes of `located` that its anchor carries along with a member of
/// `ids`: each absent `x` / `y` when the node's sibling anchor (directly,
/// or through a chain of sibling anchors with [`Reach::Chain`]) targets a
/// member. An authored `x` / `y` wins over the anchor, so it is not held.
pub(crate) fn held(located: &Located<'_>, ids: &[String], reach: Reach) -> Held {
    let Some(view) = located.node.anchor_view() else {
        return Held::default();
    };
    // A chain back to the node itself is a cycle: it places nothing.
    let mut seen: BTreeSet<&str> = BTreeSet::from([view.id]);
    let mut next = sibling_target(located.node);
    while let Some(target) = next {
        if !seen.insert(target) {
            break;
        }
        if let Some(index) = ids.iter().position(|id| id == target) {
            return Held {
                x: view.x.is_none(),
                y: view.y.is_none(),
                target: Some(index),
            };
        }
        next = match reach {
            Reach::Direct => None,
            Reach::Chain => located
                .siblings
                .iter()
                .find(|n| n.id() == Some(target))
                .and_then(sibling_target),
        };
    }
    Held::default()
}

/// The sibling whose box places `node`: its `anchor-sibling`, when an
/// `anchor` or `anchor-edge` puts it in effect and no `anchor-zone` (which
/// takes precedence) overrides it.
pub(crate) fn sibling_target(node: &Node) -> Option<&str> {
    let view = node.anchor_view()?;
    if view.anchor_zone.is_some() || (view.anchor.is_none() && view.anchor_edge.is_none()) {
        return None;
    }
    view.anchor_sibling
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::tree::locate;
    use zenith_core::{KdlAdapter, KdlSource};

    const SRC: &str = r#"zenith version=1 { document id="d" { page id="p" w=(px)400 h=(px)400 {
      rect id="a" x=(px)0 y=(px)0 w=(px)10 h=(px)10
      rect id="b" anchor-sibling="a" anchor-edge="below" w=(px)10 h=(px)10
      rect id="c" anchor-sibling="b" anchor-edge="after" y=(px)5 w=(px)10 h=(px)10
      rect id="z" anchor-sibling="a" anchor-zone="safe" anchor="center" w=(px)10 h=(px)10
      rect id="n" anchor-sibling="a" w=(px)10 h=(px)10
      rect id="loop1" anchor-sibling="loop2" anchor-edge="below" w=(px)10 h=(px)10
      rect id="loop2" anchor-sibling="loop1" anchor-edge="below" w=(px)10 h=(px)10
    } } }"#;

    fn held_of(id: &str, ids: &[&str], reach: Reach) -> Held {
        let doc = KdlAdapter.parse(SRC.as_bytes()).expect("parse");
        let located = locate(&doc, id).expect("node");
        let ids: Vec<String> = ids.iter().map(|s| (*s).to_owned()).collect();
        held(&located, &ids, reach)
    }

    #[test]
    fn a_direct_target_holds_the_absent_axes() {
        let h = held_of("b", &["a", "b"], Reach::Chain);
        assert!(h.x && h.y);
        assert_eq!(h.target(), Some(0));
        assert_eq!(h.mask((3.0, 4.0)), (0.0, 0.0));
    }

    #[test]
    fn a_chain_reaches_a_member_but_not_with_direct_reach() {
        let h = held_of("c", &["a", "c"], Reach::Chain);
        assert!(h.x && !h.y, "authored y wins: {h:?}");
        assert_eq!(h.mask((3.0, 4.0)), (0.0, 4.0));
        assert!(!held_of("c", &["a", "c"], Reach::Direct).any());
    }

    #[test]
    fn zone_anchors_unanchored_siblings_and_cycles_hold_nothing() {
        assert!(!held_of("z", &["a", "z"], Reach::Chain).any());
        assert!(!held_of("n", &["a", "n"], Reach::Chain).any());
        assert!(!held_of("loop1", &["a", "loop1"], Reach::Chain).any());
        assert!(!held_of("a", &["a", "b"], Reach::Chain).any());
    }
}
