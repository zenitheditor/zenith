//! Position-dependent heights: lower again until positions settle.
//!
//! A hugging text that wraps around a `text-exclusion` box, or whose lines
//! snap to the page baseline grid, has a height that depends on where it
//! lands, and where it lands depends on the heights before it. One pass
//! measures such a text at its previous-pass position ([`ProbeHints`]) and
//! against the previous pass's runaround boxes. The list lowers again from
//! its authored nodes until a pass reproduces the positions and boxes it
//! measured with: then every measured height is the rendered height. A list
//! that has not settled after [`MAX_PASSES`] passes keeps the last pass and
//! reports `layout.conflicting_size` on the first text still moving.
//!
//! A list without position-dependent text lowers once.

use std::collections::BTreeMap;

use zenith_core::{Node, Span};

use crate::compile::{IntrinsicEnv, ProbeHints};

use super::diag::unsettled;
use super::lower::Lowered;

/// The most layout passes one node list runs.
pub(crate) const MAX_PASSES: usize = 8;

/// Runaround boxes `(x, y, w, h)` by node id.
pub(crate) type NodeBoxes = BTreeMap<String, (f64, f64, f64, f64)>;

/// One lowering pass over `nodes` with the given probe hints and runaround
/// boxes.
pub(crate) type Pass<'p> = dyn FnMut(&mut [Node], &ProbeHints, &NodeBoxes) -> Lowered + 'p;

/// Rebuilds the runaround boxes from a lowered list.
pub(crate) type Rebox<'r> = dyn Fn(&[Node]) -> NodeBoxes + 'r;

/// Lower `target` (a copy of `original`) until positions settle.
///
/// `rebox` rebuilds the runaround boxes from a lowered list; with `None` the
/// boxes are fixed (`boxes`, typically empty: the pass reads its own).
/// Returns the last pass's result and the number of passes run.
pub(crate) fn settle(
    original: &[Node],
    target: &mut [Node],
    boxes: NodeBoxes,
    rebox: Option<&Rebox<'_>>,
    pass: &mut Pass<'_>,
) -> (Lowered, usize) {
    let mut hints = ProbeHints::new();
    let mut boxes = boxes;
    let mut lowered = pass(target, &hints, &boxes);
    let mut passes = 1;
    loop {
        if !lowered.position_dependent {
            return (lowered, passes);
        }
        let next_boxes = rebox.map_or_else(|| boxes.clone(), |f| f(target));
        let moving =
            first_change(&hints, &lowered.hints).or_else(|| first_change(&boxes, &next_boxes));
        let Some(moving) = moving else {
            return (lowered, passes);
        };
        if passes >= MAX_PASSES {
            let span = find_span(target, &moving);
            lowered
                .diagnostics
                .push(unsettled(&moving, span, MAX_PASSES));
            return (lowered, passes);
        }
        hints = std::mem::take(&mut lowered.hints);
        boxes = next_boxes;
        for (t, o) in target.iter_mut().zip(original) {
            t.clone_from(o);
        }
        lowered = pass(target, &hints, &boxes);
        passes += 1;
    }
}

/// The first key whose value differs between `old` and `new`: a changed or
/// new key of `new` first (in key order), else a key `new` dropped.
fn first_change<V: PartialEq>(
    old: &BTreeMap<String, V>,
    new: &BTreeMap<String, V>,
) -> Option<String> {
    new.iter()
        .find(|(k, v)| old.get(*k) != Some(*v))
        .map(|(k, _)| k.clone())
        .or_else(|| old.keys().find(|k| !new.contains_key(*k)).cloned())
}

/// The source span of the first node with `id` in `nodes`.
fn find_span(nodes: &[Node], id: &str) -> Option<Span> {
    for node in nodes {
        if node.id() == Some(id) {
            return node.source_span();
        }
        let found = match node {
            Node::Frame(f) => find_span(&f.children, id),
            Node::Group(g) => find_span(&g.children, id),
            Node::Unknown(u) => find_span(&u.children, id),
            Node::Table(t) => t
                .rows
                .iter()
                .flat_map(|r| r.cells.iter())
                .find_map(|c| find_span(&c.children, id)),
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
            | Node::Mesh(_) => None,
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

/// `true` when `nodes` hold a node whose hug height can depend on its own
/// position under `env` (callers keep an authored copy only then).
pub(crate) fn may_depend_on_position(nodes: &[Node], env: &IntrinsicEnv<'_>) -> bool {
    nodes.iter().any(|node| {
        env.is_position_dependent(node)
            || match node {
                Node::Frame(f) => may_depend_on_position(&f.children, env),
                Node::Group(g) => may_depend_on_position(&g.children, env),
                Node::Unknown(u) => may_depend_on_position(&u.children, env),
                Node::Table(t) => t
                    .rows
                    .iter()
                    .flat_map(|r| r.cells.iter())
                    .any(|c| may_depend_on_position(&c.children, env)),
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
                | Node::Mesh(_) => false,
            }
    })
}
