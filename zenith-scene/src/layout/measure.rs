//! Hug sizes of flow children: content sizes from [`IntrinsicEnv`], layout
//! frames solved as hugging containers, and the child extents of groups and
//! absolute frames.

use std::collections::BTreeMap;

use zenith_core::{Dimension, FrameNode, Node, ResolvedToken, dim_to_px};

use crate::compile::IntrinsicEnv;

use super::diag::Sink;
use super::model::{Avail, Mode, px_of};
use super::solve::solve;

/// A child with no size on an axis: no fixed size, no content to hug.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Unsized;

/// The layout engine: measurement environment plus the measuring entry points.
#[derive(Clone, Copy)]
pub(super) struct Engine<'a> {
    pub(super) env: IntrinsicEnv<'a>,
}

impl<'a> Engine<'a> {
    pub(super) fn resolved(&self) -> &'a BTreeMap<String, ResolvedToken> {
        self.env.resolved()
    }

    /// The width `node` hugs to, at most `cap` px for wrapping content.
    pub(super) fn hug_w(&self, node: &Node, cap: Option<f64>) -> Result<f64, Unsized> {
        match node {
            Node::Frame(f) => Ok(match Mode::of(f) {
                Some(_) => {
                    let ah = self.own_avail(f.h.as_ref(), None);
                    solve(*self, f, (0.0, 0.0), Avail::Hug(cap), ah, &mut Sink::none()).w
                }
                None => self.extents(&f.children).0,
            }),
            Node::Group(g) => Ok(self.extents(&g.children).0),
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
            | Node::Table(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => {
                let natural = self.env.natural_width(node).ok_or(Unsized)?;
                Ok(cap.map_or(natural, |c| natural.min(c)))
            }
        }
    }

    /// The height `node` hugs to when laid out `w` px wide.
    pub(super) fn hug_h(&self, node: &Node, w: f64) -> Result<f64, Unsized> {
        match node {
            Node::Frame(f) => Ok(match Mode::of(f) {
                Some(_) => {
                    solve(
                        *self,
                        f,
                        (0.0, 0.0),
                        Avail::Definite(w),
                        Avail::Hug(None),
                        &mut Sink::none(),
                    )
                    .h
                }
                None => self.extents(&f.children).1,
            }),
            Node::Group(g) => Ok(self.extents(&g.children).1),
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
            | Node::Table(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => self.env.height_at(node, w).ok_or(Unsized),
        }
    }

    /// A fixed px size from `pv`, else a hugging size capped at `cap`.
    fn own_avail(&self, pv: Option<&zenith_core::PropertyValue>, cap: Option<f64>) -> Avail {
        px_of(pv, self.resolved()).map_or(Avail::Hug(cap), Avail::Definite)
    }

    /// The `(w, h)` of a layout frame placed at its authored size: a fixed
    /// axis keeps its size, an unsized axis hugs.
    pub(super) fn root_size(&self, f: &FrameNode) -> (Avail, Avail) {
        (
            self.own_avail(f.w.as_ref(), None),
            self.own_avail(f.h.as_ref(), None),
        )
    }

    /// The far right and bottom edges of `children` in their parent's local
    /// space, from the origin `(0, 0)`.
    ///
    /// Box children count their x/y (0 when absent) plus their fixed or hug
    /// size; lines, polygons, polylines, and paths count their points. Hidden
    /// and guide children, and kinds without geometry, add nothing.
    pub(super) fn extents(&self, children: &[Node]) -> (f64, f64) {
        let mut right: f64 = 0.0;
        let mut bottom: f64 = 0.0;
        let mut add = |x: f64, y: f64| {
            right = right.max(x);
            bottom = bottom.max(y);
        };
        for child in children {
            if child.role() == Some("guide") || child.visible() == Some(false) {
                continue;
            }
            if let Some(view) = child.box_view() {
                let x = px_of(view.x, self.resolved()).unwrap_or(0.0);
                let y = px_of(view.y, self.resolved()).unwrap_or(0.0);
                let (w, h) = self.free_size(child);
                add(x + w, y + h);
                continue;
            }
            for (x, y) in point_list(child) {
                add(x, y);
            }
        }
        (right, bottom)
    }

    /// The size of a child that no layout frame constrains: fixed axes keep
    /// their size, other axes hug without a cap (0 when nothing to hug).
    pub(super) fn free_size(&self, node: &Node) -> (f64, f64) {
        if let Node::Frame(f) = node
            && Mode::of(f).is_some()
        {
            let (aw, ah) = self.root_size(f);
            let sol = solve(*self, f, (0.0, 0.0), aw, ah, &mut Sink::none());
            return (sol.w, sol.h);
        }
        let Some(view) = node.box_view() else {
            return (0.0, 0.0);
        };
        let w =
            px_of(view.w, self.resolved()).unwrap_or_else(|| self.hug_w(node, None).unwrap_or(0.0));
        let h =
            px_of(view.h, self.resolved()).unwrap_or_else(|| self.hug_h(node, w).unwrap_or(0.0));
        (w, h)
    }
}

/// The px points of a point-based node (line ends, polygon / polyline
/// vertices, path anchors and handles). Empty for every other kind.
pub(super) fn point_list(node: &Node) -> Vec<(f64, f64)> {
    let px = |d: &Option<Dimension>| d.as_ref().and_then(|d| dim_to_px(d.value, &d.unit));
    let pair = |x: &Option<Dimension>, y: &Option<Dimension>| px(x).zip(px(y));
    match node {
        Node::Line(l) => [pair(&l.x1, &l.y1), pair(&l.x2, &l.y2)]
            .into_iter()
            .flatten()
            .collect(),
        Node::Polygon(p) => p
            .points
            .iter()
            .filter_map(|pt| pair(&pt.x, &pt.y))
            .collect(),
        Node::Polyline(p) => p
            .points
            .iter()
            .filter_map(|pt| pair(&pt.x, &pt.y))
            .collect(),
        Node::Path(p) => p
            .anchors
            .iter()
            .chain(p.subpaths.iter().flat_map(|s| s.anchors.iter()))
            .flat_map(|a| {
                [
                    pair(&a.x, &a.y),
                    pair(&a.in_x, &a.in_y),
                    pair(&a.out_x, &a.out_y),
                ]
            })
            .flatten()
            .collect(),
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Frame(_)
        | Node::Group(_)
        | Node::Image(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Footnote(_)
        | Node::Table(_)
        | Node::Shape(_)
        | Node::Connector(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_)
        | Node::Unknown(_) => Vec::new(),
    }
}
