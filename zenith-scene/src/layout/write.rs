//! Write lowered geometry into nodes: set a box and mark hugging text.
//!
//! Every box is written in the node's parent space: a layout frame's own box
//! in its parent's space, a flow child's slot frame-local (the frame's
//! child space is its `x` / `y`). Nothing moves a subtree: absolute children
//! already count from their frame's top-left.

use zenith_core::{FrameNode, InstanceNode, Node, PropertyValue};

use crate::compile::{px as px_dim, px_prop as px};

use super::model::LayoutBox;

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
/// unchanged ([`set_instance_box`] places an instance).
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

/// Place an instance in the slot `b`.
///
/// When the slot is exactly the component's content size (`bounds`, from the
/// instance origin) and the instance sets no `w` / `h`, only its origin
/// moves, so the content's top-left lands on the slot's: the instance
/// compiles on the translate-only path, as outside a layout frame. Otherwise
/// the slot becomes the instance's `x` / `y` / `w` / `h` box and the component
/// fits into it (`fit`, default `contain`).
pub(super) fn set_instance_box(
    i: &mut InstanceNode,
    b: LayoutBox,
    bounds: Option<(f64, f64, f64, f64)>,
) {
    match bounds {
        Some((min_x, min_y, w, h)) if i.w.is_none() && i.h.is_none() && b.w == w && b.h == h => {
            i.x = Some(px_dim(b.x - min_x));
            i.y = Some(px_dim(b.y - min_y));
        }
        Some(_) | None => {
            i.x = Some(px_dim(b.x));
            i.y = Some(px_dim(b.y));
            i.w = Some(px_dim(b.w));
            i.h = Some(px_dim(b.h));
        }
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
