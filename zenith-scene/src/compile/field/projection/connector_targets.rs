//! Connector-target lookups: shape family per node id, outline-box map for
//! named/auto anchors on free-form targets, and exact geometry maps for
//! divided-anchor perimeter sampling (polygon / polyline / path).

use std::collections::BTreeMap;

use zenith_core::{ComponentDef, InstanceNode, Node, Point, ResolvedToken, dim_to_px};
use zenith_geometry::{CompoundFillRule, CompoundPathGeometry, Point2};

use super::common::{child_offset, resolve_imported_component};
use crate::compile::container::prefix_ids_in_children;
use crate::compile::imports::ImportScopes;
use crate::compile::leaf::{path_outline_bounds, path_to_compound_geometry};
use crate::compile::util::{points_bbox, resolve_geometry_px};

/// Shape family for connector divided-anchor perimeter resolution.
///
/// Free-form vector targets use exact outline kinds ([`ClosedRing`],
/// [`OpenPolyline`], [`PathOutline`]); named/`auto`/grid anchors still resolve
/// against the bounds box stored in [`ConnectorTargets::outline_boxes`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::compile) enum ConnectorTargetKind {
    BoxLike,
    Capsule,
    Diamond,
    Ellipse,
    /// A rounded rectangle with corner radii `(tl, tr, br, bl)` in pixels.
    /// Divided anchors walk the true perimeter (straight edges + circular
    /// corner arcs). Radii are clamped to `min(w, h) / 2` at resolution time.
    RoundedRect {
        tl: f64,
        tr: f64,
        br: f64,
        bl: f64,
    },
    /// A closed polygon ring. Divided anchors sample the true edge perimeter
    /// (see [`ConnectorTargets::closed_rings`]); named/auto stay bounds-based.
    ClosedRing,
    /// An open polyline. Divided anchors sample arc-length start→end
    /// (see [`ConnectorTargets::open_polylines`]); named/auto stay bounds-based.
    OpenPolyline,
    /// A structured path. Divided anchors sample the exterior closed outline
    /// (or open flatten walk when no closed exterior exists); named/auto stay
    /// bounds-based. Geometry lives in [`ConnectorTargets::path_geometries`].
    PathOutline,
}

/// Absolute page-space compound path geometry plus the authored fill rule,
/// used only by divided-anchor sampling on [`ConnectorTargetKind::PathOutline`].
#[derive(Debug, Clone)]
pub(in crate::compile) struct PathConnectorGeometry {
    pub(in crate::compile) geometry: CompoundPathGeometry,
    pub(in crate::compile) fill_rule: CompoundFillRule,
}

/// A page's connector-target lookups: shape family per id, a CONNECTOR-SCOPED
/// outline-box map for named/auto anchors on free-form targets, and exact
/// geometry maps for divided-anchor sampling.
///
/// `outline_boxes` is kept separate from `node_boxes` on purpose: `node_boxes`
/// also drives text runaround and must not gain polygon/path entries (that would
/// silently change text layout). Connectors consult `node_boxes` first and fall
/// back to `outline_boxes`, so an unmodeled outline attaches at its bounds
/// perimeter for named/auto instead of erroring.
#[derive(Debug, Default)]
pub(in crate::compile) struct ConnectorTargets {
    pub(in crate::compile) kinds: BTreeMap<String, ConnectorTargetKind>,
    pub(in crate::compile) outline_boxes: BTreeMap<String, (f64, f64, f64, f64)>,
    /// Absolute page-space closed rings for polygon divided anchors.
    pub(in crate::compile) closed_rings: BTreeMap<String, Vec<Point2>>,
    /// Absolute page-space open polylines for polyline divided anchors.
    pub(in crate::compile) open_polylines: BTreeMap<String, Vec<Point2>>,
    /// Absolute page-space path geometry for path divided anchors.
    pub(in crate::compile) path_geometries: BTreeMap<String, PathConnectorGeometry>,
}

/// Build a page's connector-target lookups, keyed by node id.
///
/// The first occurrence of an id wins, matching
/// [`crate::compile::field::projection::build_node_boxes`]. An id already present
/// in `node_boxes` takes its box from there (and only gets a kind here); an id
/// NOT in `node_boxes` but carrying a resolvable free-form outline
/// (polygon/polyline/path) is recorded in `outline_boxes` with its ABSOLUTE
/// bounds rect, the exact outline kind, and (when resolvable) exact geometry
/// for divided sampling.
pub(in crate::compile) fn build_connector_targets(
    page: &zenith_core::Page,
    node_boxes: &BTreeMap<String, (f64, f64, f64, f64)>,
    resolved: &BTreeMap<String, ResolvedToken>,
    components: &BTreeMap<&str, &ComponentDef>,
    imports: &ImportScopes<'_>,
) -> ConnectorTargets {
    let mut targets = ConnectorTargets::default();
    collect_connector_targets(
        &page.children,
        0.0,
        0.0,
        ConnectorTargetsEnv {
            node_boxes,
            resolved,
            components,
            imports,
        },
        &mut targets,
    );
    targets
}

/// Read-only borrow bundle for the connector-target walk, keeping the recursion
/// under the argument-count lint. `dx`/`dy` are threaded as separate scalars.
#[derive(Clone, Copy)]
struct ConnectorTargetsEnv<'a> {
    node_boxes: &'a BTreeMap<String, (f64, f64, f64, f64)>,
    resolved: &'a BTreeMap<String, ResolvedToken>,
    components: &'a BTreeMap<&'a str, &'a ComponentDef>,
    imports: &'a ImportScopes<'a>,
}

fn collect_connector_targets(
    children: &[Node],
    dx: f64,
    dy: f64,
    env: ConnectorTargetsEnv<'_>,
    targets: &mut ConnectorTargets,
) {
    let ConnectorTargetsEnv {
        node_boxes,
        resolved,
        components: _,
        imports: _,
    } = env;
    for child in children {
        if let Some(id) = child.id() {
            if node_boxes.contains_key(id) {
                // Already a rectangular routing box; record only its kind.
                targets
                    .kinds
                    .entry(id.to_owned())
                    .or_insert(connector_target_kind(child, resolved));
            } else if let Some((x, y, w, h)) = connector_outline_rect(child) {
                // Not in node_boxes but has a free-form outline: register its
                // ABSOLUTE bounds box (named/auto), exact kind, and geometry for
                // divided-anchor sampling.
                targets
                    .outline_boxes
                    .entry(id.to_owned())
                    .or_insert((dx + x, dy + y, w, h));
                targets
                    .kinds
                    .entry(id.to_owned())
                    .or_insert(connector_target_kind(child, resolved));
                insert_exact_outline_geometry(child, id, dx, dy, targets);
            }
        }
        match child {
            // A frame and a group add their child space to their children.
            Node::Frame(f) => {
                let (cdx, cdy) = child_offset(child, dx, dy, resolved);
                collect_connector_targets(&f.children, cdx, cdy, env, targets);
            }
            Node::Group(g) => {
                let (cdx, cdy) = child_offset(child, dx, dy, resolved);
                collect_connector_targets(&g.children, cdx, cdy, env, targets);
            }
            Node::Instance(i) => {
                collect_instance_connector_targets(i, dx, dy, env, targets);
            }
            Node::Table(_)
            | Node::Rect(_)
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
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_)
            | Node::Unknown(_) => {}
        }
    }
}

/// Record absolute exact-outline geometry for divided anchors. First id wins.
fn insert_exact_outline_geometry(
    node: &Node,
    id: &str,
    dx: f64,
    dy: f64,
    targets: &mut ConnectorTargets,
) {
    match node {
        Node::Polygon(n) => {
            if let Some(points) = absolute_poly_points(&n.points, dx, dy)
                && points.len() >= 3
            {
                targets.closed_rings.entry(id.to_owned()).or_insert(points);
            }
        }
        Node::Polyline(n) => {
            if let Some(points) = absolute_poly_points(&n.points, dx, dy)
                && points.len() >= 2
            {
                targets
                    .open_polylines
                    .entry(id.to_owned())
                    .or_insert(points);
            }
        }
        Node::Path(n) => {
            if let Some(geometry) = path_to_compound_geometry(n, dx, dy) {
                let fill_rule = path_fill_rule(n.fill_rule.as_deref());
                targets
                    .path_geometries
                    .entry(id.to_owned())
                    .or_insert(PathConnectorGeometry {
                        geometry,
                        fill_rule,
                    });
            }
        }
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Line(_)
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
        | Node::Unknown(_) => {}
    }
}

/// Absolute page-space `Point2` list from authored poly points. Skips points
/// whose x/y does not resolve (same filter as bounds). `None` when no point is
/// finite.
fn absolute_poly_points(points: &[Point], dx: f64, dy: f64) -> Option<Vec<Point2>> {
    let mut out = Vec::with_capacity(points.len());
    for point in points {
        let (Some(xd), Some(yd)) = (&point.x, &point.y) else {
            continue;
        };
        let (Some(px), Some(py)) = (dim_to_px(xd.value, &xd.unit), dim_to_px(yd.value, &yd.unit))
        else {
            continue;
        };
        let Ok(p) = Point2::new(px + dx, py + dy) else {
            continue;
        };
        out.push(p);
    }
    if out.is_empty() { None } else { Some(out) }
}

fn path_fill_rule(authored: Option<&str>) -> CompoundFillRule {
    match authored {
        Some("evenodd") => CompoundFillRule::EvenOdd,
        _ => CompoundFillRule::NonZero,
    }
}

/// The LOCAL axis-aligned bounds rect `(x, y, w, h)` a connector uses when a
/// target is NOT a rectangular
/// [`crate::compile::field::projection::build_node_boxes`] box. Only free-form
/// vector outlines yield a rect:
///
/// - `polygon`/`polyline`: the bounding box of the resolvable `point`s
///   ([`points_bbox`]).
/// - `path`: the EXTREMA-AWARE bounds (true cubic-curve extent, unioned across
///   compound subpaths), via [`path_outline_bounds`].
///
/// Every other node kind returns `None` — a rectangular node already lives in
/// `node_boxes`, and a geometry-less node (e.g. `light`) legitimately has no box,
/// so the connector reports it unresolved.
fn connector_outline_rect(node: &Node) -> Option<(f64, f64, f64, f64)> {
    match node {
        Node::Polygon(n) => points_bbox(&n.points),
        Node::Polyline(n) => points_bbox(&n.points),
        Node::Path(n) => path_outline_bounds(n),
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Line(_)
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
        | Node::Unknown(_) => None,
    }
}

fn connector_target_kind(
    node: &Node,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> ConnectorTargetKind {
    match node {
        Node::Ellipse(_) => ConnectorTargetKind::Ellipse,
        Node::Shape(n) if n.kind.as_deref() == Some("decision") => ConnectorTargetKind::Diamond,
        Node::Shape(n) if n.kind.as_deref() == Some("terminator") => ConnectorTargetKind::Capsule,
        Node::Shape(n) if n.kind.as_deref() == Some("ellipse") => ConnectorTargetKind::Ellipse,
        // A `process` shape is a (possibly rounded) rect. When radius > 0, walk
        // the true rounded perimeter; otherwise BoxLike (sharp bounds).
        Node::Shape(n) if n.kind.as_deref() == Some("process") => {
            match shape_uniform_radius(n, resolved) {
                Some(r) if r > 0.0 => ConnectorTargetKind::RoundedRect {
                    tl: r,
                    tr: r,
                    br: r,
                    bl: r,
                },
                _ => ConnectorTargetKind::BoxLike,
            }
        }
        // A rect with any positive corner radius walks the true rounded perimeter.
        Node::Rect(n) => match rect_corner_radii(n, resolved) {
            Some((tl, tr, br, bl)) => ConnectorTargetKind::RoundedRect { tl, tr, br, bl },
            None => ConnectorTargetKind::BoxLike,
        },
        Node::Polygon(_) => ConnectorTargetKind::ClosedRing,
        Node::Polyline(_) => ConnectorTargetKind::OpenPolyline,
        Node::Path(_) => ConnectorTargetKind::PathOutline,
        Node::Text(_)
        | Node::Code(_)
        | Node::Frame(_)
        | Node::Group(_)
        | Node::Image(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Table(_)
        | Node::Shape(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Mesh(_) => ConnectorTargetKind::BoxLike,
        Node::Line(_)
        | Node::Instance(_)
        | Node::Footnote(_)
        | Node::Connector(_)
        | Node::Light(_)
        | Node::Unknown(_) => ConnectorTargetKind::BoxLike,
    }
}

/// Resolve uniform radius on a process shape (px). `None` when absent/unresolvable.
fn shape_uniform_radius(
    shape: &zenith_core::ShapeNode,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Option<f64> {
    resolve_geometry_px(shape.radius.as_ref(), resolved)
}

/// Resolve corner radii for a rect. Returns `Some((tl, tr, br, bl))` when at
/// least one corner is positive; `None` for a sharp box (all corners 0 / absent).
fn rect_corner_radii(
    rect: &zenith_core::RectNode,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Option<(f64, f64, f64, f64)> {
    let has_any = rect.radius.is_some()
        || rect.radius_tl.is_some()
        || rect.radius_tr.is_some()
        || rect.radius_br.is_some()
        || rect.radius_bl.is_some();
    if !has_any {
        return None;
    }
    let uniform = resolve_geometry_px(rect.radius.as_ref(), resolved).unwrap_or(0.0);
    let tl = resolve_geometry_px(rect.radius_tl.as_ref(), resolved).unwrap_or(uniform);
    let tr = resolve_geometry_px(rect.radius_tr.as_ref(), resolved).unwrap_or(uniform);
    let br = resolve_geometry_px(rect.radius_br.as_ref(), resolved).unwrap_or(uniform);
    let bl = resolve_geometry_px(rect.radius_bl.as_ref(), resolved).unwrap_or(uniform);
    if tl <= 0.0 && tr <= 0.0 && br <= 0.0 && bl <= 0.0 {
        return None;
    }
    Some((tl.max(0.0), tr.max(0.0), br.max(0.0), bl.max(0.0)))
}

fn collect_instance_connector_targets(
    instance: &InstanceNode,
    dx: f64,
    dy: f64,
    env: ConnectorTargetsEnv<'_>,
    targets: &mut ConnectorTargets,
) {
    let ConnectorTargetsEnv {
        node_boxes,
        resolved: _,
        components: _,
        imports,
    } = env;
    let (ix, iy) = instance.child_space();

    if let Some(source) = instance.source.as_deref() {
        // An imported instance resolves its children against the IMPORTED scope's
        // token table (not the host page's), mirroring `collect_node_boxes`.
        let Some((imported, component)) = resolve_imported_component(source, imports) else {
            return;
        };
        let mut children = component.children.clone();
        let prefix = format!("{}/", instance.id);
        prefix_ids_in_children(&mut children, &prefix);
        collect_connector_targets(
            &children,
            dx + ix,
            dy + iy,
            ConnectorTargetsEnv {
                node_boxes,
                resolved: &imported.resolved,
                components: &imported.components,
                imports,
            },
            targets,
        );
        return;
    }

    let Some(component_id) = instance.component.as_deref() else {
        return;
    };
    let Some(component) = env.components.get(component_id) else {
        return;
    };
    let mut children = component.children.clone();
    let prefix = format!("{}/", instance.id);
    prefix_ids_in_children(&mut children, &prefix);
    collect_connector_targets(&children, dx + ix, dy + iy, env, targets);
}
