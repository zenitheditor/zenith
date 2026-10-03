//! Placement advisories that read a node's resolved box: `frame.child_overflow`
//! and `layout.off_canvas`.
//!
//! [`check_placement`] runs once per node from the main walk. [`placement_walk`]
//! repeats only these two checks over a node tree. The scene engine calls it
//! through [`super::super::geometry`] on a page whose auto-layout it has
//! lowered to absolute geometry.

use crate::ast::node::{FrameNode, LayoutKind, Node};
use crate::diagnostics::Diagnostic;

use super::node::shared::{node_bbox, node_rotate_deg, pv_to_dim, resolve_axis};

/// A resolved px box `(x, y, w, h)`.
pub(super) type PxBox = (f64, f64, f64, f64);

/// Push `frame.child_overflow` when `node` protrudes past `enclosing_frame`,
/// and `layout.off_canvas` when it leaves the page.
///
/// Both checks need the page size. With `page_bounds = None` they are skipped.
pub(super) fn check_placement(
    node: &Node,
    enclosing_frame: Option<PxBox>,
    page_bounds: Option<(f64, f64)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some((page_w, page_h)) = page_bounds else {
        return;
    };
    let Some((nx, ny, nw, nh)) = node_bbox(node, page_w, page_h) else {
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
    // [0, 0, page_w, page_h]. Group translation offsets are NOT accumulated
    // (v0 advisory behavior).
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
    let cos = rad.cos();
    let sin = rad.sin();
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

/// The px box a frame's children are checked against for
/// `frame.child_overflow`.
///
/// `None` when the page size is unknown, when any of x/y/w/h does not resolve,
/// or when the frame lays out its children (its own layout reports overflow).
pub(super) fn frame_child_box(f: &FrameNode, page_bounds: Option<(f64, f64)>) -> Option<PxBox> {
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
    Some((x, y, w, h))
}

/// Run [`check_placement`] over `children` and their descendants, with the
/// same enclosing-frame propagation as the main walk.
pub(in crate::validate::check) fn placement_walk(
    children: &[Node],
    enclosing_frame: Option<PxBox>,
    page_bounds: (f64, f64),
    diagnostics: &mut Vec<Diagnostic>,
) {
    for node in children {
        check_placement(node, enclosing_frame, Some(page_bounds), diagnostics);
        match node {
            Node::Frame(f) => placement_walk(
                &f.children,
                frame_child_box(f, Some(page_bounds)),
                page_bounds,
                diagnostics,
            ),
            Node::Group(g) => {
                placement_walk(&g.children, enclosing_frame, page_bounds, diagnostics);
            }
            Node::Unknown(u) => {
                placement_walk(&u.children, enclosing_frame, page_bounds, diagnostics);
            }
            Node::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        placement_walk(&cell.children, enclosing_frame, page_bounds, diagnostics);
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
