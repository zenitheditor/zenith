//! `detach_anchor` application: remove the anchor attributes of a node and
//! write each anchor-supplied `x` / `y` as px, so the node keeps its place.

use zenith_core::{Diagnostic, Document, PropertyValue};

use super::super::geometry::anchored;
use super::super::layout::in_layout_flow;
use super::super::space::{anchor_origin_of, resolved_tokens};
use super::super::{find_node_any_mut, find_node_any_shared, px, record_affected};
use super::fields::anchor_fields_mut;

const OP: &str = "detach_anchor";

pub(in crate::engine) fn apply_detach_anchor(
    node_id: &str,
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
    let has_anchor = view.anchor.is_some()
        || view.anchor_zone.is_some()
        || view.anchor_sibling.is_some()
        || view.anchor_parent.is_some()
        || view.anchor_edge.is_some()
        || view.anchor_gap.is_some();
    if !has_anchor {
        diagnostics.push(Diagnostic::advisory(
            "tx.noop",
            format!("{OP}: node {node_id:?} has no anchor attributes. The document is unchanged."),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    }
    let needs_x = view.x.is_none();
    let needs_y = view.y.is_none();
    // An in-flow node takes its x/y from the frame, and an anchor that
    // places nothing supplies no axis: both only drop the attributes.
    let places = anchored(node) && !in_layout_flow(doc, node_id) && (needs_x || needs_y);
    let origin = if places {
        match anchor_origin_of(doc, node_id, &resolved_tokens(doc)) {
            Some(origin) => Some(origin),
            None => {
                diagnostics.push(error(
                    "tx.anchored",
                    format!(
                        "the anchor position of node {node_id:?} does not derive from authored \
                         values (a reference box, size, or container origin is not px). Write \
                         x/y with set_geometry from the measured box, then clear the anchor \
                         with set_anchor."
                    ),
                ));
                return;
            }
        }
    } else {
        None
    };
    let Some(mut fields) = find_node_any_mut(doc, node_id).and_then(anchor_fields_mut) else {
        return;
    };
    if let Some((x, y)) = origin {
        if needs_x {
            *fields.x = Some(PropertyValue::Dimension(px(x)));
        }
        if needs_y {
            *fields.y = Some(PropertyValue::Dimension(px(y)));
        }
    }
    fields.clear();
    record_affected(node_id, affected);
}
