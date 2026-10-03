//! The lowering walk: find each layout frame, solve it, and write the
//! resolved geometry into the nodes, so later passes see absolute geometry
//! only.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, FrameNode, Node};

use crate::compile::IntrinsicEnv;

use super::diag::Sink;
use super::measure::Engine;
use super::model::{Avail, ChildRole, LayoutBox, Mode, child_role, px_of};
use super::solve::{Slot, solve};
use super::write::{mark_hugging_text, set_box, set_frame_box, translate};

/// The result of lowering one node list.
#[derive(Debug, Default)]
pub(crate) struct Lowered {
    /// `true` when at least one layout frame was lowered.
    pub(crate) changed: bool,
    /// Layout diagnostics (`layout.unsized_child`, `layout.child_overflow`,
    /// `layout.fill_in_hug_parent`, `layout.conflicting_size`).
    pub(crate) diagnostics: Vec<Diagnostic>,
    /// Resolved box of every layout frame and flow child, keyed by node id,
    /// in page-absolute px. The first occurrence of an id wins.
    pub(crate) boxes: BTreeMap<String, LayoutBox>,
}

/// Lower every layout frame in `nodes` (at any depth) to absolute geometry.
///
/// A layout frame needs a resolvable `x` and `y`; one without them is left
/// as authored (compile reports `scene.missing_geometry`). Nodes outside
/// layout frames are unchanged.
pub(crate) fn lower_nodes(nodes: &mut [Node], env: IntrinsicEnv<'_>) -> Lowered {
    let engine = Engine { env };
    let mut out = Lowered::default();
    walk(engine, nodes, (0.0, 0.0), &mut out);
    out
}

/// Visit `nodes`; `offset` is the summed translation of enclosing groups.
fn walk(engine: Engine<'_>, nodes: &mut [Node], offset: (f64, f64), out: &mut Lowered) {
    for node in nodes {
        walk_node(engine, node, offset, out);
    }
}

fn walk_node(engine: Engine<'_>, node: &mut Node, offset: (f64, f64), out: &mut Lowered) {
    match node {
        Node::Frame(f) => {
            if Mode::of(f).is_some() {
                lower_root(engine, f, offset, out);
            } else {
                walk(engine, &mut f.children, offset, out);
            }
        }
        Node::Group(g) => {
            let gx = px_of(g.x.as_ref(), engine.resolved()).unwrap_or(0.0);
            let gy = px_of(g.y.as_ref(), engine.resolved()).unwrap_or(0.0);
            walk(engine, &mut g.children, (offset.0 + gx, offset.1 + gy), out);
        }
        Node::Unknown(u) => walk(engine, &mut u.children, offset, out),
        Node::Table(t) => {
            for row in &mut t.rows {
                for cell in &mut row.cells {
                    walk(engine, &mut cell.children, offset, out);
                }
            }
        }
        Node::Pattern(p) => walk_node(engine, &mut p.motif, offset, out),
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
        | Node::Chart(_)
        | Node::Light(_)
        | Node::Mesh(_) => {}
    }
}

/// Lower a layout frame that no layout frame places: it keeps its authored
/// `x` / `y`, and each axis without a size hugs.
fn lower_root(engine: Engine<'_>, f: &mut FrameNode, offset: (f64, f64), out: &mut Lowered) {
    let x = px_of(f.x.as_ref(), engine.resolved());
    let y = px_of(f.y.as_ref(), engine.resolved());
    let (Some(x), Some(y)) = (x, y) else {
        return;
    };
    let (aw, ah) = engine.root_size(f);
    arrange_frame(engine, f, (x, y), (aw, ah), offset, out);
}

/// Solve `f` at `origin` with the offered size, write its box, place its flow
/// children, and move its absolute children to its top-left.
fn arrange_frame(
    engine: Engine<'_>,
    f: &mut FrameNode,
    origin: (f64, f64),
    (aw, ah): (Avail, Avail),
    offset: (f64, f64),
    out: &mut Lowered,
) {
    out.changed = true;
    let sol = solve(
        engine,
        f,
        origin,
        aw,
        ah,
        &mut Sink::to(&mut out.diagnostics),
    );
    let own = LayoutBox {
        x: origin.0,
        y: origin.1,
        w: sol.w,
        h: sol.h,
    };
    record(out, &f.id, own, offset);
    set_frame_box(f, own);
    for slot in &sol.slots {
        if let Some(child) = f.children.get_mut(slot.index) {
            place_child(engine, child, *slot, offset, out);
        }
    }
    for child in &mut f.children {
        if child_role(child) == ChildRole::Absolute {
            translate(child, origin.0, origin.1, engine.resolved());
            walk_node(engine, child, offset, out);
        }
    }
}

/// Place one flow child in its slot.
fn place_child(
    engine: Engine<'_>,
    child: &mut Node,
    slot: Slot,
    offset: (f64, f64),
    out: &mut Lowered,
) {
    let b = LayoutBox {
        x: slot.x,
        y: slot.y,
        w: slot.w,
        h: slot.h,
    };
    if let Node::Frame(f) = child
        && Mode::of(f).is_some()
    {
        // A hugging axis re-solves as a hug (capped at the size it hugged
        // to), so its own `fill` children keep hug semantics.
        let along = |size: f64, hugs: bool| {
            if hugs {
                Avail::Hug(Some(size))
            } else {
                Avail::Definite(size)
            }
        };
        arrange_frame(
            engine,
            f,
            (b.x, b.y),
            (along(b.w, slot.hug_w), along(b.h, slot.hug_h)),
            offset,
            out,
        );
        return;
    }
    record(out, child.id_or_kind(), b, offset);
    set_box(child, b);
    if slot.hug_h {
        mark_hugging_text(child);
    }
    match child {
        // An absolute frame's children count from its top-left.
        Node::Frame(f) => {
            for grandchild in &mut f.children {
                translate(grandchild, b.x, b.y, engine.resolved());
            }
            walk(engine, &mut f.children, offset, out);
        }
        // A group translates its children; they stay group-local.
        Node::Group(g) => walk(
            engine,
            &mut g.children,
            (offset.0 + b.x, offset.1 + b.y),
            out,
        ),
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
        | Node::Unknown(_) => {}
    }
}

fn record(out: &mut Lowered, id: &str, b: LayoutBox, offset: (f64, f64)) {
    out.boxes.entry(id.to_owned()).or_insert(LayoutBox {
        x: b.x + offset.0,
        y: b.y + offset.1,
        ..b
    });
}
