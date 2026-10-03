//! Hug sizes of flow children: content sizes from [`IntrinsicEnv`], layout
//! frames solved as hugging containers, instances from their expanded
//! component bounds, and the child extents of groups and absolute frames.
//!
//! Every query is memoized for the lowering ([`Memo`]).

use std::collections::BTreeMap;

use zenith_core::{Dimension, FrameNode, InstanceNode, Node, ResolvedToken};

use crate::compile::{IntrinsicEnv, ProbeAt};

use super::diag::Sink;
use super::memo::{Bounds, Memo, cap_key, node_key, probe_key};
use super::model::{Avail, Mode, dim_px, px_of};
use super::solve::{At, solve};

/// A child with no size on an axis: no fixed size, no content to hug.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Unsized;

/// The layout engine: measurement environment, the lowering's memo, and the
/// measuring entry points.
#[derive(Clone, Copy)]
pub(super) struct Engine<'a> {
    pub(super) env: IntrinsicEnv<'a>,
    pub(super) memo: &'a Memo,
}

impl<'a> Engine<'a> {
    pub(super) fn resolved(&self) -> &'a BTreeMap<String, ResolvedToken> {
        self.env.resolved()
    }

    /// The width `node` hugs to, at most `cap` px for wrapping content.
    pub(super) fn hug_w(&self, node: &Node, cap: Option<f64>) -> Result<f64, Unsized> {
        self.memo.hug_w((node_key(node), cap_key(cap)), || {
            // A cap at or above the uncapped hug width binds nothing: the
            // content lays out as uncapped, so share that entry.
            if let Some(c) = cap {
                let free = self.hug_w(node, None)?;
                if free <= c {
                    return Ok(free);
                }
            }
            self.hug_w_uncached(node, cap)
        })
    }

    fn hug_w_uncached(&self, node: &Node, cap: Option<f64>) -> Result<f64, Unsized> {
        match node {
            Node::Frame(f) => Ok(match Mode::of(f) {
                Some(_) => {
                    let ah = self.own_avail(f.h.as_ref(), None);
                    solve(
                        *self,
                        f,
                        At::UNPLACED,
                        Avail::Hug(cap),
                        ah,
                        &mut Sink::none(),
                    )
                    .w
                }
                None => self.extents(&f.children).0,
            }),
            Node::Group(g) => Ok(self.extents(&g.children).0),
            Node::Instance(i) => {
                let (_, _, bw, bh) = self.instance_bounds(i).ok_or(Unsized)?;
                let natural = match dim_px(&i.h) {
                    Some(h) if h != bh => h * bw / bh,
                    Some(_) | None => bw,
                };
                Ok(cap.map_or(natural, |c| natural.min(c)))
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

    /// The height `node` hugs to when laid out `w` px wide, at an unknown
    /// position.
    pub(super) fn hug_h(&self, node: &Node, w: f64) -> Result<f64, Unsized> {
        self.memo.hug_h((node_key(node), w.to_bits()), || {
            self.hug_h_uncached(node, w, None)
        })
    }

    /// The height `node` hugs to when laid out `w` px wide at `at`. A node
    /// whose subtree holds position-dependent text measures there; any other
    /// node measures as [`Engine::hug_h`].
    pub(super) fn hug_h_at(
        &self,
        node: &Node,
        w: f64,
        at: Option<ProbeAt>,
    ) -> Result<f64, Unsized> {
        match at {
            Some(p) if self.depends_on_position(node) => {
                let key = (node_key(node), w.to_bits(), probe_key(p));
                self.memo
                    .hug_h_at(key, || self.hug_h_uncached(node, w, Some(p)))
            }
            Some(_) | None => self.hug_h(node, w),
        }
    }

    /// `true` when `node` or a descendant has a position-dependent height.
    fn depends_on_position(&self, node: &Node) -> bool {
        self.memo.depends(node_key(node), || {
            self.env.is_position_dependent(node)
                || match node {
                    Node::Frame(f) => f.children.iter().any(|c| self.depends_on_position(c)),
                    Node::Group(g) => g.children.iter().any(|c| self.depends_on_position(c)),
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
                    | Node::Unknown(_) => false,
                }
        })
    }

    fn hug_h_uncached(&self, node: &Node, w: f64, at: Option<ProbeAt>) -> Result<f64, Unsized> {
        match node {
            Node::Frame(f) => Ok(match Mode::of(f) {
                Some(_) => {
                    let at = at.map_or(At::UNPLACED, |p| At {
                        origin: (p.x, p.y),
                        dev: Some((p.dx, p.dy)),
                    });
                    solve(
                        *self,
                        f,
                        at,
                        Avail::Definite(w),
                        Avail::Hug(None),
                        &mut Sink::none(),
                    )
                    .h
                }
                None => self.extents(&f.children).1,
            }),
            Node::Group(g) => Ok(self.extents(&g.children).1),
            Node::Instance(i) => {
                let (_, _, bw, bh) = self.instance_bounds(i).ok_or(Unsized)?;
                Ok(if w == bw { bh } else { w * bh / bw })
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
            | Node::Unknown(_) => self.env.height_at(node, w, at).ok_or(Unsized),
        }
    }

    /// The content bounds `(min_x, min_y, w, h)` of an instance's expanded
    /// component, in its local space.
    pub(super) fn instance_bounds(&self, instance: &InstanceNode) -> Bounds {
        let key = std::ptr::from_ref(instance).addr();
        self.memo.bounds(key, || self.env.instance_bounds(instance))
    }

    /// The far right and bottom edges an instance covers in its parent's
    /// local space: its fit box when `w` and `h` are both set, else its
    /// content bounds moved by its origin.
    fn instance_extent(&self, instance: &InstanceNode) -> Option<(f64, f64)> {
        let x = dim_px(&instance.x).unwrap_or(0.0);
        let y = dim_px(&instance.y).unwrap_or(0.0);
        if let (Some(w), Some(h)) = (dim_px(&instance.w), dim_px(&instance.h)) {
            return Some((x + w, y + h));
        }
        let (min_x, min_y, w, h) = self.instance_bounds(instance)?;
        Some((x + min_x + w, y + min_y + h))
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
    /// size; instances count their fit box or content bounds; lines,
    /// polygons, polylines, and paths count their points. Hidden and guide
    /// children, and kinds without geometry, add nothing.
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
            if let Node::Instance(i) = child {
                if let Some((x, y)) = self.instance_extent(i) {
                    add(x, y);
                }
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
            return self.memo.free(node_key(node), || {
                let (aw, ah) = self.root_size(f);
                let sol = solve(*self, f, At::UNPLACED, aw, ah, &mut Sink::none());
                (sol.w, sol.h)
            });
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
    let pair = |x: &Option<Dimension>, y: &Option<Dimension>| dim_px(x).zip(dim_px(y));
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
