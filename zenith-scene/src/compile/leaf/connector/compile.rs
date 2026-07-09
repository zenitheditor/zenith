//! The `connector` leaf compiler: resolves both endpoint boxes, routes the
//! path, and emits the stroke, arrowheads, and optional owned label — surfacing
//! `connector.anchor_unresolved` and `connector.unsupported_outline` diagnostics
//! along the way.

use std::collections::BTreeMap;

use zenith_core::ast::{ConnectorAnchor, parse_connector_anchor};
use zenith_core::{ConnectorNode, Diagnostic, FontProvider, ResolvedToken, Style};
use zenith_layout::RustybuzzEngine;

use crate::compile::field::{ConnectorTargetKind, PortTarget};
use crate::ir::{FillRule, Paint, SceneCommand, StrokeAlign};

use super::super::super::RenderCtx;
use super::super::super::anchor::AnchorMap;
use super::super::super::chain::ChainAssignments;
use super::super::super::paint::resolve_property_color;
use super::super::super::style_prop;
use super::super::super::util::{resolve_property_dimension_px, rotation_degrees};
use super::super::poly::flat_points_centroid_center;
use super::super::routing;
use super::anchor::resolve_anchor;
use super::label::emit_connector_label;
use super::route::{
    arrowhead_points, loop_side, orthogonal_route, outward_dir, point_at, self_loop_path,
};

/// Obstacle-clearance margin (px) used by `route="avoid"`: obstacles inflate by
/// this amount and the path stubs out of each box face by the same distance, so
/// the routed line keeps a small gap from every box it skirts.
const ROUTE_MARGIN: f64 = 8.0;

/// Read-only borrow + scalar context for [`compile_connector`].
///
/// Bundles the maps and the per-subtree [`RenderCtx`] so the connector compiler
/// stays under the argument-count lint without an `#[allow]`. All fields are
/// borrows/`Copy` scalars held for the duration of a single compile call.
///
/// The font/engine/chains/footnote_markers/anchors fields are needed for the
/// optional owned label — they mirror the same fields in [`ShapeCompileEnv`]
/// and are threaded through from the page-level compile context.
#[derive(Clone, Copy)]
pub(in crate::compile) struct ConnectorEnv<'a> {
    pub(in crate::compile) resolved: &'a BTreeMap<String, ResolvedToken>,
    pub(in crate::compile) style_map: &'a BTreeMap<&'a str, &'a Style>,
    pub(in crate::compile) fonts: &'a dyn FontProvider,
    pub(in crate::compile) engine: &'a RustybuzzEngine,
    pub(in crate::compile) chains: &'a ChainAssignments,
    pub(in crate::compile) footnote_markers: &'a BTreeMap<String, String>,
    pub(in crate::compile) node_boxes: &'a BTreeMap<String, (f64, f64, f64, f64)>,
    /// CONNECTOR-SCOPED outline-box fallback: node id → ABSOLUTE bounds rect for
    /// polygon/polyline/path targets not present in `node_boxes`. Consulted only
    /// after `node_boxes` misses (see [`endpoint_box`]).
    pub(in crate::compile) connector_outline_boxes: &'a BTreeMap<String, (f64, f64, f64, f64)>,
    pub(in crate::compile) connector_target_kinds: &'a BTreeMap<String, ConnectorTargetKind>,
    pub(in crate::compile) port_map: &'a BTreeMap<String, BTreeMap<String, PortTarget>>,
    pub(in crate::compile) anchors: &'a AnchorMap,
    pub(in crate::compile) ctx: RenderCtx,
}

fn split_endpoint(endpoint: &str) -> (&str, Option<&str>) {
    match endpoint.split_once('#') {
        Some((node, port)) if !node.is_empty() && !port.is_empty() => (node, Some(port)),
        _ => (endpoint, None),
    }
}

fn endpoint_target<'a>(
    endpoint_node_id: &'a str,
    port_id: Option<&str>,
    explicit_anchor: &'a str,
    port_map: &'a BTreeMap<String, BTreeMap<String, PortTarget>>,
) -> (&'a str, &'a str) {
    let Some(target) = port_id.and_then(|id| {
        port_map
            .get(endpoint_node_id)
            .and_then(|ports| ports.get(id))
    }) else {
        return (endpoint_node_id, explicit_anchor);
    };
    (target.node_id.as_str(), target.anchor.as_str())
}

/// Emit `connector.unsupported_outline` (Warning) when a **divided** (`i/N`)
/// anchor targets a node whose exact outline is not modeled — a rounded rect,
/// a `shape kind="process"`, or a polygon/polyline/path ([`ConnectorTargetKind::
/// ApproxOutline`]). The divided point is approximated on the bounds perimeter;
/// named/`auto` anchors on these shapes are the intended bounds semantics and do
/// NOT warn.
fn warn_unsupported_outline(
    connector: &ConnectorNode,
    label: &str,
    node_id: &str,
    anchor: &str,
    kind: ConnectorTargetKind,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if kind != ConnectorTargetKind::ApproxOutline {
        return;
    }
    if !matches!(
        parse_connector_anchor(anchor),
        Ok(ConnectorAnchor::Divided { .. })
    ) {
        return;
    }
    diagnostics.push(Diagnostic::warning(
        "connector.unsupported_outline",
        format!(
            "connector '{}': {label}-anchor '{anchor}' is a divided anchor on node '{node_id}', \
             whose exact outline is not modeled; approximating on the bounds perimeter",
            connector.id
        ),
        connector.source_span,
        Some(connector.id.clone()),
    ));
}

/// Compile a `connector` leaf node — a semantic arrow whose endpoints are
/// DERIVED at compile time from the resolved boxes of its `from`/`to` targets.
///
/// Unit 1 draws a STRAIGHT 2-point line between the resolved edge anchors. Unit 2
/// adds filled-triangle arrowheads at the `to` end (`marker-end="arrow"`) and/or
/// the `from` end (`marker-start="arrow"`), in the line's stroke color and inside
/// the same rotation bracket. Unit 3 adds `route="orthogonal"` — a right-angle
/// elbow path (4-point Z-route or 3-point L-corner) instead of the straight
/// diagonal — and orients arrowheads along the actual first/last routed segment
/// so they land axis-aligned. When `from`/`to` is absent nothing is emitted
/// (graceful — validation warned). When a target box is unresolvable a
/// `connector.anchor_unresolved` error is emitted for the failing endpoint and
/// the connector is skipped; markers follow the same guards.
///
/// When the connector carries `span` children an owned label is emitted at the
/// geometric midpoint of the routed polyline (see [`emit_connector_label`]).
/// A connector without spans renders exactly as before — byte-identical output.
pub(in crate::compile) fn compile_connector(
    connector: &ConnectorNode,
    commands: &mut Vec<SceneCommand>,
    diagnostics: &mut Vec<Diagnostic>,
    env: ConnectorEnv,
) {
    // ConnectorEnv is Copy; bind individual fields for use in this function
    // while keeping `env` available to pass to emit_connector_label at the end.
    let ConnectorEnv {
        resolved,
        style_map,
        node_boxes,
        connector_outline_boxes,
        connector_target_kinds,
        port_map,
        ctx,
        ..
    } = env;

    if connector.visible == Some(false) {
        return;
    }

    // Both endpoints are required to route; absent → emit nothing (validation
    // already warned via `connector.missing_target`).
    let (Some(from_endpoint), Some(to_endpoint)) =
        (connector.from.as_deref(), connector.to.as_deref())
    else {
        return;
    };
    let (from_id, from_port) = split_endpoint(from_endpoint);
    let (to_id, to_port) = split_endpoint(to_endpoint);
    let (from_id, from_anchor) = endpoint_target(
        from_id,
        from_port,
        connector.from_anchor.as_deref().unwrap_or("auto"),
        port_map,
    );
    let (to_id, to_anchor) = endpoint_target(
        to_id,
        to_port,
        connector.to_anchor.as_deref().unwrap_or("auto"),
        port_map,
    );

    // Look up the resolved page-absolute boxes of both targets. `node_boxes`
    // (rectangular routing boxes, also used by text runaround) is consulted
    // FIRST; a polygon/polyline/path target lives only in the connector-scoped
    // `connector_outline_boxes` fallback (its bounds-perimeter rect). Only when
    // BOTH miss is the endpoint genuinely geometry-less: emit
    // `connector.anchor_unresolved` naming the failing endpoint, then skip.
    let from_box = endpoint_box(from_id, node_boxes, connector_outline_boxes);
    let to_box = endpoint_box(to_id, node_boxes, connector_outline_boxes);
    if from_box.is_none() {
        emit_anchor_unresolved(connector, "from", from_id, diagnostics);
    }
    if to_box.is_none() {
        emit_anchor_unresolved(connector, "to", to_id, diagnostics);
    }
    let (Some(from_box), Some(to_box)) = (from_box, to_box) else {
        return;
    };
    let from_box = *from_box;
    let to_box = *to_box;

    let from_center = (from_box.0 + from_box.2 / 2.0, from_box.1 + from_box.3 / 2.0);
    let to_center = (to_box.0 + to_box.2 / 2.0, to_box.1 + to_box.3 / 2.0);
    let from_kind = connector_target_kinds
        .get(from_id)
        .copied()
        .unwrap_or(ConnectorTargetKind::BoxLike);
    let to_kind = connector_target_kinds
        .get(to_id)
        .copied()
        .unwrap_or(ConnectorTargetKind::BoxLike);

    // A divided anchor on a node whose true outline is not modeled falls back to
    // the bounds perimeter — surface that approximation as a Warning (named/auto
    // anchors on the same shapes are the intended bounds semantics, no warning).
    warn_unsupported_outline(
        connector,
        "from",
        from_id,
        from_anchor,
        from_kind,
        diagnostics,
    );
    warn_unsupported_outline(connector, "to", to_id, to_anchor, to_kind, diagnostics);

    // Resolve anchors: each end aims toward the OTHER box's center for "auto".
    let (f_pt, f_side) = resolve_anchor(from_box, from_kind, from_anchor, to_center);
    let (t_pt, t_side) = resolve_anchor(to_box, to_kind, to_anchor, from_center);

    // A self-loop (`from` and `to` name the same node) cannot be a line between
    // two distinct points — it routes as a small rectangular loop off one edge of
    // the box (the side picked from the `from`/`to` anchor, defaulting to the
    // top), with the marker landing back on that edge.
    //
    // Otherwise route selection applies: `orthogonal` builds a right-angle elbow
    // path; `avoid` runs an obstacle-avoiding orthogonal router around the other
    // boxes (and falls back to the plain elbow when no clear path exists);
    // everything else (None / "straight" / unknown — validation already warned)
    // is the straight 2-point line, byte-identical to Unit 1/2.
    let flat_points = if from_id == to_id {
        let side = loop_side(
            connector
                .from_anchor
                .as_deref()
                .or(connector.to_anchor.as_deref()),
        );
        self_loop_path(from_box, side)
    } else {
        match connector.route.as_deref() {
            Some("orthogonal") => orthogonal_route(f_pt, f_side, t_pt, t_side),
            Some("avoid") => {
                let obstacles: Vec<(f64, f64, f64, f64)> = node_boxes
                    .iter()
                    .filter(|(id, _)| id.as_str() != from_id && id.as_str() != to_id)
                    .map(|(_, b)| *b)
                    .collect();
                let f_out = outward_dir(f_side, f_pt, from_box);
                let t_out = outward_dir(t_side, t_pt, to_box);
                routing::route_orthogonal_avoiding(
                    f_pt,
                    f_out,
                    t_pt,
                    t_out,
                    &obstacles,
                    ROUTE_MARGIN,
                )
                .unwrap_or_else(|| orthogonal_route(f_pt, f_side, t_pt, t_side))
            }
            _ => vec![f_pt.0, f_pt.1, t_pt.0, t_pt.1],
        }
    };

    // STROKE — only emit when a stroke color is present (mirrors polyline: no
    // stroke token → nothing drawn). Style cascade for stroke + stroke-width.
    let node_opacity = connector.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
    let stroke_prop = connector
        .stroke
        .as_ref()
        .or_else(|| style_prop(&connector.style, style_map, "stroke"));
    let Some(stroke_prop) = stroke_prop else {
        return;
    };
    let Some(mut color) = resolve_property_color(stroke_prop, resolved, diagnostics, &connector.id)
    else {
        return;
    };
    color.a = (color.a as f64 * node_opacity * ctx.opacity).round() as u8;

    let sw = connector
        .stroke_width
        .clone()
        .or_else(|| style_prop(&connector.style, style_map, "stroke-width").cloned());
    let stroke_width = resolve_property_dimension_px(sw.as_ref(), resolved, 1.0);

    // Rotation bracket: rotate about the line's bbox center, matching polyline.
    let rot = rotation_degrees(connector.rotate.as_ref());
    if let Some(angle) = rot {
        let (cx, cy) = flat_points_centroid_center(&flat_points);
        commands.push(SceneCommand::PushTransform {
            angle_deg: angle,
            cx,
            cy,
        });
    }

    // Derive marker endpoints from the ACTUAL routed path BEFORE the Vec is moved
    // into the stroke command, so orthogonal arrowheads orient along the real
    // last/first segment (axis-aligned), not the global anchor line. For a
    // 2-point straight line these reduce to today's (tx,ty)/(fx,fy) endpoints.
    let n = flat_points.len() / 2;
    let end_tip = point_at(&flat_points, n.saturating_sub(1));
    let end_from = point_at(&flat_points, n.saturating_sub(2));
    let start_tip = point_at(&flat_points, 0);
    let start_from = point_at(&flat_points, 1);

    commands.push(SceneCommand::StrokePolyline {
        points: flat_points.clone(),
        color,
        stroke_width,
        closed: false,
        align: StrokeAlign::Center,
        clip_fill_rule: FillRule::NonZero,
    });

    // ARROWHEAD MARKERS (Unit 2/3) — filled triangles in the SAME stroke color,
    // INSIDE the rotation bracket so they rotate with the line. The tip sits
    // exactly on the path endpoint; the base extends back along the adjacent
    // segment. Fewer than 2 points → endpoints are `None` and markers are skipped.
    {
        let mut emit_head = |tip, from_pt| {
            if let Some(points) = arrowhead_points(tip, from_pt, stroke_width) {
                commands.push(SceneCommand::FillPolygon {
                    points,
                    paint: Paint::solid(color),
                    fill_rule: FillRule::NonZero,
                });
            }
        };
        if connector.marker_end.as_deref() == Some("arrow")
            && let (Some(tip), Some(from_pt)) = (end_tip, end_from)
        {
            emit_head(tip, from_pt);
        }
        if connector.marker_start.as_deref() == Some("arrow")
            && let (Some(tip), Some(from_pt)) = (start_tip, start_from)
        {
            emit_head(tip, from_pt);
        }
    }

    if rot.is_some() {
        commands.push(SceneCommand::PopTransform);
    }

    // OWNED LABEL — emitted OUTSIDE the rotation bracket so the label text is
    // not rotated with the line (branch labels like "Yes"/"No" stay readable
    // regardless of line orientation). The midpoint is computed in page-absolute
    // coordinates before the rotation bracket, so this is correct.
    emit_connector_label(connector, &flat_points, commands, diagnostics, env);
}

/// Resolve an endpoint's routing box: `node_boxes` (rectangular boxes, shared
/// with text runaround) takes precedence, falling back to the connector-scoped
/// `connector_outline_boxes` (polygon/polyline/path bounds perimeter). `None`
/// only when the id is in NEITHER — a genuinely geometry-less target.
fn endpoint_box<'a>(
    node_id: &str,
    node_boxes: &'a BTreeMap<String, (f64, f64, f64, f64)>,
    connector_outline_boxes: &'a BTreeMap<String, (f64, f64, f64, f64)>,
) -> Option<&'a (f64, f64, f64, f64)> {
    node_boxes
        .get(node_id)
        .or_else(|| connector_outline_boxes.get(node_id))
}

/// Emit a `connector.anchor_unresolved` (Error) for an endpoint whose target box
/// could not be resolved (unknown id, or a target with no authored geometry).
fn emit_anchor_unresolved(
    connector: &ConnectorNode,
    label: &str,
    node_id: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(Diagnostic::error(
        "connector.anchor_unresolved",
        format!(
            "connector '{}': {label} endpoint '{node_id}' could not be resolved to a box \
             (unknown node id or a target with no authored geometry); the connector is skipped",
            connector.id
        ),
        connector.source_span,
        Some(connector.id.clone()),
    ));
}
