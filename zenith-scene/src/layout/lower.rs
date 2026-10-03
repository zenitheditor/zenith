//! The lowering walk: find each layout frame, solve it, and write the
//! resolved geometry into the nodes, so later passes see absolute geometry
//! only.
//!
//! Every frame translates its children ([`Node::child_space`]): a layout
//! frame's box is written in its parent's space, and its flow children's
//! slots are written frame-local. Absolute children already count from the
//! frame's top-left and keep their authored values. [`Lowered::boxes`] stays
//! page-absolute.
//!
//! A layout frame no layout frame places is a root. A root with a resolvable
//! `x` and `y` lowers at them. A root placed by an anchor (`anchor`,
//! `anchor-parent`, `anchor-sibling`, `anchor-zone`, `anchor-edge`) is
//! measured first, then anchored at its measured size with the compile-time
//! anchor derivation, then arranged; its resolved `x` / `y` are written, so
//! compile and every dependent anchor read the same origin. A scope lowers in
//! source order, except that an anchor-sibling target lowers before its
//! dependent; a dependency cycle leaves its members unresolved (validation
//! reports `anchor.cycle`).

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, FrameNode, Node};

use crate::compile::{
    AnchorMap, IntrinsicEnv, ParentCtx, PrePassEnv, ProbeAt, ProbeHints, anchor_origin,
    anchor_sibling_of,
};

use super::diag::Sink;
use super::measure::Engine;
use super::memo::Memo;
use super::model::{Avail, ChildRole, LayoutBox, Mode, child_role, px_of};
use super::solve::{At, Slot, solve};
use super::write::{mark_hugging_text, set_box, set_frame_box, set_instance_box};

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
    /// Where each placed position-dependent text landed: the probe position
    /// for the next pass ([`super::settle`]).
    pub(crate) hints: ProbeHints,
    /// `true` when a placed flow child's height depends on its position.
    pub(crate) position_dependent: bool,
}

/// Lower every layout frame in `nodes` (at any depth) to absolute geometry.
///
/// `anchors` is the page's anchor environment; with `None` (a projected
/// master or an expanded instance), an anchored root stays unresolved, as
/// compile resolves no anchor there either. A root without a resolvable
/// origin is left as authored (compile reports `scene.missing_geometry`).
/// Nodes outside layout frames are unchanged.
pub(crate) fn lower_nodes(
    nodes: &mut [Node],
    env: IntrinsicEnv<'_>,
    anchors: Option<PrePassEnv<'_>>,
) -> Lowered {
    let memo = Memo::default();
    let cx = Walk {
        engine: Engine { env, memo: &memo },
        anchors,
    };
    let mut out = Lowered::default();
    let place = Place {
        offset: (0.0, 0.0),
        dev: env.base_translation(),
        parent_box: None,
    };
    walk_scope(&cx, nodes, place, &|_| true, &mut out);
    out
}

/// The walk-wide inputs.
#[derive(Clone, Copy)]
struct Walk<'a> {
    engine: Engine<'a>,
    anchors: Option<PrePassEnv<'a>>,
}

/// Where a node list sits.
#[derive(Clone, Copy)]
struct Place {
    /// Page-absolute translation: the summed child spaces of enclosing
    /// containers ([`Node::child_space`]).
    offset: (f64, f64),
    /// The render translation compile applies to the list, accumulated in
    /// compile's order (base translation, then each container child space).
    dev: (f64, f64),
    /// The anchor-parent reference box of the list's container, in
    /// page-absolute px, when it resolves.
    parent_box: Option<(f64, f64, f64, f64)>,
}

impl Place {
    fn anchor_parent(self) -> ParentCtx {
        ParentCtx {
            parent_box: self.parent_box,
            acc_dx: self.offset.0,
            acc_dy: self.offset.1,
        }
    }

    /// The children of a container with the local box `b`, when it
    /// resolves. `(sx, sy)` is the container's [`Node::child_space`]: the
    /// origin it adds to its children.
    fn enter(self, (sx, sy): (f64, f64), b: Option<(f64, f64, f64, f64)>) -> Self {
        Self {
            offset: (self.offset.0 + sx, self.offset.1 + sy),
            dev: (self.dev.0 + sx, self.dev.1 + sy),
            parent_box: b.map(|(x, y, w, h)| (x + self.offset.0, y + self.offset.1, w, h)),
        }
    }

    /// A list with no anchor-parent container (table cells, unknown nodes).
    fn detached(self) -> Self {
        Self {
            parent_box: None,
            ..self
        }
    }
}

/// Lowering state of one node in an anchored scope.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Pending,
    Visiting,
    Done,
}

/// A sibling scope that holds an anchored layout root: the anchor-bearing
/// ids, the per-node state, and the anchor entries derived so far.
struct Scope {
    index: BTreeMap<String, usize>,
    state: Vec<State>,
    entries: AnchorMap,
}

/// `true` for a layout frame placed by an anchor.
fn is_anchored_root(node: &Node) -> bool {
    layout_root(node).is_some() && node.box_view().is_some_and(|v| v.anchored)
}

/// Lower the nodes of one sibling scope that `include` selects.
fn walk_scope(
    cx: &Walk<'_>,
    nodes: &mut [Node],
    place: Place,
    include: &dyn Fn(&Node) -> bool,
    out: &mut Lowered,
) {
    let anchored = cx.anchors.is_some() && nodes.iter().any(|n| include(n) && is_anchored_root(n));
    if !anchored {
        for i in 0..nodes.len() {
            if nodes.get(i).is_some_and(include) {
                lower_at(cx, nodes, i, place, None, out);
            }
        }
        return;
    }
    let mut index = BTreeMap::new();
    for (i, node) in nodes.iter().enumerate() {
        if let Some(id) = node.id() {
            index.entry(id.to_owned()).or_insert(i);
        }
    }
    let mut scope = Scope {
        index,
        state: nodes
            .iter()
            .map(|n| {
                if include(n) {
                    State::Pending
                } else {
                    State::Done
                }
            })
            .collect(),
        entries: AnchorMap::new(),
    };
    for i in 0..nodes.len() {
        ensure(cx, nodes, i, place, &mut scope, out);
    }
}

/// Lower node `i` of an anchored scope after the sibling it anchors to.
fn ensure(
    cx: &Walk<'_>,
    nodes: &mut [Node],
    i: usize,
    place: Place,
    scope: &mut Scope,
    out: &mut Lowered,
) {
    match scope.state.get(i) {
        Some(State::Pending) => {}
        Some(State::Visiting | State::Done) | None => return,
    }
    if let Some(s) = scope.state.get_mut(i) {
        *s = State::Visiting;
    }
    let target = nodes
        .get(i)
        .and_then(anchor_sibling_of)
        .and_then(|id| scope.index.get(id).copied());
    if let Some(j) = target
        && j != i
    {
        ensure(cx, nodes, j, place, scope, out);
    }
    // The entry of a non-layout anchored node, for later dependents.
    let entry = nodes
        .get(i)
        .filter(|node| layout_root(node).is_none())
        .and_then(|node| {
            let view = node.box_view()?;
            let r = cx.engine.resolved();
            let size = (px_of(view.w, r)?, px_of(view.h, r)?);
            let lookup = |id: &str| scope.index.get(id).and_then(|&j| nodes.get(j));
            let xy = anchor_origin(
                node,
                size,
                cx.anchors?,
                place.anchor_parent(),
                &lookup,
                &scope.entries,
            )?;
            Some((node.id()?.to_owned(), xy))
        });
    if let Some((id, xy)) = entry {
        scope.entries.insert(id, xy);
    }
    lower_at(cx, nodes, i, place, Some(scope), out);
    if let Some(s) = scope.state.get_mut(i) {
        *s = State::Done;
    }
}

/// Lower node `i` of `nodes`: a layout root at its origin, any other node by
/// descending into it.
fn lower_at(
    cx: &Walk<'_>,
    nodes: &mut [Node],
    i: usize,
    place: Place,
    scope: Option<&Scope>,
    out: &mut Lowered,
) {
    let Some(node) = nodes.get(i) else {
        return;
    };
    let Some(f) = layout_root(node) else {
        if let Some(node) = nodes.get_mut(i) {
            walk_node(cx, node, place, out);
        }
        return;
    };
    // A root without a resolvable origin is left as authored.
    let Some(at) = root_origin(cx, nodes, node, f, place, scope) else {
        return;
    };
    if let Some(Node::Frame(f)) = nodes.get_mut(i) {
        arrange_root(cx, f, at, place, out);
    }
}

/// The layout frame `node` is, if any.
fn layout_root(node: &Node) -> Option<&FrameNode> {
    match node {
        Node::Frame(f) => Mode::of(f).map(|_| f),
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Line(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Group(_)
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
        | Node::Unknown(_) => None,
    }
}

/// The origin of root `f`: its authored `x` / `y`, an anchor-derived value
/// for each missing axis, or `None` when an axis does not resolve.
fn root_origin(
    cx: &Walk<'_>,
    nodes: &[Node],
    node: &Node,
    f: &FrameNode,
    place: Place,
    scope: Option<&Scope>,
) -> Option<(f64, f64)> {
    let x = px_of(f.x.as_ref(), cx.engine.resolved());
    let y = px_of(f.y.as_ref(), cx.engine.resolved());
    if let (Some(x), Some(y)) = (x, y) {
        return Some((x, y));
    }
    let (anchors, scope) = (cx.anchors?, scope?);
    if !is_anchored_root(node) {
        return None;
    }
    let size = cx.engine.free_size(node);
    let lookup = |id: &str| scope.index.get(id).and_then(|&j| nodes.get(j));
    let (ax, ay) = anchor_origin(
        node,
        size,
        anchors,
        place.anchor_parent(),
        &lookup,
        &scope.entries,
    )?;
    Some((x.unwrap_or(ax), y.unwrap_or(ay)))
}

/// Descend into a node that is not a scope-level layout root.
fn walk_node(cx: &Walk<'_>, node: &mut Node, place: Place, out: &mut Lowered) {
    let resolved = cx.engine.resolved();
    match node {
        Node::Frame(f) => {
            if Mode::of(f).is_some() {
                // A pattern motif: lowers at its authored origin.
                let x = px_of(f.x.as_ref(), resolved);
                let y = px_of(f.y.as_ref(), resolved);
                if let (Some(x), Some(y)) = (x, y) {
                    arrange_root(cx, f, (x, y), place, out);
                }
            } else {
                let inner = place.enter(f.child_space(resolved), frame_box(f, cx));
                walk_scope(cx, &mut f.children, inner, &|_| true, out);
            }
        }
        Node::Group(g) => {
            let (gx, gy) = g.child_space(resolved);
            let size = px_of(g.w.as_ref(), resolved).zip(px_of(g.h.as_ref(), resolved));
            let inner = place.enter((gx, gy), size.map(|(w, h)| (gx, gy, w, h)));
            walk_scope(cx, &mut g.children, inner, &|_| true, out);
        }
        Node::Unknown(u) => walk_scope(cx, &mut u.children, place.detached(), &|_| true, out),
        Node::Table(t) => {
            for row in &mut t.rows {
                for cell in &mut row.cells {
                    walk_scope(cx, &mut cell.children, place.detached(), &|_| true, out);
                }
            }
        }
        Node::Pattern(p) => walk_node(cx, &mut p.motif, place, out),
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

/// The local box of an absolute frame, when all four values resolve.
fn frame_box(f: &FrameNode, cx: &Walk<'_>) -> Option<(f64, f64, f64, f64)> {
    let r = cx.engine.resolved();
    Some((
        px_of(f.x.as_ref(), r)?,
        px_of(f.y.as_ref(), r)?,
        px_of(f.w.as_ref(), r)?,
        px_of(f.h.as_ref(), r)?,
    ))
}

/// Lower a root at `origin`: each axis without a size hugs.
fn arrange_root(
    cx: &Walk<'_>,
    f: &mut FrameNode,
    origin: (f64, f64),
    place: Place,
    out: &mut Lowered,
) {
    let avail = cx.engine.root_size(f);
    arrange_frame(cx, f, origin, avail, place, out);
}

/// Solve `f` at `origin` with the offered size, write its box, and place its
/// flow children. Slots solve in the frame's parent space and are written
/// frame-local (`slot - origin`). Absolute children already count from the
/// frame's top-left and stay as authored.
fn arrange_frame(
    cx: &Walk<'_>,
    f: &mut FrameNode,
    origin: (f64, f64),
    (aw, ah): (Avail, Avail),
    place: Place,
    out: &mut Lowered,
) {
    out.changed = true;
    let at = At {
        origin,
        dev: Some(place.dev),
    };
    let sol = solve(
        cx.engine,
        f,
        at,
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
    record(out, &f.id, own, place);
    set_frame_box(f, own);
    // The written box makes the frame's child space its origin.
    let space = f.child_space(cx.engine.resolved());
    let inner = place.enter(space, Some((own.x, own.y, own.w, own.h)));
    for slot in &sol.slots {
        if let Some(child) = f.children.get_mut(slot.index) {
            place_child(cx, child, frame_local(*slot, space), inner, out);
        }
    }
    let absolute = |n: &Node| child_role(n) == ChildRole::Absolute;
    walk_scope(cx, &mut f.children, inner, &absolute, out);
}

/// A slot solved in the frame's parent space, moved into the frame's child
/// space `(sx, sy)`.
fn frame_local(slot: Slot, (sx, sy): (f64, f64)) -> Slot {
    Slot {
        x: slot.x - sx,
        y: slot.y - sy,
        ..slot
    }
}

/// Place one flow child in its slot.
fn place_child(cx: &Walk<'_>, child: &mut Node, slot: Slot, place: Place, out: &mut Lowered) {
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
            cx,
            f,
            (b.x, b.y),
            (along(b.w, slot.hug_w), along(b.h, slot.hug_h)),
            place,
            out,
        );
        return;
    }
    record(out, child.id_or_kind(), b, place);
    if cx.engine.env.is_position_dependent(child) {
        out.position_dependent = true;
        out.hints
            .entry(child.id_or_kind().to_owned())
            .or_insert(ProbeAt {
                x: b.x,
                y: b.y,
                dx: place.dev.0,
                dy: place.dev.1,
            });
    }
    if let Node::Instance(i) = child {
        let bounds = cx.engine.instance_bounds(i);
        set_instance_box(i, b, bounds);
        return;
    }
    set_box(child, b);
    if slot.hug_h {
        mark_hugging_text(child);
    }
    match child {
        // An absolute frame's children count from its top-left; they stay
        // frame-local.
        Node::Frame(f) => {
            let space = f.child_space(cx.engine.resolved());
            let inner = place.enter(space, Some((b.x, b.y, b.w, b.h)));
            walk_scope(cx, &mut f.children, inner, &|_| true, out);
        }
        // A group translates its children; they stay group-local.
        Node::Group(g) => {
            let space = g.child_space(cx.engine.resolved());
            let inner = place.enter(space, Some((b.x, b.y, b.w, b.h)));
            walk_scope(cx, &mut g.children, inner, &|_| true, out);
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

fn record(out: &mut Lowered, id: &str, b: LayoutBox, place: Place) {
    out.boxes.entry(id.to_owned()).or_insert(LayoutBox {
        x: b.x + place.offset.0,
        y: b.y + place.offset.1,
        ..b
    });
}
