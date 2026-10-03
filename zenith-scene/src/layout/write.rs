//! Write lowered geometry into nodes: set a box, mark hugging text, and
//! translate a subtree.

use std::collections::BTreeMap;

use zenith_core::{
    Dimension, FrameNode, Node, PathAnchor, Point, PropertyValue, ResolvedToken, Unit, dim_to_px,
};

use crate::compile::resolve_geometry_px;

use super::model::LayoutBox;

fn px(v: f64) -> PropertyValue {
    PropertyValue::Dimension(Dimension {
        value: v,
        unit: Unit::Px,
    })
}

fn put(
    x: &mut Option<PropertyValue>,
    y: &mut Option<PropertyValue>,
    w: &mut Option<PropertyValue>,
    h: &mut Option<PropertyValue>,
    b: LayoutBox,
) {
    *x = Some(px(b.x));
    *y = Some(px(b.y));
    *w = Some(px(b.w));
    *h = Some(px(b.h));
}

/// Set the `x` / `y` / `w` / `h` of a box node to `b` (px). Other kinds are
/// unchanged.
pub(super) fn set_box(node: &mut Node, b: LayoutBox) {
    match node {
        Node::Rect(n) => put(&mut n.x, &mut n.y, &mut n.w, &mut n.h, b),
        Node::Ellipse(n) => put(&mut n.x, &mut n.y, &mut n.w, &mut n.h, b),
        Node::Text(n) => put(&mut n.x, &mut n.y, &mut n.w, &mut n.h, b),
        Node::Code(n) => put(&mut n.x, &mut n.y, &mut n.w, &mut n.h, b),
        Node::Frame(n) => set_frame_box(n, b),
        Node::Group(n) => put(&mut n.x, &mut n.y, &mut n.w, &mut n.h, b),
        Node::Image(n) => put(&mut n.x, &mut n.y, &mut n.w, &mut n.h, b),
        Node::Field(n) => put(&mut n.x, &mut n.y, &mut n.w, &mut n.h, b),
        Node::Toc(n) => put(&mut n.x, &mut n.y, &mut n.w, &mut n.h, b),
        Node::Table(n) => put(&mut n.x, &mut n.y, &mut n.w, &mut n.h, b),
        Node::Shape(n) => put(&mut n.x, &mut n.y, &mut n.w, &mut n.h, b),
        Node::Pattern(n) => put(&mut n.x, &mut n.y, &mut n.w, &mut n.h, b),
        Node::Chart(n) => put(&mut n.x, &mut n.y, &mut n.w, &mut n.h, b),
        Node::Mesh(n) => put(&mut n.x, &mut n.y, &mut n.w, &mut n.h, b),
        Node::Line(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Instance(_)
        | Node::Footnote(_)
        | Node::Connector(_)
        | Node::Light(_)
        | Node::Unknown(_) => {}
    }
}

/// Set a frame's `x` / `y` / `w` / `h` to `b` (px).
pub(super) fn set_frame_box(f: &mut FrameNode, b: LayoutBox) {
    put(&mut f.x, &mut f.y, &mut f.w, &mut f.h, b);
}

/// A text whose height hugs its content draws exactly its box: it never
/// overflows, so it renders with `overflow="visible"` (no clip, no overflow
/// diagnostic for ink past the line box).
pub(super) fn mark_hugging_text(node: &mut Node) {
    if let Node::Text(t) = node {
        t.overflow = Some("visible".to_owned());
    }
}

/// Add `d` to a resolvable geometry value. An absent value (anchor-placed)
/// stays absent.
fn shift_pv(pv: &mut Option<PropertyValue>, d: f64, resolved: &BTreeMap<String, ResolvedToken>) {
    if let Some(v) = resolve_geometry_px(pv.as_ref(), resolved) {
        *pv = Some(px(v + d));
    }
}

/// Add `d` to a group origin; an absent origin counts as 0.
fn shift_origin(
    pv: &mut Option<PropertyValue>,
    d: f64,
    resolved: &BTreeMap<String, ResolvedToken>,
) {
    match pv {
        None => *pv = Some(px(d)),
        Some(_) => shift_pv(pv, d, resolved),
    }
}

fn shift_dim(dim: &mut Option<Dimension>, d: f64) {
    if let Some(v) = dim.as_ref().and_then(|x| dim_to_px(x.value, &x.unit)) {
        *dim = Some(Dimension {
            value: v + d,
            unit: Unit::Px,
        });
    }
}

fn shift_points(points: &mut [Point], dx: f64, dy: f64) {
    for p in points {
        shift_dim(&mut p.x, dx);
        shift_dim(&mut p.y, dy);
    }
}

fn shift_anchors(anchors: &mut [PathAnchor], dx: f64, dy: f64) {
    for a in anchors {
        shift_dim(&mut a.x, dx);
        shift_dim(&mut a.y, dy);
        shift_dim(&mut a.in_x, dx);
        shift_dim(&mut a.in_y, dy);
        shift_dim(&mut a.out_x, dx);
        shift_dim(&mut a.out_y, dy);
    }
}

/// Move `node` by `(dx, dy)` px.
///
/// A frame moves its descendants too (frames do not translate children). A
/// group and an instance move their origin, so their children follow. A
/// connector follows its targets. Footnotes and unknown nodes have no
/// geometry.
pub(super) fn translate(
    node: &mut Node,
    dx: f64,
    dy: f64,
    resolved: &BTreeMap<String, ResolvedToken>,
) {
    let xy = |x: &mut Option<PropertyValue>, y: &mut Option<PropertyValue>| {
        shift_pv(x, dx, resolved);
        shift_pv(y, dy, resolved);
    };
    match node {
        Node::Rect(n) => xy(&mut n.x, &mut n.y),
        Node::Ellipse(n) => xy(&mut n.x, &mut n.y),
        Node::Text(n) => xy(&mut n.x, &mut n.y),
        Node::Code(n) => xy(&mut n.x, &mut n.y),
        Node::Image(n) => xy(&mut n.x, &mut n.y),
        Node::Field(n) => xy(&mut n.x, &mut n.y),
        Node::Toc(n) => xy(&mut n.x, &mut n.y),
        Node::Table(n) => xy(&mut n.x, &mut n.y),
        Node::Shape(n) => xy(&mut n.x, &mut n.y),
        Node::Pattern(n) => xy(&mut n.x, &mut n.y),
        Node::Chart(n) => xy(&mut n.x, &mut n.y),
        Node::Mesh(n) => xy(&mut n.x, &mut n.y),
        Node::Light(n) => xy(&mut n.x, &mut n.y),
        Node::Frame(n) => {
            xy(&mut n.x, &mut n.y);
            for child in &mut n.children {
                translate(child, dx, dy, resolved);
            }
        }
        Node::Group(n) => {
            shift_origin(&mut n.x, dx, resolved);
            shift_origin(&mut n.y, dy, resolved);
        }
        Node::Instance(n) => {
            let shift = |dim: &mut Option<Dimension>, d: f64| match dim {
                None => {
                    *dim = Some(Dimension {
                        value: d,
                        unit: Unit::Px,
                    });
                }
                Some(_) => shift_dim(dim, d),
            };
            shift(&mut n.x, dx);
            shift(&mut n.y, dy);
        }
        Node::Line(n) => {
            shift_dim(&mut n.x1, dx);
            shift_dim(&mut n.y1, dy);
            shift_dim(&mut n.x2, dx);
            shift_dim(&mut n.y2, dy);
        }
        Node::Polygon(n) => shift_points(&mut n.points, dx, dy),
        Node::Polyline(n) => shift_points(&mut n.points, dx, dy),
        Node::Path(n) => {
            shift_anchors(&mut n.anchors, dx, dy);
            for sub in &mut n.subpaths {
                shift_anchors(&mut sub.anchors, dx, dy);
            }
        }
        Node::Connector(_) | Node::Footnote(_) | Node::Unknown(_) => {}
    }
}
