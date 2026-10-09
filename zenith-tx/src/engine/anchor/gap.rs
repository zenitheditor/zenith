//! `nudge_anchor_gap` application: move an edge-anchored node along its
//! anchor axis by changing `anchor-gap`, in the gap's unit.

use zenith_core::{AnchorEdge, Diagnostic, Document, parse_anchor_edge};

use super::super::geometry::{AxisCtx, dimension_px};
use super::super::layout::reject_layout_managed;
use super::super::{find_node_any_mut, find_node_any_shared, px, record_affected};
use super::fields::anchor_fields_mut;

const OP: &str = "nudge_anchor_gap";

pub(in crate::engine) fn apply_nudge_anchor_gap(
    node_id: &str,
    (dx, dy): (Option<f64>, Option<f64>),
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let error = |code: &str, message: String| {
        Diagnostic::error(
            code,
            format!("{OP}: {message}"),
            None,
            Some(node_id.to_owned()),
        )
    };
    for (axis, d) in [("dx", dx), ("dy", dy)] {
        if let Some(d) = d
            && !d.is_finite()
        {
            diagnostics.push(error(
                "tx.invalid_value",
                format!("{axis} {d} on node {node_id:?} is not finite"),
            ));
            return;
        }
    }
    if reject_layout_managed(doc, [node_id], OP, diagnostics) {
        return;
    }
    let Some(node) = find_node_any_shared(doc, node_id) else {
        diagnostics.push(error(
            "tx.unknown_node",
            format!("node {node_id:?} not found in document"),
        ));
        return;
    };
    let kind = node.kind_str();
    let Some(view) = node.anchor_view() else {
        diagnostics.push(error(
            "tx.unsupported_property",
            format!("{kind} {node_id:?} takes no anchor"),
        ));
        return;
    };
    let (Some(sibling), Some(edge_name)) = (view.anchor_sibling, view.anchor_edge) else {
        diagnostics.push(error(
            "tx.unsupported_property",
            format!(
                "node {node_id:?} has no anchor-sibling with anchor-edge, so it has no anchor \
                 gap. Move it with nudge_geometry, or set an edge anchor with set_anchor."
            ),
        ));
        return;
    };
    let Some(edge) = parse_anchor_edge(edge_name) else {
        diagnostics.push(error(
            "tx.invalid_value",
            format!("node {node_id:?} has anchor-edge {edge_name:?}, which is not an edge"),
        ));
        return;
    };
    // (main delta, sign, main attr, main authored, cross delta, cross attr, cross authored)
    let (main, sign, main_attr, main_set, cross, cross_attr, cross_set) = match edge {
        AnchorEdge::Below => (dy, 1.0, "y", view.y.is_some(), dx, "x", view.x.is_some()),
        AnchorEdge::Above => (dy, -1.0, "y", view.y.is_some(), dx, "x", view.x.is_some()),
        AnchorEdge::After => (dx, 1.0, "x", view.x.is_some(), dy, "y", view.y.is_some()),
        AnchorEdge::Before => (dx, -1.0, "x", view.x.is_some(), dy, "y", view.y.is_some()),
    };
    if let Some(c) = cross.filter(|c| *c != 0.0) {
        let diagnostic = if cross_set {
            error(
                "tx.invalid_value",
                format!(
                    "d{cross_attr} {c} on node {node_id:?} moves the authored {cross_attr}, not \
                     the anchor gap. Send it with nudge_geometry."
                ),
            )
        } else {
            error(
                "tx.anchored",
                format!(
                    "d{cross_attr} {c} on node {node_id:?} crosses the {edge_name} edge of \
                     {sibling:?}. Its {cross_attr} comes from the anchor alignment. Change the \
                     alignment with set_anchor anchor, or place the node by x/y with \
                     detach_anchor."
                ),
            )
        };
        diagnostics.push(diagnostic);
        return;
    }
    let Some(main) = main.filter(|m| *m != 0.0) else {
        diagnostics.push(Diagnostic::advisory(
            "tx.noop",
            format!("{OP} on {node_id:?} has no delta along the {edge_name} edge. The document is unchanged."),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    };
    if main_set {
        diagnostics.push(error(
            "tx.unsupported_property",
            format!(
                "node {node_id:?} has an authored {main_attr}, which overrides its anchor, so the \
                 gap does not move it. Send d{main_attr} with nudge_geometry."
            ),
        ));
        return;
    }
    let ctx = AxisCtx {
        op: OP,
        node_id,
        attr: "anchor-gap",
    };
    let current = view.anchor_gap.cloned().unwrap_or_else(|| px(0.0));
    let planned = ctx
        .offset(&current, sign * main)
        .and_then(|gap| match dimension_px(&gap) {
            Some(v) if v.is_finite() => Ok(gap),
            Some(_) | None => Err(ctx.error(
                "tx.invalid_geometry",
                format!("would become {}, which is not finite", gap.to_kdl_string()),
            )),
        });
    let gap = match planned {
        Ok(gap) => gap,
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            return;
        }
    };
    if let Some(fields) = find_node_any_mut(doc, node_id).and_then(anchor_fields_mut) {
        *fields.gap = Some(gap);
        record_affected(node_id, affected);
    }
}
