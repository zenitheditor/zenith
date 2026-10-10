//! The numeric range every geometry value an op writes must keep.
//!
//! An op computes new numbers (a nudge adds a delta, a path transform
//! scales anchors). A result that is not finite, or so large that the text
//! cannot carry it exactly, is rejected with `tx.value_out_of_range` and
//! the transaction does not apply.

use zenith_core::{Diagnostic, Dimension, Document, Node, PropertyValue};

use super::find_node_any_shared;

/// The largest geometry magnitude an op may write: 2^53 px, the largest
/// range in which every integer is exact in an `f64`.
pub(crate) const MAX_GEOMETRY_ABS: f64 = 9_007_199_254_740_992.0;

/// `true` when `v` is finite and within ±[`MAX_GEOMETRY_ABS`].
pub(crate) fn in_range(v: f64) -> bool {
    v.is_finite() && v.abs() <= MAX_GEOMETRY_ABS
}

/// The `tx.value_out_of_range` error for `what` of `node_id` set to `v`
/// by op `op`.
pub(crate) fn out_of_range(op: &str, node_id: &str, what: &str, v: f64) -> Diagnostic {
    Diagnostic::error(
        "tx.value_out_of_range",
        format!(
            "{op}: {what} of node {node_id:?} would be {v}, outside the finite range \
             ±{MAX_GEOMETRY_ABS} px; use a smaller value or delta"
        ),
        None,
        Some(node_id.to_owned()),
    )
}

/// The out-of-range geometry values of each node in `ids`, before an op
/// runs: the values the op did not write.
pub(super) fn snapshot(doc: &Document, ids: &[String]) -> Vec<(String, Vec<(String, u64)>)> {
    ids.iter()
        .map(|id| {
            let bad = find_node_any_shared(doc, id)
                .map(|n| {
                    out_of_range_values(n)
                        .map(|(w, v)| (w, v.to_bits()))
                        .collect()
                })
                .unwrap_or_default();
            (id.clone(), bad)
        })
        .collect()
}

/// Check the nodes of `before` (see [`snapshot`]) after op `op` ran: one
/// `tx.value_out_of_range` per node with a value out of range that was not
/// out of range before. `true` when any node fails.
pub(super) fn check_nodes(
    doc: &Document,
    op: &dyn Fn() -> String,
    before: &[(String, Vec<(String, u64)>)],
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let mut failed = false;
    for (id, old) in before {
        let Some(node) = find_node_any_shared(doc, id) else {
            continue;
        };
        if let Some((what, v)) = out_of_range_values(node)
            .find(|(w, v)| !old.iter().any(|(ow, ov)| ow == w && *ov == v.to_bits()))
        {
            diagnostics.push(out_of_range(&op(), id, &what, v));
            failed = true;
        }
    }
    failed
}

/// The geometry values of `node` out of range: each name and value.
fn out_of_range_values(node: &Node) -> impl Iterator<Item = (String, f64)> {
    let mut values: Vec<(String, f64)> = Vec::new();
    let mut dim = |name: &str, d: Option<&Dimension>| {
        if let Some(d) = d {
            values.push((name.to_owned(), d.value));
        }
    };
    if let Some(view) = node.box_view() {
        for (name, p) in [("x", view.x), ("y", view.y), ("w", view.w), ("h", view.h)] {
            if let Some(PropertyValue::Dimension(d)) = p {
                dim(name, Some(d));
            }
        }
    }
    if let Some(anchor) = node.anchor_view() {
        dim("anchor-gap", anchor.anchor_gap);
    }
    dim("rotate", node.rotate());
    match node {
        Node::Instance(i) => {
            dim("x", i.x.as_ref());
            dim("y", i.y.as_ref());
            dim("w", i.w.as_ref());
            dim("h", i.h.as_ref());
        }
        Node::Line(l) => {
            dim("x1", l.x1.as_ref());
            dim("y1", l.y1.as_ref());
            dim("x2", l.x2.as_ref());
            dim("y2", l.y2.as_ref());
        }
        Node::Polygon(p) => points(&p.points, &mut dim),
        Node::Polyline(p) => points(&p.points, &mut dim),
        Node::Path(p) => {
            for (s, sub) in p.effective_subpaths().enumerate() {
                for (i, a) in sub.anchors.iter().enumerate() {
                    for (name, d) in [
                        ("x", &a.x),
                        ("y", &a.y),
                        ("in-x", &a.in_x),
                        ("in-y", &a.in_y),
                        ("out-x", &a.out_x),
                        ("out-y", &a.out_y),
                    ] {
                        dim(&format!("subpath {s} anchor {i} {name}"), d.as_ref());
                    }
                }
            }
        }
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Frame(_)
        | Node::Group(_)
        | Node::Image(_)
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
    values.into_iter().filter(|(_, v)| !in_range(*v))
}

fn points(points: &[zenith_core::Point], dim: &mut impl FnMut(&str, Option<&Dimension>)) {
    for (i, p) in points.iter().enumerate() {
        dim(&format!("point {i} x"), p.x.as_ref());
        dim(&format!("point {i} y"), p.y.as_ref());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_range_is_finite_and_exact() {
        assert!(in_range(0.0) && in_range(-MAX_GEOMETRY_ABS) && in_range(MAX_GEOMETRY_ABS));
        for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e19, -1e19] {
            assert!(!in_range(v), "{v}");
        }
    }
}
