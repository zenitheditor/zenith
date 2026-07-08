//! Anchor-property validation: the single-node `check_anchor` and the
//! per-scope sibling-anchor graph validator `check_sibling_anchors`.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::node::{Node, parse_anchor, parse_anchor_edge};
use crate::ast::value::{Dimension, dim_to_px};
use crate::diagnostics::Diagnostic;

/// Container context for the parent-relative anchor checks.
///
/// `in_container` is `true` when the node is a direct (or group-nested) child of
/// a `frame`/`group`. `parent_box_known` is `true` when that enclosing container
/// has a usable reference box (frame: always; group: only when it declares both
/// `w` and `h`). At the page root both are `false`.
#[derive(Clone, Copy)]
pub(in crate::validate::check) struct AnchorParentCtx {
    pub(in crate::validate::check) in_container: bool,
    pub(in crate::validate::check) parent_box_known: bool,
}

/// The anchor property reads, bundled so [`check_anchor`] stays
/// within the argument-count lint without suppression.
#[derive(Clone, Copy)]
pub(in crate::validate::check) struct AnchorProps<'a> {
    pub(in crate::validate::check) anchor: Option<&'a str>,
    pub(in crate::validate::check) anchor_zone: Option<&'a str>,
    pub(in crate::validate::check) anchor_sibling: Option<&'a str>,
    pub(in crate::validate::check) anchor_parent: bool,
    pub(in crate::validate::check) anchor_edge: Option<&'a str>,
    pub(in crate::validate::check) anchor_gap: Option<&'a Dimension>,
}

/// Validate the `anchor`, `anchor_zone`, `anchor_sibling`, `anchor_parent`,
/// `anchor_edge`, and `anchor_gap` properties on a node.
///
/// Returns `true` when `anchor` is present and recognized, OR when
/// `anchor_sibling` + `anchor_edge` are both present (edge-placement mode:
/// x/y geometry is NOT required in that case either), `false` otherwise.
///
/// Diagnostics pushed:
/// - `anchor.unknown_value` (Error) — `anchor` present with an unrecognized value.
/// - `anchor.zone_without_anchor` (Warning) — `anchor_zone` set but `anchor` absent.
/// - `anchor.unresolved_zone` (Error) — `anchor_zone` names a zone not on this page.
/// - `anchor.sibling_without_anchor` (Warning) — `anchor_sibling` set but `anchor` absent
///   and `anchor_edge` is also absent (edge-placement makes `anchor` optional).
///   (The sibling-reference graph — `anchor.unresolved_sibling` / `anchor.cycle` —
///   is validated per-scope by [`check_sibling_anchors`], not here.)
/// - `anchor.parent_without_anchor` (Warning) — `anchor_parent` set but `anchor` absent.
/// - `anchor.unresolvable_parent` (Error) — `anchor_parent` set but the node is
///   not inside a frame/group container, or the parent container's box is unknown
///   (a group without `w`/`h`).
/// - `anchor.edge_without_sibling` (Warning) — `anchor_edge` set but `anchor_sibling` absent.
/// - `anchor.unknown_edge` (Error) — `anchor_edge` value is not one of the four
///   recognized directional values.
/// - `anchor.gap_invalid_unit` (Warning) — `anchor_gap` unit cannot be resolved to px.
pub(in crate::validate::check) fn check_anchor(
    node_id: &str,
    props: AnchorProps,
    parent_ctx: AnchorParentCtx,
    zone_ids: &BTreeSet<&str>,
    span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let AnchorProps {
        anchor,
        anchor_zone,
        anchor_sibling,
        anchor_parent,
        anchor_edge,
        anchor_gap,
    } = props;
    // When anchor-zone is present without anchor, emit a warning and treat zone as
    // irrelevant (anchor-zone has no effect without an anchor value).
    if anchor_zone.is_some() && anchor.is_none() {
        diagnostics.push(Diagnostic::warning(
            "anchor.zone_without_anchor",
            format!(
                "node '{}': anchor-zone is set but anchor is absent; \
                 anchor-zone has no effect without an anchor value",
                node_id
            ),
            span,
            Some(node_id.to_owned()),
        ));
    }

    // When anchor-sibling is present without anchor, emit a warning — BUT only
    // when anchor-edge is also absent. When anchor-edge is set, anchor-sibling
    // enables edge-placement mode and anchor is intentionally optional.
    if anchor_sibling.is_some() && anchor.is_none() && anchor_edge.is_none() {
        diagnostics.push(Diagnostic::warning(
            "anchor.sibling_without_anchor",
            format!(
                "node '{}': anchor-sibling is set but anchor is absent; \
                 anchor-sibling has no effect without an anchor value \
                 (unless anchor-edge is set)",
                node_id
            ),
            span,
            Some(node_id.to_owned()),
        ));
    }

    // When anchor-parent is set without anchor, emit a warning (anchor-parent has
    // no effect without an anchor value to position).
    if anchor_parent && anchor.is_none() {
        diagnostics.push(Diagnostic::warning(
            "anchor.parent_without_anchor",
            format!(
                "node '{}': anchor-parent is set but anchor is absent; \
                 anchor-parent has no effect without an anchor value",
                node_id
            ),
            span,
            Some(node_id.to_owned()),
        ));
    }

    // When anchor-parent is set, the node must live inside a frame/group whose
    // reference box is resolvable; otherwise the parent-relative anchor cannot be
    // derived. `anchor_zone` takes precedence and disables parent mode, so only
    // flag when no zone is set.
    if anchor_parent
        && anchor_zone.is_none()
        && (!parent_ctx.in_container || !parent_ctx.parent_box_known)
    {
        diagnostics.push(Diagnostic::error(
            "anchor.unresolvable_parent",
            format!(
                "node '{}': anchor-parent is set but the node is not inside a \
                 frame/group container with a usable box",
                node_id
            ),
            span,
            Some(node_id.to_owned()),
        ));
    }

    // When anchor-edge is present without anchor-sibling, it has no effect.
    if anchor_edge.is_some() && anchor_sibling.is_none() {
        diagnostics.push(Diagnostic::warning(
            "anchor.edge_without_sibling",
            format!(
                "node '{}': anchor-edge is set but anchor-sibling is absent; \
                 it has no effect without an anchor-sibling target",
                node_id
            ),
            span,
            Some(node_id.to_owned()),
        ));
    }

    // When anchor-edge is present, validate that the value is one of the four
    // recognized directional values. `parse_anchor_edge` is the single source
    // of truth for the valid names (shared with the scene pre-pass).
    if let Some(edge) = anchor_edge
        && parse_anchor_edge(edge).is_none()
    {
        diagnostics.push(Diagnostic::error(
            "anchor.unknown_edge",
            format!(
                "node '{}': anchor-edge value '{}' is not recognized; \
                 valid values are above, below, before, after",
                node_id, edge
            ),
            span,
            Some(node_id.to_owned()),
        ));
    }

    // When anchor-gap is present, the unit must be px-convertible.
    if let Some(gap) = anchor_gap
        && dim_to_px(gap.value, &gap.unit).is_none()
    {
        diagnostics.push(Diagnostic::warning(
            "anchor.gap_invalid_unit",
            format!(
                "node '{}': anchor-gap unit '{}' cannot be resolved to px; \
                 gap must resolve to px",
                node_id,
                gap.unit.as_annotation()
            ),
            span,
            Some(node_id.to_owned()),
        ));
    }

    let anchor_active = match anchor {
        None => false,
        Some(s) => {
            if parse_anchor(s).is_some() {
                true
            } else {
                diagnostics.push(Diagnostic::error(
                    "anchor.unknown_value",
                    format!(
                        "node '{}': anchor value '{}' is not recognized; \
                         valid values are top-left, top-center, top-right, \
                         center-left, center, center-right, \
                         bottom-left, bottom-center, bottom-right",
                        node_id, s
                    ),
                    span,
                    Some(node_id.to_owned()),
                ));
                false
            }
        }
    };

    // When anchor-zone names a zone, check that it exists on the page.
    if let Some(zone_id) = anchor_zone
        && !zone_ids.contains(zone_id)
    {
        diagnostics.push(Diagnostic::error(
            "anchor.unresolved_zone",
            format!(
                "node '{}': anchor-zone '{}' does not name a safe-zone on this page",
                node_id, zone_id
            ),
            span,
            Some(node_id.to_owned()),
        ));
    }

    // Edge-placement mode: anchor_sibling + anchor_edge together supply both x
    // and y (the engine positions the node relative to the sibling edge), so
    // x/y geometry is not required even without a nine-point anchor.
    anchor_active || (anchor_sibling.is_some() && anchor_edge.is_some())
}

/// Per-node sibling-anchor read: the node's id, its `anchor_sibling` target (if
/// any), and its span — for anchor-bearing node kinds only. Kinds that never
/// carry an `anchor` return `None` (they are not valid sibling targets and
/// cannot themselves reference a sibling). The match is EXHAUSTIVE over `Node`
/// so a new kind forces a decision here.
fn node_sibling_fields(node: &Node) -> Option<(&str, Option<&str>, Option<crate::ast::Span>)> {
    let f = match node {
        Node::Rect(n) => (n.id.as_str(), n.anchor_sibling.as_deref(), n.source_span),
        Node::Ellipse(n) => (n.id.as_str(), n.anchor_sibling.as_deref(), n.source_span),
        Node::Text(n) => (n.id.as_str(), n.anchor_sibling.as_deref(), n.source_span),
        Node::Code(n) => (n.id.as_str(), n.anchor_sibling.as_deref(), n.source_span),
        Node::Image(n) => (n.id.as_str(), n.anchor_sibling.as_deref(), n.source_span),
        Node::Frame(n) => (n.id.as_str(), n.anchor_sibling.as_deref(), n.source_span),
        Node::Group(n) => (n.id.as_str(), n.anchor_sibling.as_deref(), n.source_span),
        Node::Shape(n) => (n.id.as_str(), n.anchor_sibling.as_deref(), n.source_span),
        Node::Table(n) => (n.id.as_str(), n.anchor_sibling.as_deref(), n.source_span),
        Node::Field(n) => (n.id.as_str(), n.anchor_sibling.as_deref(), n.source_span),
        Node::Toc(n) => (n.id.as_str(), n.anchor_sibling.as_deref(), n.source_span),
        Node::Pattern(n) => (n.id.as_str(), n.anchor_sibling.as_deref(), n.source_span),
        Node::Chart(n) => (n.id.as_str(), n.anchor_sibling.as_deref(), n.source_span),
        Node::Light(_) => return None,
        Node::Mesh(_) => return None,
        // Kinds that never carry an `anchor` are not sibling-bearing.
        Node::Line(_)
        | Node::Connector(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Footnote(_)
        | Node::Instance(_)
        | Node::Unknown(_) => return None,
    };
    Some(f)
}

/// Validate the sibling-anchor (`anchor-sibling`) graph of one container scope
/// (`children` = the direct children of a page / frame / group).
///
/// Diagnostics pushed:
/// - `anchor.unresolved_sibling` (Error) — a node names an `anchor-sibling`
///   target that is not an in-scope anchor-bearing node id.
/// - `anchor.cycle` (Error) — a node participates in a sibling-anchor reference
///   cycle within this scope. Each cyclic node is reported at most once.
///
/// The cycle detection mirrors the iterative visited-set chain-follow used by
/// the `token.cyclic_reference` detector in `tokens::resolve::driver`: each walk
/// follows the in-scope `id → target` map with a per-walk `BTreeSet` visited;
/// revisiting an id signals a cycle. A scope-wide `BTreeSet` of already-reported
/// ids dedupes across walks. Bounded by scope size; no recursion, no panic.
pub(in crate::validate::check) fn check_sibling_anchors(
    children: &[Node],
    diagnostics: &mut Vec<Diagnostic>,
) {
    // In-scope anchor-bearing node ids (valid sibling targets).
    let mut in_scope: BTreeSet<&str> = BTreeSet::new();
    for child in children {
        if let Some((id, _, _)) = node_sibling_fields(child) {
            in_scope.insert(id);
        }
    }

    // Unresolved-reference pass, plus build the in-scope id → target edge map for
    // cycle detection (only edges whose target is itself in-scope and anchor-
    // bearing form the graph; an out-of-scope target is reported as unresolved
    // and never enters the graph).
    let mut edges: BTreeMap<&str, &str> = BTreeMap::new();
    for child in children {
        let Some((id, anchor_sibling, span)) = node_sibling_fields(child) else {
            continue;
        };
        if let Some(target) = anchor_sibling {
            if in_scope.contains(target) {
                edges.insert(id, target);
            } else {
                diagnostics.push(Diagnostic::error(
                    "anchor.unresolved_sibling",
                    format!(
                        "node '{}': anchor-sibling '{}' does not name a sibling \
                         node in the same container",
                        id, target
                    ),
                    span,
                    Some(id.to_owned()),
                ));
            }
        }
    }

    // Cycle detection: follow each id's chain through `edges` with a per-walk
    // visited set. A revisit means a cycle; report it once per node.
    let mut reported: BTreeSet<&str> = BTreeSet::new();
    let span_of: BTreeMap<&str, Option<crate::ast::Span>> = children
        .iter()
        .filter_map(|c| node_sibling_fields(c).map(|(id, _, span)| (id, span)))
        .collect();
    for &start in edges.keys() {
        if reported.contains(start) {
            continue;
        }
        let mut visited: BTreeSet<&str> = BTreeSet::new();
        let mut current = start;
        visited.insert(current);
        while let Some(&next) = edges.get(current) {
            if visited.contains(next) {
                // `next` closes a cycle. Report `start` (the walk origin) once.
                if reported.insert(start) {
                    diagnostics.push(Diagnostic::error(
                        "anchor.cycle",
                        format!(
                            "node '{}': anchor-sibling chain reaches a cycle \
                             (at '{}'); its position cannot be resolved",
                            start, next
                        ),
                        span_of.get(start).copied().flatten(),
                        Some(start.to_owned()),
                    ));
                }
                break;
            }
            visited.insert(next);
            current = next;
        }
    }
}
