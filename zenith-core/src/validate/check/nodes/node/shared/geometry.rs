//! Geometry resolution, bounding-box/AABB computation, and per-node
//! role/id/rotation extraction shared by the per-kind checks.

use crate::ast::node::{Node, PathAnchor, Point, anchor_xy, parse_anchor};
use crate::ast::value::{Dimension, PropertyValue, Unit, dim_to_px};

use super::dims::pv_to_dim;

/// Resolve a single geometry axis dimension to pixels.
///
/// `Pct` is resolved against `basis` (e.g. page_w for x/w, page_h for y/h).
/// All other convertible units delegate to [`dim_to_px`]; `None` on failure.
pub(in crate::validate::check) fn resolve_axis(dim: &Dimension, basis: f64) -> Option<f64> {
    if dim.unit == Unit::Pct {
        Some(dim.value / 100.0 * basis)
    } else {
        dim_to_px(dim.value, &dim.unit)
    }
}

/// Compute the authored bounding box `(x, y, w, h)` of a node in pixels.
///
/// Returns `None` when the node has no resolvable bounding box (Group, Unknown,
/// or any node with a missing/unresolvable required dimension). Callers should
/// treat `None` as "no check possible" and produce no advisory.
///
/// v0 NOTE: authored coordinates are used as-is. Group translation offsets are
/// NOT accumulated here (that is a scene-compiler / render-time concern). The
/// off_canvas advisory documents this v0 behavior: it checks authored geometry
/// against the page rectangle, not render-time geometry.
pub(in crate::validate::check) fn node_bbox(
    node: &Node,
    page_w: f64,
    page_h: f64,
) -> Option<(f64, f64, f64, f64)> {
    match node {
        Node::Rect(n) => {
            let x = resolve_axis(pv_to_dim(n.x.as_ref())?, page_w)?;
            let y = resolve_axis(pv_to_dim(n.y.as_ref())?, page_h)?;
            let w = resolve_axis(pv_to_dim(n.w.as_ref())?, page_w)?;
            let h = resolve_axis(pv_to_dim(n.h.as_ref())?, page_h)?;
            Some((x, y, w, h))
        }
        Node::Ellipse(n) => {
            let x = resolve_axis(pv_to_dim(n.x.as_ref())?, page_w)?;
            let y = resolve_axis(pv_to_dim(n.y.as_ref())?, page_h)?;
            let w = resolve_axis(pv_to_dim(n.w.as_ref())?, page_w)?;
            let h = resolve_axis(pv_to_dim(n.h.as_ref())?, page_h)?;
            Some((x, y, w, h))
        }
        Node::Image(n) => {
            let x = resolve_axis(pv_to_dim(n.x.as_ref())?, page_w)?;
            let y = resolve_axis(pv_to_dim(n.y.as_ref())?, page_h)?;
            let w = resolve_axis(pv_to_dim(n.w.as_ref())?, page_w)?;
            let h = resolve_axis(pv_to_dim(n.h.as_ref())?, page_h)?;
            Some((x, y, w, h))
        }
        Node::Frame(n) => {
            let x = resolve_axis(pv_to_dim(n.x.as_ref())?, page_w)?;
            let y = resolve_axis(pv_to_dim(n.y.as_ref())?, page_h)?;
            let w = resolve_axis(pv_to_dim(n.w.as_ref())?, page_w)?;
            let h = resolve_axis(pv_to_dim(n.h.as_ref())?, page_h)?;
            Some((x, y, w, h))
        }
        Node::Text(n) => {
            let w = resolve_axis(pv_to_dim(n.w.as_ref())?, page_w)?;
            let h = resolve_axis(pv_to_dim(n.h.as_ref())?, page_h)?;
            let resolve_text_axis = |value: &PropertyValue, basis: f64| -> Option<f64> {
                resolve_axis(pv_to_dim(Some(value))?, basis)
            };
            let (x, y) = match (n.x.as_ref(), n.y.as_ref()) {
                (Some(x), Some(y)) => {
                    (resolve_text_axis(x, page_w)?, resolve_text_axis(y, page_h)?)
                }
                (None, None) => {
                    let anchor = parse_anchor(n.anchor.as_deref()?)?;
                    anchor_xy(anchor, page_w, page_h, w, h)
                }
                (Some(_), None) | (None, Some(_)) => return None,
            };
            Some((x, y, w, h))
        }
        Node::Code(n) => {
            let x = resolve_axis(pv_to_dim(n.x.as_ref())?, page_w)?;
            let y = resolve_axis(pv_to_dim(n.y.as_ref())?, page_h)?;
            let w = resolve_axis(pv_to_dim(n.w.as_ref())?, page_w)?;
            let h = resolve_axis(pv_to_dim(n.h.as_ref())?, page_h)?;
            Some((x, y, w, h))
        }
        Node::Line(n) => {
            let x1 = resolve_axis(n.x1.as_ref()?, page_w)?;
            let y1 = resolve_axis(n.y1.as_ref()?, page_h)?;
            let x2 = resolve_axis(n.x2.as_ref()?, page_w)?;
            let y2 = resolve_axis(n.y2.as_ref()?, page_h)?;
            let bx = x1.min(x2);
            let by = y1.min(y2);
            let bw = (x2 - x1).abs();
            let bh = (y2 - y1).abs();
            Some((bx, by, bw, bh))
        }
        Node::Polygon(n) => points_bbox(&n.points, page_w, page_h),
        Node::Polyline(n) => points_bbox(&n.points, page_w, page_h),
        Node::Path(n) => path_anchors_bbox(&n.anchors, page_w, page_h),
        // Groups have no authoritative bbox in v0 — children are checked
        // individually. An instance likewise has no authoritative bbox: its
        // expanded subtree (a translated group) is the renderable geometry. A
        // field's box defaults to the page live area at compile time (x/w may be
        // omitted), so there is no authored bbox to check against the page here.
        // A toc likewise defaults to the live area; no authored bbox check.
        Node::Table(n) => {
            let x = resolve_axis(pv_to_dim(n.x.as_ref())?, page_w)?;
            let y = resolve_axis(pv_to_dim(n.y.as_ref())?, page_h)?;
            let w = resolve_axis(pv_to_dim(n.w.as_ref())?, page_w)?;
            let h = resolve_axis(pv_to_dim(n.h.as_ref())?, page_h)?;
            Some((x, y, w, h))
        }
        Node::Shape(n) => {
            let x = resolve_axis(pv_to_dim(n.x.as_ref())?, page_w)?;
            let y = resolve_axis(pv_to_dim(n.y.as_ref())?, page_h)?;
            let w = resolve_axis(pv_to_dim(n.w.as_ref())?, page_w)?;
            let h = resolve_axis(pv_to_dim(n.h.as_ref())?, page_h)?;
            Some((x, y, w, h))
        }
        Node::Pattern(n) => {
            let x = resolve_axis(pv_to_dim(n.x.as_ref())?, page_w)?;
            let y = resolve_axis(pv_to_dim(n.y.as_ref())?, page_h)?;
            let w = resolve_axis(pv_to_dim(n.w.as_ref())?, page_w)?;
            let h = resolve_axis(pv_to_dim(n.h.as_ref())?, page_h)?;
            Some((x, y, w, h))
        }
        Node::Chart(n) => {
            let x = resolve_axis(pv_to_dim(n.x.as_ref())?, page_w)?;
            let y = resolve_axis(pv_to_dim(n.y.as_ref())?, page_h)?;
            let w = resolve_axis(pv_to_dim(n.w.as_ref())?, page_w)?;
            let h = resolve_axis(pv_to_dim(n.h.as_ref())?, page_h)?;
            Some((x, y, w, h))
        }
        Node::Light(n) => {
            let x = resolve_axis(pv_to_dim(n.x.as_ref())?, page_w)?;
            let y = resolve_axis(pv_to_dim(n.y.as_ref())?, page_h)?;
            let r = resolve_axis(pv_to_dim(n.radius.as_ref())?, page_w.max(page_h))?;
            Some((x - r, y - r, r * 2.0, r * 2.0))
        }
        Node::Mesh(n) => {
            let x = resolve_axis(pv_to_dim(n.x.as_ref())?, page_w)?;
            let y = resolve_axis(pv_to_dim(n.y.as_ref())?, page_h)?;
            let w = resolve_axis(pv_to_dim(n.w.as_ref())?, page_w)?;
            let h = resolve_axis(pv_to_dim(n.h.as_ref())?, page_h)?;
            Some((x, y, w, h))
        }
        // A connector has no authored bbox: its endpoints are DERIVED from the
        // resolved boxes of its `from`/`to` targets at compile time and are not
        // known at validate time, so there is no off_canvas check here.
        Node::Group(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Footnote(_)
        | Node::Connector(_)
        | Node::Unknown(_) => None,
    }
}

/// Compute the bounding box of a slice of [`Point`]s, resolving each coordinate
/// against the given page axis bases.
///
/// Returns `Some((min_x, min_y, w, h))` when at least one point resolves
/// successfully, `None` when no point has resolvable coordinates.
fn points_bbox(points: &[Point], page_w: f64, page_h: f64) -> Option<(f64, f64, f64, f64)> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut any = false;
    for pt in points {
        if let (Some(px_val), Some(py_val)) = (
            pt.x.as_ref().and_then(|d| resolve_axis(d, page_w)),
            pt.y.as_ref().and_then(|d| resolve_axis(d, page_h)),
        ) {
            extend_bbox(
                &mut min_x, &mut min_y, &mut max_x, &mut max_y, px_val, py_val,
            );
            any = true;
        }
    }
    if any {
        Some((min_x, min_y, max_x - min_x, max_y - min_y))
    } else {
        None
    }
}

fn path_anchors_bbox(
    anchors: &[PathAnchor],
    page_w: f64,
    page_h: f64,
) -> Option<(f64, f64, f64, f64)> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut any = false;
    for anchor in anchors {
        for (x, y) in [
            (&anchor.x, &anchor.y),
            (&anchor.in_x, &anchor.in_y),
            (&anchor.out_x, &anchor.out_y),
        ] {
            if let (Some(px_val), Some(py_val)) = (
                x.as_ref().and_then(|d| resolve_axis(d, page_w)),
                y.as_ref().and_then(|d| resolve_axis(d, page_h)),
            ) {
                extend_bbox(
                    &mut min_x, &mut min_y, &mut max_x, &mut max_y, px_val, py_val,
                );
                any = true;
            }
        }
    }
    if any {
        Some((min_x, min_y, max_x - min_x, max_y - min_y))
    } else {
        None
    }
}

fn extend_bbox(
    min_x: &mut f64,
    min_y: &mut f64,
    max_x: &mut f64,
    max_y: &mut f64,
    px_val: f64,
    py_val: f64,
) {
    *min_x = min_x.min(px_val);
    *min_y = min_y.min(py_val);
    *max_x = max_x.max(px_val);
    *max_y = max_y.max(py_val);
}

/// Read the authored rotation of a node in degrees, if the node carries a
/// `rotate` field and the stored unit is `Deg`.
///
/// Returns `Some(degrees)` for rotate-bearing node kinds when the stored unit
/// is `Unit::Deg`. Returns `None` when the node has no `rotate` field, the
/// field is absent (`None`), or the unit is not `Deg` (e.g. an exotic unit
/// produced by forward-compat).
///
/// Covered (have a `rotate` field): `Rect`, `Ellipse`, `Frame`, `Image`,
/// `Text`, `Code`, `Group`, `Polygon`, `Polyline`, `Path`, `Table`, `Shape`,
/// `Connector`.
/// Not covered: `Line`, `Instance`, `Field`, `Footnote`, `Unknown`.
pub(in crate::validate::check) fn node_rotate_deg(node: &Node) -> Option<f64> {
    let dim = match node {
        Node::Rect(n) => n.rotate.as_ref(),
        Node::Ellipse(n) => n.rotate.as_ref(),
        Node::Frame(n) => n.rotate.as_ref(),
        Node::Image(n) => n.rotate.as_ref(),
        Node::Text(n) => n.rotate.as_ref(),
        Node::Code(n) => n.rotate.as_ref(),
        Node::Group(n) => n.rotate.as_ref(),
        Node::Polygon(n) => n.rotate.as_ref(),
        Node::Polyline(n) => n.rotate.as_ref(),
        Node::Path(n) => n.rotate.as_ref(),
        Node::Table(n) => n.rotate.as_ref(),
        Node::Shape(n) => n.rotate.as_ref(),
        Node::Connector(n) => n.rotate.as_ref(),
        Node::Pattern(n) => n.rotate.as_ref(),
        Node::Chart(n) => n.rotate.as_ref(),
        Node::Light(_) => None,
        Node::Mesh(_) => None,
        Node::Line(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Footnote(_)
        | Node::Unknown(_) => None,
    }?;
    (dim.unit == Unit::Deg).then_some(dim.value)
}

/// Extract the optional `role` attribute from any node variant.
///
/// Returns `None` for nodes that have no role set (or none in the AST at all,
/// such as `Unknown`). Used by the margin advisory to exempt `role="guide"`
/// nodes, which intentionally live in the page margins.
pub(in crate::validate::check) fn node_role(node: &Node) -> Option<&str> {
    match node {
        Node::Rect(n) => n.role.as_deref(),
        Node::Ellipse(n) => n.role.as_deref(),
        Node::Line(n) => n.role.as_deref(),
        Node::Text(n) => n.role.as_deref(),
        Node::Code(n) => n.role.as_deref(),
        Node::Frame(n) => n.role.as_deref(),
        Node::Group(n) => n.role.as_deref(),
        Node::Image(n) => n.role.as_deref(),
        Node::Polygon(n) => n.role.as_deref(),
        Node::Polyline(n) => n.role.as_deref(),
        Node::Path(n) => n.role.as_deref(),
        Node::Instance(n) => n.role.as_deref(),
        Node::Field(n) => n.role.as_deref(),
        Node::Toc(n) => n.role.as_deref(),
        Node::Footnote(n) => n.role.as_deref(),
        Node::Table(n) => n.role.as_deref(),
        Node::Shape(n) => n.role.as_deref(),
        Node::Connector(n) => n.role.as_deref(),
        Node::Pattern(n) => n.role.as_deref(),
        Node::Chart(n) => n.role.as_deref(),
        Node::Light(n) => n.role.as_deref(),
        Node::Mesh(n) => n.role.as_deref(),
        Node::Unknown(_) => None,
    }
}

/// Extract the string id and source span from any node variant.
pub(in crate::validate::check) fn node_id_and_span(
    node: &Node,
) -> (&str, Option<crate::ast::Span>) {
    match node {
        Node::Rect(n) => (&n.id, n.source_span),
        Node::Ellipse(n) => (&n.id, n.source_span),
        Node::Line(n) => (&n.id, n.source_span),
        Node::Text(n) => (&n.id, n.source_span),
        Node::Code(n) => (&n.id, n.source_span),
        Node::Frame(n) => (&n.id, n.source_span),
        Node::Group(n) => (&n.id, n.source_span),
        Node::Image(n) => (&n.id, n.source_span),
        Node::Polygon(n) => (&n.id, n.source_span),
        Node::Polyline(n) => (&n.id, n.source_span),
        Node::Path(n) => (&n.id, n.source_span),
        Node::Instance(n) => (&n.id, n.source_span),
        Node::Field(n) => (&n.id, n.source_span),
        Node::Toc(n) => (&n.id, n.source_span),
        Node::Footnote(n) => (&n.id, n.source_span),
        Node::Table(n) => (&n.id, n.source_span),
        Node::Shape(n) => (&n.id, n.source_span),
        Node::Connector(n) => (&n.id, n.source_span),
        Node::Pattern(n) => (&n.id, n.source_span),
        Node::Chart(n) => (&n.id, n.source_span),
        Node::Light(n) => (&n.id, n.source_span),
        Node::Mesh(n) => (&n.id, n.source_span),
        Node::Unknown(n) => (n.id.as_deref().unwrap_or(&n.kind), n.source_span),
    }
}
