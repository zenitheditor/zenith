//! Placement advisories that read a node's resolved box: `frame.child_overflow`
//! and `layout.off_canvas`.
//!
//! [`check_placement`] runs once per node from the main walk. [`placement_walk`]
//! repeats only these two checks over a node tree. The scene engine calls it
//! through [`super::super::geometry`] on a page whose auto-layout it has
//! lowered to absolute geometry.

use std::collections::BTreeMap;

use zenith_geometry::math;

use crate::ast::node::{FrameNode, LayoutKind, Node};
use crate::diagnostics::Diagnostic;
use crate::tokens::ResolvedToken;

use super::node::shared::{node_bbox, node_rotate_deg, pv_to_dim, resolve_axis};

/// A resolved px box `(x, y, w, h)`.
pub(super) type PxBox = (f64, f64, f64, f64);

/// A px translation `(dx, dy)` from a node list's space to page space.
pub(super) type Origin = (f64, f64);

/// Inputs to [`check_placement`] besides the node.
#[derive(Clone, Copy)]
pub(super) struct PlacementCtx {
    /// The page-space box of the nearest enclosing absolute frame.
    pub(super) enclosing_frame: Option<PxBox>,
    /// The page-space origin of the node's list.
    pub(super) origin: Origin,
    /// The page size in px, when it resolved.
    pub(super) page_bounds: Option<(f64, f64)>,
    /// `true` when the node or an ancestor has a decorative role.
    pub(super) exempt: bool,
}

/// `b` moved by `origin` into page space.
fn to_page((x, y, w, h): PxBox, origin: Origin) -> PxBox {
    if origin == (0.0, 0.0) {
        return (x, y, w, h);
    }
    (x + origin.0, y + origin.1, w, h)
}

/// Push `frame.child_overflow` when `node` protrudes past `enclosing_frame`,
/// and `layout.off_canvas` when it leaves the page.
///
/// `origin` moves the node's authored box into page space.
/// `enclosing_frame` is a page-space box. Both checks need the page size.
/// With `page_bounds = None` they are skipped.
/// An `exempt` node (decoration/background, own or inherited) gets neither.
pub(super) fn check_placement(node: &Node, ctx: PlacementCtx, diagnostics: &mut Vec<Diagnostic>) {
    let PlacementCtx {
        enclosing_frame,
        origin,
        page_bounds,
        exempt,
    } = ctx;
    if exempt {
        return;
    }
    let Some((page_w, page_h)) = page_bounds else {
        return;
    };
    let Some((nx, ny, nw, nh)) = node_bbox(node, page_w, page_h).map(|b| to_page(b, origin)) else {
        return;
    };

    // ── frame.child_overflow advisory ─────────────────────────────────────
    // When this node is a direct (or group-nested) child of a frame whose px
    // box resolved, advise if the child's bbox protrudes beyond the frame box
    // on any side. `node_bbox` returns None for missing geometry.
    if let Some((fx, fy, fw, fh)) = enclosing_frame {
        const EPSILON: f64 = 0.5;
        let over_left = nx < fx - EPSILON;
        let over_top = ny < fy - EPSILON;
        let over_right = nx + nw > fx + fw + EPSILON;
        let over_bottom = ny + nh > fy + fh + EPSILON;
        if over_left || over_top || over_right || over_bottom {
            let (node_id, node_span) = node.id_and_span();
            diagnostics.push(Diagnostic::advisory(
                "frame.child_overflow",
                format!(
                    "node '{}' (bbox {nx}, {ny}, {nw}, {nh}) protrudes beyond its \
                     enclosing frame (bbox {fx}, {fy}, {fw}, {fh})",
                    node_id
                ),
                node_span,
                Some(node_id.to_owned()),
            ));
        }
    }

    // ── off_canvas advisory ───────────────────────────────────────────────
    // Check whether the node's bounding box exceeds the page rect
    // [0, 0, page_w, page_h], in page space.
    //
    // A node with a non-zero `rotate` (deg) uses the axis-aligned bounding box
    // (AABB) of the four rotated corners instead of its box.
    let (ax, ay, aw, ah) = match node_rotate_deg(node) {
        Some(deg) if deg != 0.0 => rotated_aabb((nx, ny, nw, nh), deg),
        _ => (nx, ny, nw, nh),
    };
    if ax < 0.0 || ay < 0.0 || ax + aw > page_w || ay + ah > page_h {
        let (node_id, node_span) = node.id_and_span();
        diagnostics.push(Diagnostic::advisory(
            "layout.off_canvas",
            format!(
                "node '{}' extends outside the page bounds (0, 0, {page_w}, {page_h})",
                node_id
            ),
            node_span,
            Some(node_id.to_owned()),
        ));
    }
}

/// The AABB of `b` rotated by `deg` about its center.
fn rotated_aabb((nx, ny, nw, nh): PxBox, deg: f64) -> PxBox {
    let rad = deg.to_radians();
    let cos = math::cos(rad);
    let sin = math::sin(rad);
    let cx = nx + nw / 2.0;
    let cy = ny + nh / 2.0;
    let hw = nw / 2.0;
    let hh = nh / 2.0;
    let locals: [(f64, f64); 4] = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)];
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (lx, ly) in locals {
        let rx = cx + lx * cos - ly * sin;
        let ry = cy + lx * sin + ly * cos;
        min_x = min_x.min(rx);
        min_y = min_y.min(ry);
        max_x = max_x.max(rx);
        max_y = max_y.max(ry);
    }
    (min_x, min_y, max_x - min_x, max_y - min_y)
}

/// The page-space px box a frame's children are checked against for
/// `frame.child_overflow`. `origin` is the page-space origin of the frame's
/// own list.
///
/// `None` when the page size is unknown, when any of x/y/w/h does not resolve,
/// or when the frame lays out its children (its own layout reports overflow).
pub(super) fn frame_child_box(
    f: &FrameNode,
    origin: Origin,
    page_bounds: Option<(f64, f64)>,
) -> Option<PxBox> {
    let (page_w, page_h) = page_bounds?;
    if f.layout
        .as_ref()
        .is_some_and(LayoutKind::positions_children)
    {
        return None;
    }
    let x = pv_to_dim(f.x.as_ref()).and_then(|d| resolve_axis(d, page_w))?;
    let y = pv_to_dim(f.y.as_ref()).and_then(|d| resolve_axis(d, page_h))?;
    let w = pv_to_dim(f.w.as_ref()).and_then(|d| resolve_axis(d, page_w))?;
    let h = pv_to_dim(f.h.as_ref()).and_then(|d| resolve_axis(d, page_h))?;
    Some(to_page((x, y, w, h), origin))
}

/// Where a node list sits for [`placement_walk`].
#[derive(Clone, Copy)]
pub(in crate::validate::check) struct PlacementSite {
    /// The page-space box of the nearest enclosing absolute frame.
    pub(in crate::validate::check) enclosing_frame: Option<PxBox>,
    /// The page-space origin of the list.
    pub(in crate::validate::check) origin: Origin,
    pub(in crate::validate::check) page_bounds: (f64, f64),
    /// `true` when an ancestor has `role="decoration"` or `role="background"`.
    pub(in crate::validate::check) exempt: bool,
}

/// Run [`check_placement`] over `children` and their descendants, with the
/// same enclosing-frame and origin propagation as the main walk.
pub(in crate::validate::check) fn placement_walk(
    children: &[Node],
    site: PlacementSite,
    resolved: &BTreeMap<String, ResolvedToken>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let PlacementSite {
        enclosing_frame,
        origin,
        page_bounds,
        exempt: inherited,
    } = site;
    for node in children {
        let exempt = inherited || node.is_decorative();
        check_placement(
            node,
            PlacementCtx {
                enclosing_frame,
                origin,
                page_bounds: Some(page_bounds),
                exempt,
            },
            diagnostics,
        );
        let site = PlacementSite { exempt, ..site };
        match node {
            Node::Frame(f) => {
                let inner = PlacementSite {
                    enclosing_frame: frame_child_box(f, origin, Some(page_bounds)),
                    origin: node.child_origin(origin, resolved),
                    page_bounds,
                    exempt,
                };
                placement_walk(&f.children, inner, resolved, diagnostics);
            }
            Node::Group(g) => {
                let inner = PlacementSite {
                    origin: node.child_origin(origin, resolved),
                    ..site
                };
                placement_walk(&g.children, inner, resolved, diagnostics);
            }
            Node::Unknown(u) => {
                placement_walk(&u.children, site, resolved, diagnostics);
            }
            Node::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        placement_walk(&cell.children, site, resolved, diagnostics);
                    }
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
