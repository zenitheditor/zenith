//! Per-kind checks for `shape`, `connector`, and `unknown` nodes.
//!
//! `check_unknown` emits the unknown-kind warning and registers the optional
//! id; the dispatcher in [`super::super::nodes::walk_node`] performs the child
//! recursion so traversal order is unchanged.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::node::{
    ConnectorAnchorParseError, ConnectorNode, ShapeNode, UnknownNode, parse_connector_anchor,
};
use crate::diagnostics::Diagnostic;
use crate::suggest::invalid_value_message;

use super::shared::{
    AnchorParentCtx, AnchorProps, TokenEnv, check_anchor, check_optional_dim, check_spans,
    check_style_ref,
};
use super::suggest::check_unknown_props;
use crate::validate::check::nodes::WalkCtx;
use crate::validate::check::register_id;
use crate::validate::check::visual::{VisualExpect, check_visual_prop};

pub(in crate::validate::check) fn check_shape(
    s: &ShapeNode,
    ctx: WalkCtx,
    seen_ids: &mut BTreeSet<String>,
    referenced_token_ids: &mut BTreeSet<String>,
    geom_required: bool,
    parent_ctx: AnchorParentCtx,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let WalkCtx {
        resolved_tokens,
        declared_style_ids,
        zone_ids,
        ..
    } = ctx;
    register_id(&s.id, seen_ids, diagnostics);
    check_style_ref(
        &s.id,
        s.style.as_deref(),
        declared_style_ids,
        s.source_span,
        diagnostics,
    );
    check_style_ref(
        &s.id,
        s.text_style.as_deref(),
        declared_style_ids,
        s.source_span,
        diagnostics,
    );

    // A recognized anchor supplies both x and y.
    let anchor_active = check_anchor(
        &s.id,
        AnchorProps {
            anchor: s.anchor.as_deref(),
            anchor_zone: s.anchor_zone.as_deref(),
            anchor_sibling: s.anchor_sibling.as_deref(),
            anchor_parent: s.anchor_parent == Some(true),
            anchor_edge: s.anchor_edge.as_deref(),
            anchor_gap: s.anchor_gap.as_ref(),
        },
        parent_ctx,
        zone_ids,
        s.source_span,
        diagnostics,
    );
    let xy_required = geom_required && !anchor_active;

    // Required geometry: x, y, w, h must all be present.
    {
        let mut tokens = TokenEnv {
            referenced: referenced_token_ids,
            resolved: resolved_tokens,
        };
        check_optional_dim(
            &s.id,
            "x",
            s.x.as_ref(),
            xy_required,
            s.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &s.id,
            "y",
            s.y.as_ref(),
            xy_required,
            s.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &s.id,
            "w",
            s.w.as_ref(),
            geom_required,
            s.source_span,
            &mut tokens,
            diagnostics,
        );
        check_optional_dim(
            &s.id,
            "h",
            s.h.as_ref(),
            geom_required,
            s.source_span,
            &mut tokens,
            diagnostics,
        );
    }

    // Visual properties — all token-required.
    check_visual_prop(
        &s.id,
        "fill",
        s.fill.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &s.id,
        "stroke",
        s.stroke.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &s.id,
        "stroke-width",
        s.stroke_width.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &s.id,
        "radius",
        s.radius.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &s.id,
        "padding",
        s.padding.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    // Enum-value checks (an unrecognized value is an Error).
    if let Some(k) = s.kind.as_deref()
        && !matches!(k, "process" | "decision" | "terminator" | "ellipse")
    {
        diagnostics.push(Diagnostic::error(
            "shape.unknown_kind",
            invalid_value_message(
                &format!("shape '{}'", s.id),
                "kind",
                k,
                &["process", "decision", "terminator", "ellipse"],
            ),
            s.source_span,
            Some(s.id.clone()),
        ));
    }
    if let Some(sa) = s.stroke_alignment.as_deref()
        && !matches!(sa, "inside" | "center" | "outside")
    {
        diagnostics.push(Diagnostic::error(
            "shape.invalid_stroke_alignment",
            invalid_value_message(
                &format!("shape '{}'", s.id),
                "stroke-alignment",
                sa,
                &["inside", "center", "outside"],
            ),
            s.source_span,
            Some(s.id.clone()),
        ));
    }
    if let Some(ha) = s.h_align.as_deref()
        && !matches!(ha, "start" | "center" | "end")
    {
        diagnostics.push(Diagnostic::error(
            "shape.invalid_h_align",
            invalid_value_message(
                &format!("shape '{}'", s.id),
                "h-align",
                ha,
                &["start", "center", "end"],
            ),
            s.source_span,
            Some(s.id.clone()),
        ));
    }
    if let Some(va) = s.v_align.as_deref()
        && !matches!(va, "top" | "middle" | "bottom")
    {
        diagnostics.push(Diagnostic::error(
            "shape.invalid_v_align",
            invalid_value_message(
                &format!("shape '{}'", s.id),
                "v-align",
                va,
                &["top", "middle", "bottom"],
            ),
            s.source_span,
            Some(s.id.clone()),
        ));
    }

    // Per-span visual properties (mirrors Node::Text). Registers token
    // refs so they are not falsely flagged as unused, and type-checks
    // fill/font-weight on each span.
    check_spans(
        &s.id,
        &s.spans,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    // Unknown properties.
    check_unknown_props("shape", &s.id, &s.unknown_props, s.source_span, diagnostics);
}

pub(in crate::validate::check) fn check_connector(
    c: &ConnectorNode,
    ctx: WalkCtx,
    seen_ids: &mut BTreeSet<String>,
    referenced_token_ids: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let WalkCtx {
        resolved_tokens,
        declared_style_ids,
        all_node_ids,
        local_node_ids,
        ports_by_node,
        ..
    } = ctx;
    register_id(&c.id, seen_ids, diagnostics);
    check_style_ref(
        &c.id,
        c.style.as_deref(),
        declared_style_ids,
        c.source_span,
        diagnostics,
    );

    // Stroke visual properties — token-required (connector has no fill).
    check_visual_prop(
        &c.id,
        "stroke",
        c.stroke.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &c.id,
        "stroke-width",
        c.stroke_width.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    check_connector_endpoint(
        c,
        "from",
        c.from.as_deref(),
        all_node_ids,
        local_node_ids,
        ports_by_node,
        diagnostics,
    );
    check_connector_endpoint(
        c,
        "to",
        c.to.as_deref(),
        all_node_ids,
        local_node_ids,
        ports_by_node,
        diagnostics,
    );

    // Missing endpoints: a connector with no `from`/`to` can't route.
    if c.from.is_none() || c.to.is_none() {
        diagnostics.push(Diagnostic::warning(
            "connector.missing_target",
            format!(
                "connector '{}': both 'from' and 'to' are required to route",
                c.id
            ),
            c.source_span,
            Some(c.id.clone()),
        ));
    }

    // Enum-value checks (an unrecognized value is an Error).
    if let Some(r) = c.route.as_deref()
        && !matches!(r, "straight" | "orthogonal" | "avoid")
    {
        diagnostics.push(Diagnostic::error(
            "connector.invalid_route",
            invalid_value_message(
                &format!("connector '{}'", c.id),
                "route",
                r,
                &["straight", "orthogonal", "avoid"],
            ),
            c.source_span,
            Some(c.id.clone()),
        ));
    }
    for (label, marker) in [
        ("marker-start", c.marker_start.as_deref()),
        ("marker-end", c.marker_end.as_deref()),
    ] {
        if let Some(m) = marker
            && !matches!(m, "none" | "arrow")
        {
            diagnostics.push(Diagnostic::error(
                "connector.invalid_marker",
                invalid_value_message(
                    &format!("connector '{}'", c.id),
                    label,
                    m,
                    &["none", "arrow"],
                ),
                c.source_span,
                Some(c.id.clone()),
            ));
        }
    }
    for (label, anchor) in [
        ("from-anchor", c.from_anchor.as_deref()),
        ("to-anchor", c.to_anchor.as_deref()),
    ] {
        if let Some(a) = anchor {
            match parse_connector_anchor(a) {
                Ok(_) => {}
                Err(ConnectorAnchorParseError::ZeroCount) => {
                    diagnostics.push(Diagnostic::error(
                        "connector.anchor_division_zero",
                        format!(
                            "connector '{}': {label} '{a}' has a divided anchor count of 0",
                            c.id
                        ),
                        c.source_span,
                        Some(c.id.clone()),
                    ));
                }
                Err(ConnectorAnchorParseError::IndexOutOfRange { index, count }) => {
                    diagnostics.push(Diagnostic::error(
                        "connector.anchor_index_out_of_range",
                        format!(
                            "connector '{}': {label} '{a}' has index {index} outside divided anchor count {count}",
                            c.id
                        ),
                        c.source_span,
                        Some(c.id.clone()),
                    ));
                }
                Err(ConnectorAnchorParseError::InvalidSyntax) => {
                    diagnostics.push(Diagnostic::error(
                        "connector.invalid_anchor",
                        format!(
                            "connector '{}': {label} '{a}' is not 'auto', a divided anchor like '4/16', or a nine-point anchor (top/center/bottom × left/center/right, e.g. bottom-right)",
                            c.id
                        ),
                        c.source_span,
                        Some(c.id.clone()),
                    ));
                }
            }
        }
    }

    // Unknown properties.
    check_unknown_props(
        "connector",
        &c.id,
        &c.unknown_props,
        c.source_span,
        diagnostics,
    );
}

fn check_connector_endpoint(
    c: &ConnectorNode,
    label: &str,
    endpoint: Option<&str>,
    all_node_ids: &BTreeSet<String>,
    local_node_ids: &BTreeSet<String>,
    ports_by_node: &BTreeMap<String, BTreeSet<String>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(target) = endpoint else {
        return;
    };
    let Some((node_id, port_id)) = target.split_once('#') else {
        if !all_node_ids.contains(target) {
            diagnostics.push(Diagnostic::warning(
                "connector.unknown_target",
                format!(
                    "connector '{}': {label} '{}' matches no node id in the document",
                    c.id, target
                ),
                c.source_span,
                Some(c.id.clone()),
            ));
        }
        return;
    };

    if node_id.is_empty() || !local_node_ids.contains(node_id) {
        diagnostics.push(Diagnostic::warning(
            "connector.port_invalid_target",
            format!(
                "connector '{}': {label} '{}' targets unknown node '{}'",
                c.id, target, node_id
            ),
            c.source_span,
            Some(c.id.clone()),
        ));
        return;
    }

    if port_id.is_empty()
        || !ports_by_node
            .get(node_id)
            .is_some_and(|ports| ports.contains(port_id))
    {
        diagnostics.push(Diagnostic::error(
            "connector.unknown_port",
            format!(
                "connector '{}': {label} '{}' references unknown port '{}' on node '{}'",
                c.id, target, port_id, node_id
            ),
            c.source_span,
            Some(c.id.clone()),
        ));
    }
}

/// Emit the `node.unknown_kind` warning (library nodes use kinds the engine does
/// not know, so this stays a Warning) and register the
/// optional id. The child recursion stays in the dispatcher so the unknown
/// node's children are walked in the same position as before.
pub(in crate::validate::check) fn check_unknown(
    u: &UnknownNode,
    seen_ids: &mut BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let subject = match &u.id {
        Some(id) => format!("node '{id}'"),
        None => "node".to_owned(),
    };
    diagnostics.push(Diagnostic::warning(
        "node.unknown_kind",
        invalid_value_message(&subject, "kind", &u.kind, crate::schema::node_kinds()),
        u.source_span,
        u.id.clone(),
    ));
    // Register the id (if any) so the unknown node is addressable and
    // participates in duplicate-id detection alongside known nodes.
    if let Some(id) = &u.id {
        register_id(id, seen_ids, diagnostics);
    }
}
