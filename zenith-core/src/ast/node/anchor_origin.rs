//! [`derive_anchor_origin`]: the x / y an anchor gives a node.
//!
//! The scene and tx both place anchored nodes with this one function. Each
//! caller supplies the reference boxes it already knows ([`AnchorRefs`])
//! and a way to read sibling boxes ([`AnchorSiblings`]).
//!
//! Reference precedence:
//!
//! 1. `anchor-zone`: the named safe zone of the page. It needs a known
//!    9-point `anchor`. Edge placement does not combine with a zone.
//! 2. `anchor-sibling`: the box of the named sibling in the same scope. With
//!    `anchor-edge`, the node sits next to the sibling, and `anchor` only
//!    aligns the cross axis. Without it, `anchor` aligns inside the box.
//! 3. `anchor-parent="true"`: the box of the parent container.
//! 4. The page.
//!
//! `anchor-edge` without a sibling gives no origin. An unknown `anchor`
//! gives no origin, except with edge placement, where it reads as absent.
//!
//! Zone, parent, and page boxes are page-absolute. The result is in the
//! scope's space, so those paths subtract the scope origin. A sibling shares
//! the scope's space, so that path adds and subtracts nothing.

use std::collections::BTreeMap;

use crate::ast::document::SafeZone;
use crate::ast::value::{Dimension, dim_to_px};
use crate::tokens::ResolvedToken;

use super::Node;
use super::anchor::{Anchor, AnchorEdge, anchor_xy, parse_anchor, parse_anchor_edge};
use super::anchor_view::AnchorView;
use super::space::resolve_geometry_px;

/// The reference boxes of one sibling scope.
#[derive(Debug, Clone, Copy)]
pub struct AnchorRefs<'a> {
    /// The page size in px. `None` when it does not resolve, or for master
    /// chrome, which has no page.
    pub page: Option<(f64, f64)>,
    /// The safe zones of the page.
    pub safe_zones: &'a [SafeZone],
    /// The page-absolute `(x, y, w, h)` of the scope's anchor-parent
    /// container, when it resolves.
    pub parent_box: Option<(f64, f64, f64, f64)>,
    /// The page-absolute origin of the scope's space, when it resolves.
    pub origin: Option<(f64, f64)>,
    /// The resolved token table, for geometry token refs.
    pub resolved: &'a BTreeMap<String, ResolvedToken>,
}

/// The siblings of the node being placed.
pub trait AnchorSiblings {
    /// The sibling with id `id` in the scope, if any.
    fn sibling(&self, id: &str) -> Option<&Node>;

    /// The anchor origin of sibling `id` at its px size `size`, when it has
    /// one. Called only when the sibling lacks an authored px `x` or `y`.
    fn sibling_origin(&mut self, id: &str, size: (f64, f64)) -> Option<(f64, f64)>;
}

/// The anchor-derived `(x, y)` of the node with `view` at px size `size`,
/// in the space of its scope.
///
/// `None` when the node has no anchor, or when a reference box, a sibling
/// box, or the scope origin it needs does not resolve.
pub fn derive_anchor_origin(
    view: &AnchorView<'_>,
    (w, h): (f64, f64),
    refs: AnchorRefs<'_>,
    siblings: &mut dyn AnchorSiblings,
) -> Option<(f64, f64)> {
    let edge = view.anchor_edge.and_then(parse_anchor_edge);
    if view.anchor.is_none() && edge.is_none() {
        return None;
    }
    // Edge placement reads an unknown anchor as absent.
    let anchor: Option<Anchor> = match view.anchor {
        Some(s) => match parse_anchor(s) {
            Some(a) => Some(a),
            None => {
                edge?;
                None
            }
        },
        None => None,
    };

    if let Some(zone_id) = view.anchor_zone {
        let anchor = anchor?;
        let zone = refs.safe_zones.iter().find(|z| z.id == zone_id)?;
        let px = |d: &Dimension| dim_to_px(d.value, &d.unit);
        let (zx, zy, zw, zh) = (px(&zone.x)?, px(&zone.y)?, px(&zone.w)?, px(&zone.h)?);
        let (ox, oy) = anchor_xy(anchor, zw, zh, w, h);
        let (ax, ay) = refs.origin?;
        return Some((zx + ox - ax, zy + oy - ay));
    }

    if let Some(sib_id) = view.anchor_sibling {
        let (sx, sy, sw, sh) = sibling_box(sib_id, refs.resolved, siblings)?;
        if let Some(edge) = edge {
            let gap = view
                .anchor_gap
                .and_then(|d| dim_to_px(d.value, &d.unit))
                .unwrap_or(0.0);
            return Some(match edge {
                AnchorEdge::Below => (cross_h(anchor, sx, sw, w), sy + sh + gap),
                AnchorEdge::Above => (cross_h(anchor, sx, sw, w), sy - gap - h),
                AnchorEdge::After => (sx + sw + gap, cross_v(anchor, sy, sh, h)),
                AnchorEdge::Before => (sx - gap - w, cross_v(anchor, sy, sh, h)),
            });
        }
        let (ox, oy) = anchor_xy(anchor?, sw, sh, w, h);
        return Some((sx + ox, sy + oy));
    }

    // Edge placement needs a sibling.
    if edge.is_some() {
        return None;
    }
    let anchor = anchor?;
    if view.anchor_parent == Some(true) {
        let (rx, ry, rw, rh) = refs.parent_box?;
        let (ox, oy) = anchor_xy(anchor, rw, rh, w, h);
        let (ax, ay) = refs.origin?;
        return Some((rx + ox - ax, ry + oy - ay));
    }
    let (pw, ph) = refs.page?;
    let (x, y) = anchor_xy(anchor, pw, ph, w, h);
    let (ax, ay) = refs.origin?;
    Some((x - ax, y - ay))
}

/// The px box `(x, y, w, h)` of sibling `id`. Its size must be authored px.
/// Each axis of its origin is authored px, else its own anchor origin.
fn sibling_box(
    id: &str,
    resolved: &BTreeMap<String, ResolvedToken>,
    siblings: &mut dyn AnchorSiblings,
) -> Option<(f64, f64, f64, f64)> {
    let (x, y, w, h) = {
        let sib = siblings.sibling(id)?.anchor_view()?;
        let px = |v| resolve_geometry_px(v, resolved);
        (px(sib.x), px(sib.y), px(sib.w)?, px(sib.h)?)
    };
    let (x, y) = match (x, y) {
        (Some(x), Some(y)) => (x, y),
        (x, y) => {
            let entry = siblings.sibling_origin(id, (w, h));
            (x.or(entry.map(|e| e.0))?, y.or(entry.map(|e| e.1))?)
        }
    };
    Some((x, y, w, h))
}

/// The cross-axis x of `above` / `below` placement. No anchor aligns left.
fn cross_h(anchor: Option<Anchor>, sib_x: f64, sib_w: f64, w: f64) -> f64 {
    match anchor {
        None | Some(Anchor::TopLeft | Anchor::CenterLeft | Anchor::BottomLeft) => sib_x,
        Some(Anchor::TopCenter | Anchor::Center | Anchor::BottomCenter) => {
            sib_x + (sib_w - w) / 2.0
        }
        Some(Anchor::TopRight | Anchor::CenterRight | Anchor::BottomRight) => sib_x + sib_w - w,
    }
}

/// The cross-axis y of `before` / `after` placement. No anchor aligns top.
fn cross_v(anchor: Option<Anchor>, sib_y: f64, sib_h: f64, h: f64) -> f64 {
    match anchor {
        None | Some(Anchor::TopLeft | Anchor::TopCenter | Anchor::TopRight) => sib_y,
        Some(Anchor::CenterLeft | Anchor::Center | Anchor::CenterRight) => {
            sib_y + (sib_h - h) / 2.0
        }
        Some(Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight) => sib_y + sib_h - h,
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::Document;
    use crate::parse::{KdlAdapter, KdlSource};
    use crate::tokens::resolve_tokens;

    use super::*;

    fn page(body: &str) -> Document {
        let src = format!(
            r##"zenith version=1 {{
  project id="p" name="P"
  tokens format="zenith-token-v1" {{
    token id="dim.w" type="dimension" value=(px)40
  }}
  styles {{}}
  document id="d" title="D" {{
    page id="pg" w=(px)400 h=(px)300 {{
      safe-zone id="z" type="safe" x=(px)20 y=(px)30 w=(px)100 h=(px)60
      {body}
    }}
  }}
}}"##
        );
        KdlAdapter.parse(src.as_bytes()).expect("parse")
    }

    /// Siblings read from a node list. Origins come from a fixed table.
    struct List<'a> {
        nodes: &'a [Node],
        origins: BTreeMap<&'a str, (f64, f64)>,
        asked: Vec<String>,
    }

    impl AnchorSiblings for List<'_> {
        fn sibling(&self, id: &str) -> Option<&Node> {
            self.nodes.iter().find(|n| n.id() == Some(id))
        }

        fn sibling_origin(&mut self, id: &str, _size: (f64, f64)) -> Option<(f64, f64)> {
            self.asked.push(id.to_owned());
            self.origins.get(id).copied()
        }
    }

    /// Derive node `id` of `doc` with scope origin `origin`.
    fn derive(doc: &Document, id: &str, origin: Option<(f64, f64)>) -> Option<(f64, f64)> {
        derive_with(doc, id, origin, BTreeMap::new()).0
    }

    /// Derive node `id` of `doc`, with sibling origins from `origins`. Also
    /// returns the sibling ids whose origin the derivation asked for.
    fn derive_with<'d>(
        doc: &'d Document,
        id: &str,
        origin: Option<(f64, f64)>,
        origins: BTreeMap<&'d str, (f64, f64)>,
    ) -> (Option<(f64, f64)>, Vec<String>) {
        let resolved = resolve_tokens(&doc.tokens).resolved;
        let page = doc.body.pages.first().expect("page");
        let node = page
            .children
            .iter()
            .find(|n| n.id() == Some(id))
            .expect("node");
        let view = node.anchor_view().expect("view");
        let size = (
            resolve_geometry_px(view.w, &resolved).expect("w"),
            resolve_geometry_px(view.h, &resolved).expect("h"),
        );
        let refs = AnchorRefs {
            page: Some((400.0, 300.0)),
            safe_zones: &page.safe_zones,
            parent_box: Some((50.0, 60.0, 200.0, 100.0)),
            origin,
            resolved: &resolved,
        };
        let mut list = List {
            nodes: &page.children,
            origins,
            asked: Vec::new(),
        };
        let xy = derive_anchor_origin(&view, size, refs, &mut list);
        (xy, list.asked)
    }

    #[test]
    fn page_anchor_subtracts_the_scope_origin() {
        let p = page(r#"rect id="r" anchor="bottom-right" w=(px)40 h=(px)20"#);
        assert_eq!(derive(&p, "r", Some((0.0, 0.0))), Some((360.0, 280.0)));
        assert_eq!(derive(&p, "r", Some((10.0, 5.0))), Some((350.0, 275.0)));
        assert_eq!(derive(&p, "r", None), None);
    }

    #[test]
    fn zone_wins_over_sibling_and_parent() {
        let p = page(
            r#"rect id="s" x=(px)0 y=(px)0 w=(px)10 h=(px)10
      rect id="r" anchor="center" anchor-zone="z" anchor-sibling="s" anchor-parent=#true w=(px)40 h=(px)20"#,
        );
        assert_eq!(derive(&p, "r", Some((0.0, 0.0))), Some((50.0, 50.0)));
        assert_eq!(derive(&p, "r", Some((5.0, 5.0))), Some((45.0, 45.0)));
    }

    #[test]
    fn unknown_zone_or_unknown_anchor_gives_none() {
        let p = page(
            r#"rect id="a" anchor="center" anchor-zone="missing" w=(px)40 h=(px)20
      rect id="b" anchor="middle" w=(px)40 h=(px)20"#,
        );
        assert_eq!(derive(&p, "a", Some((0.0, 0.0))), None);
        assert_eq!(derive(&p, "b", Some((0.0, 0.0))), None);
    }

    #[test]
    fn parent_anchor_uses_the_parent_box() {
        let p = page(r#"rect id="r" anchor="bottom-right" anchor-parent=#true w=(px)40 h=(px)20"#);
        // Parent box (50, 60, 200, 100), scope origin (50, 60).
        assert_eq!(derive(&p, "r", Some((50.0, 60.0))), Some((160.0, 80.0)));
        assert_eq!(derive(&p, "r", None), None);
    }

    #[test]
    fn sibling_anchor_is_local_and_reads_token_sizes() {
        let p = page(
            r#"rect id="s" x=(px)100 y=(px)100 w=(token)"dim.w" h=(px)40
      rect id="r" anchor="center" anchor-sibling="s" w=(px)20 h=(px)20"#,
        );
        assert_eq!(derive(&p, "r", Some((7.0, 7.0))), Some((110.0, 110.0)));
        assert_eq!(derive(&p, "r", None), Some((110.0, 110.0)));
    }

    #[test]
    fn edge_placement_aligns_the_cross_axis() {
        let p = page(
            r#"rect id="s" x=(px)100 y=(px)100 w=(px)40 h=(px)40
      rect id="below" anchor-sibling="s" anchor-edge="below" anchor-gap=(px)5 w=(px)20 h=(px)10
      rect id="after" anchor-sibling="s" anchor-edge="after" anchor="bottom-left" w=(px)20 h=(px)10
      rect id="above" anchor-sibling="s" anchor-edge="above" anchor="top-center" anchor-gap=(px)2 w=(px)20 h=(px)10
      rect id="before" anchor-sibling="s" anchor-edge="before" anchor="middle" w=(px)20 h=(px)10"#,
        );
        let o = Some((0.0, 0.0));
        assert_eq!(derive(&p, "below", o), Some((100.0, 145.0)));
        assert_eq!(derive(&p, "after", o), Some((140.0, 130.0)));
        assert_eq!(derive(&p, "above", o), Some((110.0, 88.0)));
        // An unknown anchor reads as absent for edge placement.
        assert_eq!(derive(&p, "before", o), Some((80.0, 100.0)));
    }

    #[test]
    fn edge_without_sibling_gives_none() {
        let p = page(r#"rect id="r" anchor="center" anchor-edge="below" w=(px)20 h=(px)10"#);
        assert_eq!(derive(&p, "r", Some((0.0, 0.0))), None);
    }

    #[test]
    fn sibling_origin_fills_only_missing_axes() {
        let p = page(
            r#"rect id="s" x=(px)100 anchor="top-left" w=(px)40 h=(px)40
      rect id="t" x=(px)1 y=(px)2 w=(px)40 h=(px)40
      rect id="r" anchor="top-left" anchor-sibling="s" w=(px)20 h=(px)20
      rect id="q" anchor="top-left" anchor-sibling="t" w=(px)20 h=(px)20"#,
        );
        let origins = BTreeMap::from([("s", (7.0, 9.0))]);
        let (xy, asked) = derive_with(&p, "r", Some((0.0, 0.0)), origins);
        assert_eq!(xy, Some((100.0, 9.0)));
        assert_eq!(asked, vec!["s".to_owned()]);
        let (xy, asked) = derive_with(&p, "q", Some((0.0, 0.0)), BTreeMap::new());
        assert_eq!(xy, Some((1.0, 2.0)));
        assert!(asked.is_empty());
    }

    #[test]
    fn unresolved_sibling_gives_none() {
        let p = page(
            r#"rect id="s" x=(px)100 w=(px)40 h=(px)40
      rect id="u" x=(px)0 y=(px)0 w=(pct)40 h=(px)40
      rect id="a" anchor="center" anchor-sibling="s" w=(px)20 h=(px)20
      rect id="b" anchor="center" anchor-sibling="u" w=(px)20 h=(px)20
      rect id="c" anchor="center" anchor-sibling="missing" w=(px)20 h=(px)20"#,
        );
        let o = Some((0.0, 0.0));
        assert_eq!(derive(&p, "a", o), None);
        assert_eq!(derive(&p, "b", o), None);
        assert_eq!(derive(&p, "c", o), None);
    }

    #[test]
    fn no_anchor_gives_none() {
        let p = page(r#"rect id="r" x=(px)1 y=(px)1 w=(px)20 h=(px)20"#);
        assert_eq!(derive(&p, "r", Some((0.0, 0.0))), None);
    }
}
