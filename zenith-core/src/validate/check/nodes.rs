//! Per-node validation: the recursive [`walk_node`] dispatcher and the
//! walk-wide context/position types.
//!
//! The dispatcher runs the shared prologue advisories (frame.child_overflow,
//! off_canvas with rotate-AABB), computes the per-node `geom_required` gate,
//! then dispatches to the per-kind `check_*` helpers in [`node`] and performs
//! the container (frame/group/table/unknown) recursion. All per-kind
//! validation logic lives in the [`node`] submodules.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::AssetKind;
use crate::ast::node::Node;
use crate::ast::style::Style;
use crate::diagnostics::Diagnostic;
use crate::tokens::ResolvedToken;

use super::visual::attach_visual_spans;

mod node;
mod placement;

pub(super) use node::shared::{AnchorParentCtx, check_sibling_anchors, node_bbox};
pub(super) use placement::{PlacementSite, placement_walk};

/// Walk-wide immutable validation context (never changes during a page walk).
#[derive(Clone, Copy)]
pub(super) struct WalkCtx<'a> {
    pub(super) resolved_tokens: &'a BTreeMap<String, ResolvedToken>,
    pub(super) declared_asset_ids: &'a BTreeSet<String>,
    /// Declared asset id → its declared [`AssetKind`], so an `image` node can
    /// validate that SVG-only style properties target an `svg` asset.
    pub(super) asset_kinds: &'a BTreeMap<String, AssetKind>,
    pub(super) declared_style_ids: &'a BTreeSet<String>,
    /// Declared style id → style, for style-value checks on nodes.
    pub(super) style_map: &'a BTreeMap<&'a str, &'a Style>,
    pub(super) declared_component_ids: &'a BTreeSet<String>,
    pub(super) component_local_ids: &'a BTreeMap<String, BTreeSet<String>>,
    pub(super) all_node_ids: &'a BTreeSet<String>,
    pub(super) local_node_ids: &'a BTreeSet<String>,
    pub(super) ports_by_node: &'a BTreeMap<String, BTreeSet<String>>,
    pub(super) zone_ids: &'a BTreeSet<&'a str>,
}

/// Per-recursion position state (changes as the walk descends frames/groups).
#[derive(Clone, Copy)]
pub(super) struct WalkPos {
    pub(super) page_px_bounds: Option<(f64, f64)>,
    pub(super) in_flow_parent: bool,
    /// Layout mode of the direct parent frame when it positions children.
    pub(super) flow_parent: Option<node::FlowParent>,
    /// Page-space box of the nearest enclosing absolute frame.
    pub(super) enclosing_frame: Option<(f64, f64, f64, f64)>,
    /// Page-space origin of this node's list: the summed child spaces of the
    /// enclosing containers ([`Node::child_space`]).
    pub(super) origin: (f64, f64),
    /// `true` when this node is a direct (or group-nested) child of a
    /// `frame`/`group` — the anchor-parent container context.
    pub(super) in_container: bool,
    /// `true` when the enclosing container's reference box is usable (frame:
    /// always; group: only when it declares both `w` and `h`).
    pub(super) parent_box_known: bool,
    /// `true` when an ancestor has `role="decoration"` or `role="background"`.
    /// Placement advisories skip the whole subtree.
    pub(super) exempt: bool,
}

/// Recursively walk a [`Node`], collecting all diagnostics.
///
/// `referenced_token_ids` accumulates every token id actually used so that
/// the unused-token check (done after the walk) can diff against defined ids.
///
/// `page_px_bounds` is `Some((page_w, page_h))` when the page's dimensions
/// resolved successfully. `None` skips the off_canvas and child-overflow
/// checks: the page unit was bad (already diagnosed), or the page uses
/// auto-layout and the scene engine checks its lowered geometry.
///
/// # Known limitation
/// Recursion through `Node::Group` and `Node::Frame` children has no depth
/// guard.  Pathologically deep trees can overflow the stack.  This is an
/// accepted v0 limitation.
pub(super) fn walk_node(
    node: &Node,
    ctx: WalkCtx,
    seen_ids: &mut BTreeSet<String>,
    referenced_token_ids: &mut BTreeSet<String>,
    pos: WalkPos,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let start = diagnostics.len();
    walk_node_checks(node, ctx, seen_ids, referenced_token_ids, pos, diagnostics);
    // Token-reference diagnostics are emitted without a position; anchor them
    // to this node. Children already attached their own spans.
    let (_, node_span) = node.id_and_span();
    attach_visual_spans(diagnostics, start, node_span);
}

/// Body of [`walk_node`]: prologue advisories, per-kind dispatch, recursion.
fn walk_node_checks(
    node: &Node,
    ctx: WalkCtx,
    seen_ids: &mut BTreeSet<String>,
    referenced_token_ids: &mut BTreeSet<String>,
    pos: WalkPos,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // ── frame.child_overflow + layout.off_canvas advisories ───────────────
    // Own role or an ancestor's: the exemption covers the whole subtree.
    let exempt = pos.exempt || node.is_decorative();
    placement::check_placement(
        node,
        placement::PlacementCtx {
            enclosing_frame: pos.enclosing_frame,
            origin: pos.origin,
            page_bounds: pos.page_px_bounds,
            exempt,
        },
        diagnostics,
    );

    // Direct children of a `row`/`column`/`grid` frame (and of table cells and
    // unknown nodes) have their x/y (and, when omitted, w/h) supplied by the
    // parent, so geometry is optional.
    let geom_required = !pos.in_flow_parent;
    let layout_site = node::LayoutSite {
        parent: pos.flow_parent,
        geom_required,
    };
    node::check_layout_item(
        node,
        layout_site,
        &mut node::shared::TokenEnv {
            referenced: referenced_token_ids,
            resolved: ctx.resolved_tokens,
        },
        diagnostics,
    );
    // Parent-relative anchor context for this node: whether it sits inside a
    // frame/group container and whether that container's reference box is usable.
    let parent_ctx = AnchorParentCtx {
        in_container: pos.in_container,
        parent_box_known: pos.parent_box_known,
    };
    match node {
        Node::Rect(r) => {
            node::check_rect(
                r,
                ctx,
                seen_ids,
                referenced_token_ids,
                geom_required,
                parent_ctx,
                diagnostics,
            );
        }
        Node::Ellipse(e) => {
            node::check_ellipse(
                e,
                ctx,
                seen_ids,
                referenced_token_ids,
                geom_required,
                parent_ctx,
                diagnostics,
            );
        }
        Node::Line(l) => {
            node::check_line(l, ctx, seen_ids, referenced_token_ids, diagnostics);
        }
        Node::Text(t) => {
            node::check_text(
                t,
                ctx,
                seen_ids,
                referenced_token_ids,
                geom_required,
                parent_ctx,
                diagnostics,
            );
        }
        Node::Code(c) => {
            node::check_code(
                c,
                ctx,
                seen_ids,
                referenced_token_ids,
                geom_required,
                parent_ctx,
                diagnostics,
            );
        }
        Node::Image(img) => {
            node::check_image(
                img,
                ctx,
                seen_ids,
                referenced_token_ids,
                geom_required,
                parent_ctx,
                diagnostics,
            );
        }
        Node::Shape(s) => {
            node::check_shape(
                s,
                ctx,
                seen_ids,
                referenced_token_ids,
                geom_required,
                parent_ctx,
                diagnostics,
            );
        }
        Node::Pattern(p) => {
            // The pattern is validated as a leaf; its motif is a TEMPLATE, so its
            // own ids must not enter the uniqueness set and its validity is owned
            // by the compile-time probe — hence id-collection / motif diagnostics
            // are not done here.
            node::check_pattern(
                p,
                ctx,
                seen_ids,
                referenced_token_ids,
                geom_required,
                parent_ctx,
                diagnostics,
            );
            // The motif still RENDERS (replicated once per instance), so tokens
            // referenced only inside it are genuinely used. Walk it into throwaway
            // id/diagnostic sinks to collect those token references — without
            // registering the template's ids or re-surfacing its diagnostics.
            let mut motif_seen: BTreeSet<String> = BTreeSet::new();
            let mut motif_diags: Vec<Diagnostic> = Vec::new();
            walk_node(
                &p.motif,
                ctx,
                &mut motif_seen,
                referenced_token_ids,
                pos,
                &mut motif_diags,
            );
        }
        Node::Chart(c) => {
            // The chart is validated as a leaf; its series children are pure DATA
            // (not renderable nodes), so they are never descended into here.
            node::check_chart(
                c,
                ctx,
                seen_ids,
                referenced_token_ids,
                geom_required,
                parent_ctx,
                diagnostics,
            );
        }
        Node::Light(l) => {
            node::check_light(
                l,
                ctx,
                seen_ids,
                referenced_token_ids,
                geom_required,
                diagnostics,
            );
        }
        Node::Mesh(m) => {
            node::check_mesh(
                m,
                ctx,
                seen_ids,
                referenced_token_ids,
                geom_required,
                diagnostics,
            );
        }
        Node::Polygon(poly) => {
            node::check_polygon(poly, ctx, seen_ids, referenced_token_ids, diagnostics);
        }
        Node::Polyline(poly) => {
            node::check_polyline(poly, ctx, seen_ids, referenced_token_ids, diagnostics);
        }
        Node::Path(path) => {
            node::check_path(path, ctx, seen_ids, referenced_token_ids, diagnostics);
        }
        Node::Instance(inst) => {
            node::check_instance(inst, ctx, seen_ids, referenced_token_ids, diagnostics);
        }
        Node::Field(field) => {
            node::check_field(
                field,
                ctx,
                seen_ids,
                referenced_token_ids,
                parent_ctx,
                diagnostics,
            );
        }
        Node::Toc(toc) => {
            node::check_toc(
                toc,
                ctx,
                seen_ids,
                referenced_token_ids,
                parent_ctx,
                diagnostics,
            );
        }
        Node::Footnote(footnote) => {
            node::check_footnote(footnote, ctx, seen_ids, referenced_token_ids, diagnostics);
        }
        Node::Connector(c) => {
            node::check_connector(c, ctx, seen_ids, referenced_token_ids, diagnostics);
        }

        Node::Frame(f) => {
            node::check_frame(
                f,
                ctx,
                seen_ids,
                referenced_token_ids,
                geom_required,
                parent_ctx,
                diagnostics,
            );

            node::check_frame_layout(
                f,
                layout_site,
                &mut node::shared::TokenEnv {
                    referenced: referenced_token_ids,
                    resolved: ctx.resolved_tokens,
                },
                diagnostics,
            );

            // Recurse into children, passing the SAME seen_ids so that
            // nested ids participate in the global uniqueness check. Direct
            // children of a row/column/grid frame have layout-supplied
            // geometry, so their own x/y/w/h are optional.
            let child_flow_parent = node::FlowParent::of_frame(f);
            let children_in_flow = child_flow_parent.is_some();

            // This frame's own px box; children are checked for overflow
            // against it. A missing/bad x/y/w/h or a layout frame gives None.
            let frame_box = placement::frame_child_box(f, pos.origin, pos.page_px_bounds);
            let child_origin = node.child_origin(pos.origin, ctx.resolved_tokens);

            // Validate this frame's sibling-anchor graph (one scope = its
            // direct children) once, before descending.
            check_sibling_anchors(&f.children, diagnostics);

            for child in &f.children {
                walk_node(
                    child,
                    ctx,
                    seen_ids,
                    referenced_token_ids,
                    WalkPos {
                        page_px_bounds: pos.page_px_bounds,
                        in_flow_parent: children_in_flow,
                        flow_parent: child_flow_parent,
                        enclosing_frame: frame_box,
                        origin: child_origin,
                        // A frame is always an anchor-parent container with a
                        // usable box (its geometry is required + validated).
                        in_container: true,
                        parent_box_known: true,
                        exempt,
                    },
                    diagnostics,
                );
            }
        }

        Node::Group(g) => {
            node::check_group(
                g,
                ctx,
                seen_ids,
                referenced_token_ids,
                parent_ctx,
                diagnostics,
            );

            // A group is an anchor-parent container; its reference box is
            // usable only when it declares both `w` and `h`.
            let group_box_known = g.w.is_some() && g.h.is_some();

            // Validate this group's sibling-anchor graph (one scope = its
            // direct children) once, before descending.
            check_sibling_anchors(&g.children, diagnostics);

            // Recurse into children, passing the SAME seen_ids so that
            // nested ids participate in the global uniqueness check. Groups do
            // not lay out children, so geometry remains required for them.
            // Groups don't clip, so the enclosing frame (if any) is propagated
            // unchanged: a group inside a frame still has the frame as the
            // clipping ancestor. The group adds its child space to the origin.
            let child_origin = node.child_origin(pos.origin, ctx.resolved_tokens);
            for child in &g.children {
                walk_node(
                    child,
                    ctx,
                    seen_ids,
                    referenced_token_ids,
                    WalkPos {
                        page_px_bounds: pos.page_px_bounds,
                        in_flow_parent: false,
                        flow_parent: None,
                        enclosing_frame: pos.enclosing_frame,
                        origin: child_origin,
                        in_container: true,
                        parent_box_known: group_box_known,
                        exempt,
                    },
                    diagnostics,
                );
            }
        }

        Node::Table(t) => {
            node::check_table(
                t,
                ctx,
                seen_ids,
                referenced_token_ids,
                geom_required,
                parent_ctx,
                diagnostics,
            );

            // Recurse into every cell's children with the normal walk so nested
            // node ids are registered/validated. A table cell positions and
            // sizes its children (auto-box/wrap/align), exactly like a
            // `frame layout="grid"/"flow"`, so cell children are flow-positioned
            // and their x/y/w/h are OPTIONAL.
            for row in &t.rows {
                for cell in &row.cells {
                    for child in &cell.children {
                        walk_node(
                            child,
                            ctx,
                            seen_ids,
                            referenced_token_ids,
                            WalkPos {
                                page_px_bounds: pos.page_px_bounds,
                                in_flow_parent: true,
                                flow_parent: None,
                                enclosing_frame: pos.enclosing_frame,
                                origin: pos.origin,
                                // A table cell is NOT an anchor-parent
                                // container; its children's direct parent is the
                                // cell, so anchor-parent there is unresolvable.
                                in_container: false,
                                parent_box_known: false,
                                exempt,
                            },
                            diagnostics,
                        );
                    }
                }
            }
        }

        Node::Unknown(u) => {
            node::check_unknown(u, seen_ids, diagnostics);

            // Recurse into children so nested KNOWN nodes (e.g. a `rect` inside
            // an unknown parent) are still validated for token refs, duplicate
            // ids, etc. The unknown parent's layout semantics are unknown, so
            // children must NOT trigger `node.missing_geometry`: pass
            // `in_flow_parent = true` to make their geometry optional.
            for child in &u.children {
                walk_node(
                    child,
                    ctx,
                    seen_ids,
                    referenced_token_ids,
                    WalkPos {
                        page_px_bounds: pos.page_px_bounds,
                        in_flow_parent: true,
                        flow_parent: None,
                        enclosing_frame: pos.enclosing_frame,
                        origin: pos.origin,
                        // An unknown parent is not a known anchor-parent container.
                        in_container: false,
                        parent_box_known: false,
                        exempt,
                    },
                    diagnostics,
                );
            }
        }
    }
}
