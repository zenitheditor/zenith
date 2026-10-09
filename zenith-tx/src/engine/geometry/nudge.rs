//! `nudge_geometry` application: offset a box node's `x` / `y` / `w` / `h`
//! by px deltas and keep each attribute's unit.

use std::collections::BTreeMap;

use zenith_core::{
    Diagnostic, Dimension, Document, Node, PropertyValue, ResolvedToken, resolve_geometry_px,
};

use super::super::layout::reject_layout_managed;
use super::super::space::resolved_tokens;
use super::super::{find_node_any_mut, find_node_any_shared, px, record_affected};
use super::axis::{AxisCtx, dimension_px, shown};
use super::boxes::node_geometry_mut;
use super::required::anchored;

const OP: &str = "nudge_geometry";

/// The deltas of one `nudge_geometry` op. `None` leaves that attribute.
#[derive(Clone, Copy, Debug)]
pub(in crate::engine) struct NudgeDelta {
    pub dx: Option<f64>,
    pub dy: Option<f64>,
    pub dw: Option<f64>,
    pub dh: Option<f64>,
    /// Replace a token-bound attribute with a px literal.
    pub detach: bool,
}

/// One box attribute.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Axis {
    X,
    Y,
    W,
    H,
}

impl Axis {
    fn name(self) -> &'static str {
        match self {
            Axis::X => "x",
            Axis::Y => "y",
            Axis::W => "w",
            Axis::H => "h",
        }
    }

    fn is_size(self) -> bool {
        match self {
            Axis::X | Axis::Y => false,
            Axis::W | Axis::H => true,
        }
    }
}

/// What the node says about its attributes beyond their values.
struct NodeFacts {
    /// An anchor supplies each absent `x` / `y`.
    anchor: Option<String>,
    /// An absent `x` / `y` counts as px 0 (group, instance).
    origin_defaults: bool,
    /// The `hug` / `fill` keyword on `w` and `h`.
    keywords: (Option<String>, Option<String>),
    /// Each `x` / `y` / `w` / `h` value, cloned for planning.
    values: [Option<PropertyValue>; 4],
}

pub(in crate::engine) fn apply_nudge_geometry(
    node_id: &str,
    delta: NudgeDelta,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let requested: Vec<(Axis, f64)> = [
        (Axis::X, delta.dx),
        (Axis::Y, delta.dy),
        (Axis::W, delta.dw),
        (Axis::H, delta.dh),
    ]
    .into_iter()
    .filter_map(|(axis, d)| d.map(|d| (axis, d)))
    .collect();
    if requested.is_empty() {
        diagnostics.push(Diagnostic::advisory(
            "tx.noop",
            format!("{OP} on {node_id:?} specified no deltas. The document is unchanged."),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    }
    let mut bad_input = false;
    for (axis, d) in &requested {
        if !d.is_finite() {
            bad_input = true;
            diagnostics.push(Diagnostic::error(
                "tx.invalid_value",
                format!(
                    "{OP}: d{} {d} on node {node_id:?} is not finite",
                    axis.name()
                ),
                None,
                Some(node_id.to_owned()),
            ));
        }
    }
    if bad_input {
        return;
    }
    let moves = delta.dx.is_some() || delta.dy.is_some();
    if moves && reject_layout_managed(doc, [node_id], OP, diagnostics) {
        return;
    }
    let resolved = resolved_tokens(doc);
    let Some(node) = find_node_any_shared(doc, node_id) else {
        diagnostics.push(unknown_node(node_id));
        return;
    };
    if let Some(diagnostic) = unsupported(node, node_id) {
        diagnostics.push(diagnostic);
        return;
    }
    let facts = node_facts(node);

    let mut writes: Vec<(Axis, Dimension)> = Vec::with_capacity(requested.len());
    let mut failed = false;
    for (axis, d) in requested {
        match plan_axis(node_id, axis, d, &facts, delta.detach, &resolved) {
            Ok(value) => writes.push((axis, value)),
            Err(diagnostic) => {
                failed = true;
                diagnostics.push(diagnostic);
            }
        }
    }
    if failed {
        return;
    }
    if let Some(node) = find_node_any_mut(doc, node_id) {
        write_axes(node, &writes);
        record_affected(node_id, affected);
    }
}

fn unknown_node(node_id: &str) -> Diagnostic {
    Diagnostic::error(
        "tx.unknown_node",
        format!("node {node_id:?} not found in document"),
        None,
        Some(node_id.to_owned()),
    )
}

/// The diagnostic for a node kind `nudge_geometry` does not edit, naming
/// the op that edits it. `None` for a box kind.
fn unsupported(node: &Node, node_id: &str) -> Option<Diagnostic> {
    let kind = node.kind_str();
    let (code, hint) = match node {
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Frame(_)
        | Node::Group(_)
        | Node::Image(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Table(_)
        | Node::Shape(_)
        | Node::Pattern(_)
        | Node::Chart(_)
        | Node::Mesh(_) => return None,
        Node::Line(_) => (
            "tx.unsupported_property",
            "a line has no box. Move its endpoints with nudge_line_points.",
        ),
        Node::Path(_) => (
            "tx.unsupported_property",
            "a path has no box. Move or resize it with transform_path_anchors \
             (mode translate or scale).",
        ),
        Node::Polygon(_) | Node::Polyline(_) => (
            "tx.unsupported_property",
            "a polygon or polyline has no box. Rewrite its vertices with set_points.",
        ),
        Node::Connector(_) => (
            "tx.derived_geometry",
            "a connector's geometry derives from its from/to targets. Move the targets, \
             or change from, to, from-anchor, or to-anchor.",
        ),
        Node::Footnote(_) | Node::Light(_) | Node::Unknown(_) => {
            ("tx.unsupported_property", "this kind has no x/y/w/h box.")
        }
    };
    Some(Diagnostic::error(
        code,
        format!("{OP} is not supported on {kind} {node_id:?}: {hint}"),
        None,
        Some(node_id.to_owned()),
    ))
}

/// Read what planning needs from a supported box node.
fn node_facts(node: &Node) -> NodeFacts {
    let anchor = anchored(node)
        .then(|| node.anchor_view().map(|v| describe_anchor(&v)))
        .flatten();
    let origin_defaults = matches!(node, Node::Group(_) | Node::Instance(_));
    let keywords = node.layout_item().map_or((None, None), |item| {
        (
            item.w_keyword.map(|k| k.as_str().to_owned()),
            item.h_keyword.map(|k| k.as_str().to_owned()),
        )
    });
    let values = if let Node::Instance(i) = node {
        [&i.x, &i.y, &i.w, &i.h].map(|d| d.clone().map(PropertyValue::Dimension))
    } else {
        node.box_view()
            .map(|v| [v.x, v.y, v.w, v.h].map(|p| p.cloned()))
            .unwrap_or_default()
    };
    NodeFacts {
        anchor,
        origin_defaults,
        keywords,
        values,
    }
}

/// The anchor attributes of a node as `.zen` source text, for messages.
fn describe_anchor(view: &zenith_core::AnchorView<'_>) -> String {
    let mut parts = Vec::new();
    if let Some(a) = view.anchor {
        parts.push(format!("anchor={a:?}"));
    }
    if let Some(z) = view.anchor_zone {
        parts.push(format!("anchor-zone={z:?}"));
    }
    if let Some(s) = view.anchor_sibling {
        parts.push(format!("anchor-sibling={s:?}"));
    }
    if view.anchor_parent == Some(true) {
        parts.push("anchor-parent=#true".to_owned());
    }
    if let Some(e) = view.anchor_edge {
        parts.push(format!("anchor-edge={e:?}"));
    }
    parts.join(" ")
}

/// The new value of one attribute, or the Error that rejects it.
fn plan_axis(
    node_id: &str,
    axis: Axis,
    delta: f64,
    facts: &NodeFacts,
    detach: bool,
    resolved: &BTreeMap<String, ResolvedToken>,
) -> Result<Dimension, Diagnostic> {
    let ctx = AxisCtx {
        op: OP,
        node_id,
        attr: axis.name(),
    };
    let keyword = match axis {
        Axis::W => facts.keywords.0.as_deref(),
        Axis::H => facts.keywords.1.as_deref(),
        Axis::X | Axis::Y => None,
    };
    if let Some(k) = keyword {
        return Err(computed(&ctx, &format!("the {k:?} keyword")));
    }
    let index = match axis {
        Axis::X => 0,
        Axis::Y => 1,
        Axis::W => 2,
        Axis::H => 3,
    };
    let current = facts.values.get(index).cloned().flatten();
    let value = match &current {
        Some(PropertyValue::Dimension(d)) => ctx.offset(d, delta)?,
        Some(token @ PropertyValue::TokenRef(id)) => {
            if !detach {
                return Err(ctx.error(
                    "tx.token_bound",
                    format!(
                        "is bound to token {id:?}. A nudge keeps token bindings. Change the \
                         token with update_token_value, or pass detach=true to replace {} \
                         with a px value.",
                        axis.name()
                    ),
                ));
            }
            let base = resolve_geometry_px(Some(token), resolved)
                .ok_or_else(|| ctx.unresolved(&shown(token)))?;
            px(base + delta)
        }
        Some(other @ (PropertyValue::Literal(_) | PropertyValue::DataRef(_))) => {
            return Err(ctx.unresolved(&shown(other)));
        }
        None if axis.is_size() => {
            return Err(computed(&ctx, &format!("no authored {}", axis.name())));
        }
        None => match &facts.anchor {
            Some(anchor) => {
                return Err(ctx.error(
                    "tx.anchored",
                    format!(
                        "comes from its anchor ({anchor}). Move along the anchor edge with \
                         nudge_anchor_gap, change the anchor with set_anchor, or place the \
                         node by x/y with detach_anchor."
                    ),
                ));
            }
            None if facts.origin_defaults => px(delta),
            None => return Err(ctx.unresolved("absent")),
        },
    };
    check_result(&ctx, axis, delta, &value)?;
    Ok(value)
}

/// `tx.computed_size`: layout or content computes this size.
fn computed(ctx: &AxisCtx<'_>, why: &str) -> Diagnostic {
    ctx.error(
        "tx.computed_size",
        format!(
            "is computed by layout or content ({why}), so it has no value to offset. To fix \
             the size, write the measured px size with set_geometry."
        ),
    )
}

/// Reject a non-finite result, and a negative `w` / `h`.
fn check_result(
    ctx: &AxisCtx<'_>,
    axis: Axis,
    delta: f64,
    value: &Dimension,
) -> Result<(), Diagnostic> {
    let px = dimension_px(value).unwrap_or(f64::NAN);
    if !px.is_finite() {
        return Err(ctx.error(
            "tx.invalid_geometry",
            format!(
                "would become {}, which is not finite",
                value.to_kdl_string()
            ),
        ));
    }
    if axis.is_size() && px < 0.0 {
        return Err(ctx.error(
            "tx.invalid_geometry",
            format!(
                "would become negative ({}) with d{} {delta}. A size must stay >= 0.",
                value.to_kdl_string(),
                axis.name()
            ),
        ));
    }
    Ok(())
}

/// Write the planned values onto `node`.
fn write_axes(node: &mut Node, writes: &[(Axis, Dimension)]) {
    if let Node::Instance(inst) = node {
        for (axis, value) in writes {
            let slot = match axis {
                Axis::X => &mut inst.x,
                Axis::Y => &mut inst.y,
                Axis::W => &mut inst.w,
                Axis::H => &mut inst.h,
            };
            *slot = Some(value.clone());
        }
        return;
    }
    if let Some((x, y, w, h)) = node_geometry_mut(node) {
        for (axis, value) in writes {
            let slot = match axis {
                Axis::X => &mut *x,
                Axis::Y => &mut *y,
                Axis::W => &mut *w,
                Axis::H => &mut *h,
            };
            *slot = Some(PropertyValue::Dimension(value.clone()));
        }
    }
}
