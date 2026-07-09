//! Connector-target lookups: shape family per node id, plus a connector-scoped
//! outline-box map for targets whose exact geometry is not a rectangular box.

use std::collections::BTreeMap;

use zenith_core::{ComponentDef, InstanceNode, Node, Point, ResolvedToken, dim_to_px};

use super::common::{node_id, resolve_imported_component};
use crate::compile::container::prefix_ids_in_children;
use crate::compile::imports::ImportScopes;
use crate::compile::leaf::path_outline_bounds;
use crate::compile::util::resolve_geometry_px;

/// Shape family for connector divided-anchor perimeter resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::compile) enum ConnectorTargetKind {
    BoxLike,
    Capsule,
    Diamond,
    Ellipse,
    /// A node whose exact outline is not modeled by the divided-anchor router
    /// (rounded rect, `shape kind="process"`, polygon/polyline/path). Divided
    /// anchors on these resolve on the bounds PERIMETER exactly like
    /// [`ConnectorTargetKind::BoxLike`] — the approximation is intentional and
    /// byte-identical to the pre-existing box fallback. The distinct variant
    /// exists only so the connector compiler can surface a
    /// `connector.unsupported_outline` warning for a divided anchor here.
    ApproxOutline,
}

/// A page's connector-target lookups: shape family per id, plus a
/// CONNECTOR-SCOPED outline-box map for targets whose exact geometry is not a
/// rectangular box in [`crate::compile::field::projection::build_node_boxes`].
///
/// `outline_boxes` is kept separate from `node_boxes` on purpose: `node_boxes`
/// also drives text runaround and must not gain polygon/path entries (that would
/// silently change text layout). Connectors consult `node_boxes` first and fall
/// back to `outline_boxes`, so an unmodeled outline attaches at its bounds
/// perimeter instead of erroring.
#[derive(Debug, Default)]
pub(in crate::compile) struct ConnectorTargets {
    pub(in crate::compile) kinds: BTreeMap<String, ConnectorTargetKind>,
    pub(in crate::compile) outline_boxes: BTreeMap<String, (f64, f64, f64, f64)>,
}

/// Build a page's connector-target lookups, keyed by node id.
///
/// The first occurrence of an id wins, matching
/// [`crate::compile::field::projection::build_node_boxes`]. An id already present
/// in `node_boxes` takes its box from there (and only gets a kind here); an id
/// NOT in `node_boxes` but carrying a resolvable free-form outline
/// (polygon/polyline/path) is recorded in `outline_boxes` with its ABSOLUTE
/// bounds rect and the [`ConnectorTargetKind::ApproxOutline`] kind.
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
        if let Some(id) = node_id(child) {
            if node_boxes.contains_key(id) {
                // Already a rectangular routing box; record only its kind.
                targets
                    .kinds
                    .entry(id.to_owned())
                    .or_insert(connector_target_kind(child));
            } else if let Some((x, y, w, h)) = connector_outline_rect(child, resolved) {
                // Not in node_boxes but has a free-form outline: register its
                // ABSOLUTE bounds box and its (ApproxOutline) kind, for the
                // connector's outline-fallback lookup.
                targets
                    .outline_boxes
                    .entry(id.to_owned())
                    .or_insert((dx + x, dy + y, w, h));
                targets
                    .kinds
                    .entry(id.to_owned())
                    .or_insert(connector_target_kind(child));
            }
        }
        match child {
            // A frame is clip-only: its children are NOT translated by its origin.
            Node::Frame(f) => {
                collect_connector_targets(&f.children, dx, dy, env, targets);
            }
            // A group translates its children by its own x/y (absent/bad-unit → 0).
            Node::Group(g) => {
                let gx = resolve_geometry_px(g.x.as_ref(), resolved).unwrap_or(0.0);
                let gy = resolve_geometry_px(g.y.as_ref(), resolved).unwrap_or(0.0);
                collect_connector_targets(&g.children, dx + gx, dy + gy, env, targets);
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

/// The LOCAL axis-aligned bounds rect `(x, y, w, h)` a connector uses when a
/// target is NOT a rectangular
/// [`crate::compile::field::projection::build_node_boxes`] box. Only free-form
/// vector outlines yield a rect:
///
/// - `polygon`/`polyline`: the bounding box of the resolvable `point`s.
/// - `path`: the EXTREMA-AWARE bounds (true cubic-curve extent, unioned across
///   compound subpaths), via [`path_outline_bounds`].
///
/// Every other node kind returns `None` — a rectangular node already lives in
/// `node_boxes`, and a geometry-less node (e.g. `light`) legitimately has no box,
/// so the connector reports it unresolved.
fn connector_outline_rect(
    node: &Node,
    _resolved: &BTreeMap<String, ResolvedToken>,
) -> Option<(f64, f64, f64, f64)> {
    match node {
        Node::Polygon(n) => points_bounds(&n.points),
        Node::Polyline(n) => points_bounds(&n.points),
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

/// The bounding box `(x, y, w, h)` of an ordered `point` list, skipping any point
/// whose x/y does not resolve to pixels. `None` when no point is finite.
fn points_bounds(points: &[Point]) -> Option<(f64, f64, f64, f64)> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for point in points {
        let (Some(xd), Some(yd)) = (&point.x, &point.y) else {
            continue;
        };
        let (Some(px), Some(py)) = (dim_to_px(xd.value, &xd.unit), dim_to_px(yd.value, &yd.unit))
        else {
            continue;
        };
        min_x = min_x.min(px);
        min_y = min_y.min(py);
        max_x = max_x.max(px);
        max_y = max_y.max(py);
    }
    if min_x.is_infinite() {
        return None;
    }
    Some((min_x, min_y, max_x - min_x, max_y - min_y))
}

fn connector_target_kind(node: &Node) -> ConnectorTargetKind {
    match node {
        Node::Ellipse(_) => ConnectorTargetKind::Ellipse,
        Node::Shape(n) if n.kind.as_deref() == Some("decision") => ConnectorTargetKind::Diamond,
        Node::Shape(n) if n.kind.as_deref() == Some("terminator") => ConnectorTargetKind::Capsule,
        Node::Shape(n) if n.kind.as_deref() == Some("ellipse") => ConnectorTargetKind::Ellipse,
        // A `process` shape is a rounded rect whose exact outline is not modeled;
        // divided anchors approximate on the bounds perimeter (see ApproxOutline).
        Node::Shape(n) if n.kind.as_deref() == Some("process") => {
            ConnectorTargetKind::ApproxOutline
        }
        // A rect with ANY corner radius (uniform or per-corner) is a rounded rect:
        // its true outline is not modeled, so divided anchors approximate.
        Node::Rect(n)
            if n.radius.is_some()
                || n.radius_tl.is_some()
                || n.radius_tr.is_some()
                || n.radius_br.is_some()
                || n.radius_bl.is_some() =>
        {
            ConnectorTargetKind::ApproxOutline
        }
        // Free-form vector outlines are not modeled; divided anchors approximate
        // on the bounding-box perimeter.
        Node::Polygon(_) | Node::Polyline(_) | Node::Path(_) => ConnectorTargetKind::ApproxOutline,
        Node::Rect(_)
        | Node::Text(_)
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
    let ix = instance
        .x
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit))
        .unwrap_or(0.0);
    let iy = instance
        .y
        .as_ref()
        .and_then(|d| dim_to_px(d.value, &d.unit))
        .unwrap_or(0.0);

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
