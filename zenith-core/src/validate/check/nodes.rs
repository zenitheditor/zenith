//! Per-node validation: the recursive [`walk_node`] dispatcher, the
//! polygon/polyline checks, geometry validation, and the off_canvas geometry
//! helpers.

use std::collections::{BTreeMap, HashSet};

use crate::ast::node::{Node, PolygonNode, PolylineNode};
use crate::ast::value::{Dimension, Unit, dim_to_px};
use crate::diagnostics::Diagnostic;
use crate::tokens::ResolvedToken;

use super::register_id;
use super::visual::{VisualExpect, check_visual_prop};

/// Recursively walk a [`Node`], collecting all diagnostics.
///
/// `referenced_token_ids` accumulates every token id actually used so that
/// the unused-token check (done after the walk) can diff against defined ids.
///
/// `page_px_bounds` is `Some((page_w, page_h))` when the page's dimensions
/// resolved successfully; `None` means off_canvas checks are skipped for this
/// page (page unit was bad — already diagnosed).
///
/// # Known limitation
/// Recursion through `Node::Group` and `Node::Frame` children has no depth
/// guard.  Pathologically deep trees can overflow the stack.  This is an
/// accepted v0 limitation.
#[allow(clippy::too_many_arguments)]
pub(super) fn walk_node(
    node: &Node,
    seen_ids: &mut HashSet<String>,
    referenced_token_ids: &mut HashSet<String>,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
    declared_asset_ids: &HashSet<String>,
    declared_style_ids: &HashSet<String>,
    page_px_bounds: Option<(f64, f64)>,
    in_flow_parent: bool,
    enclosing_frame: Option<(f64, f64, f64, f64)>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // ── frame.child_overflow advisory ─────────────────────────────────────
    // When this node is a direct (or group-nested) child of a frame whose px
    // box resolved, advise if the child's AUTHORED bbox protrudes beyond the
    // frame box on any side. `node_bbox` returns None for flow-supplied
    // (missing) geometry, so such children are naturally skipped.
    if let Some((fx, fy, fw, fh)) = enclosing_frame
        && let Some((page_w, page_h)) = page_px_bounds
        && let Some((nx, ny, nw, nh)) = node_bbox(node, page_w, page_h)
    {
        const EPSILON: f64 = 0.5;
        let over_left = nx < fx - EPSILON;
        let over_top = ny < fy - EPSILON;
        let over_right = nx + nw > fx + fw + EPSILON;
        let over_bottom = ny + nh > fy + fh + EPSILON;
        if over_left || over_top || over_right || over_bottom {
            let (node_id, node_span) = node_id_and_span(node);
            diagnostics.push(Diagnostic::advisory(
                "frame.child_overflow",
                format!(
                    "node '{}' (bbox {nx}, {ny}, {nw}, {nh}) protrudes beyond its \
                     enclosing frame (bbox {fx}, {fy}, {fw}, {fh})",
                    node_id
                ),
                node_span,
                Some(node_id.to_owned()),
            ));
        }
    }

    // Direct children of a `layout="flow"` frame have their x/y (and, when
    // omitted, w/h) supplied by the flow algorithm, so geometry is optional.
    let geom_required = !in_flow_parent;
    // ── off_canvas advisory ───────────────────────────────────────────────
    // Check whether the node's authored bounding box exceeds the page rect
    // [0, 0, page_w, page_h]. This uses authored coordinates only — group
    // translation offsets are NOT accumulated (v0 advisory behavior; render-
    // time offset accumulation is a scene-compiler concern, not validation).
    if let Some((page_w, page_h)) = page_px_bounds
        && let Some((nx, ny, nw, nh)) = node_bbox(node, page_w, page_h)
        && (nx < 0.0 || ny < 0.0 || nx + nw > page_w || ny + nh > page_h)
    {
        let (node_id, node_span) = node_id_and_span(node);
        diagnostics.push(Diagnostic::advisory(
            "off_canvas",
            format!(
                "node '{}' extends outside the page bounds (0, 0, {page_w}, {page_h})",
                node_id
            ),
            node_span,
            Some(node_id.to_owned()),
        ));
    }

    match node {
        Node::Rect(r) => {
            register_id(&r.id, seen_ids, diagnostics);
            check_style_ref(
                &r.id,
                r.style.as_deref(),
                declared_style_ids,
                r.source_span,
                diagnostics,
            );

            // Required geometry: x, y, w, h must all be present.
            check_optional_dim(
                &r.id,
                "x",
                r.x.as_ref(),
                geom_required,
                r.source_span,
                diagnostics,
            );
            check_optional_dim(
                &r.id,
                "y",
                r.y.as_ref(),
                geom_required,
                r.source_span,
                diagnostics,
            );
            check_optional_dim(
                &r.id,
                "w",
                r.w.as_ref(),
                geom_required,
                r.source_span,
                diagnostics,
            );
            check_optional_dim(
                &r.id,
                "h",
                r.h.as_ref(),
                geom_required,
                r.source_span,
                diagnostics,
            );

            // Visual properties.
            check_visual_prop(
                &r.id,
                "fill",
                r.fill.as_ref(),
                VisualExpect::ColorOrGradient,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
            check_visual_prop(
                &r.id,
                "stroke",
                r.stroke.as_ref(),
                VisualExpect::Color,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
            check_visual_prop(
                &r.id,
                "stroke-width",
                r.stroke_width.as_ref(),
                VisualExpect::Dimension,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
            check_visual_prop(
                &r.id,
                "radius",
                r.radius.as_ref(),
                VisualExpect::Dimension,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
            check_visual_prop(
                &r.id,
                "shadow",
                r.shadow.as_ref(),
                VisualExpect::Shadow,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );

            // Unknown properties.
            for prop_name in r.unknown_props.keys() {
                diagnostics.push(Diagnostic::warning(
                    "node.unknown_property",
                    format!(
                        "rect '{}': unknown property '{}' (version-relative; \
                         may be valid in a later schema version)",
                        r.id, prop_name
                    ),
                    r.source_span,
                    Some(r.id.clone()),
                ));
            }
        }

        Node::Ellipse(e) => {
            register_id(&e.id, seen_ids, diagnostics);
            check_style_ref(
                &e.id,
                e.style.as_deref(),
                declared_style_ids,
                e.source_span,
                diagnostics,
            );

            // Required geometry: x, y, w, h must all be present.
            check_optional_dim(
                &e.id,
                "x",
                e.x.as_ref(),
                geom_required,
                e.source_span,
                diagnostics,
            );
            check_optional_dim(
                &e.id,
                "y",
                e.y.as_ref(),
                geom_required,
                e.source_span,
                diagnostics,
            );
            check_optional_dim(
                &e.id,
                "w",
                e.w.as_ref(),
                geom_required,
                e.source_span,
                diagnostics,
            );
            check_optional_dim(
                &e.id,
                "h",
                e.h.as_ref(),
                geom_required,
                e.source_span,
                diagnostics,
            );

            // Visual properties.
            check_visual_prop(
                &e.id,
                "fill",
                e.fill.as_ref(),
                VisualExpect::ColorOrGradient,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
            check_visual_prop(
                &e.id,
                "stroke",
                e.stroke.as_ref(),
                VisualExpect::Color,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
            check_visual_prop(
                &e.id,
                "stroke-width",
                e.stroke_width.as_ref(),
                VisualExpect::Dimension,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
            check_visual_prop(
                &e.id,
                "shadow",
                e.shadow.as_ref(),
                VisualExpect::Shadow,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );

            // Unknown properties.
            for prop_name in e.unknown_props.keys() {
                diagnostics.push(Diagnostic::warning(
                    "node.unknown_property",
                    format!(
                        "ellipse '{}': unknown property '{}' (version-relative; \
                         may be valid in a later schema version)",
                        e.id, prop_name
                    ),
                    e.source_span,
                    Some(e.id.clone()),
                ));
            }
        }

        Node::Line(l) => {
            register_id(&l.id, seen_ids, diagnostics);
            check_style_ref(
                &l.id,
                l.style.as_deref(),
                declared_style_ids,
                l.source_span,
                diagnostics,
            );

            // Required geometry: x1, y1, x2, y2 must all be present.
            check_optional_dim(&l.id, "x1", l.x1.as_ref(), true, l.source_span, diagnostics);
            check_optional_dim(&l.id, "y1", l.y1.as_ref(), true, l.source_span, diagnostics);
            check_optional_dim(&l.id, "x2", l.x2.as_ref(), true, l.source_span, diagnostics);
            check_optional_dim(&l.id, "y2", l.y2.as_ref(), true, l.source_span, diagnostics);

            // Visual properties (stroke-only; no fill for line).
            // stroke is optional — only type-checked if present (a stroke-less
            // line draws nothing, but it is not an error to omit stroke).
            check_visual_prop(
                &l.id,
                "stroke",
                l.stroke.as_ref(),
                VisualExpect::Color,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
            check_visual_prop(
                &l.id,
                "stroke-width",
                l.stroke_width.as_ref(),
                VisualExpect::Dimension,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );

            // Unknown properties.
            for prop_name in l.unknown_props.keys() {
                diagnostics.push(Diagnostic::warning(
                    "node.unknown_property",
                    format!(
                        "line '{}': unknown property '{}' (version-relative; \
                         may be valid in a later schema version)",
                        l.id, prop_name
                    ),
                    l.source_span,
                    Some(l.id.clone()),
                ));
            }
        }

        Node::Text(t) => {
            register_id(&t.id, seen_ids, diagnostics);
            check_style_ref(
                &t.id,
                t.style.as_deref(),
                declared_style_ids,
                t.source_span,
                diagnostics,
            );

            // Required geometry.
            check_optional_dim(
                &t.id,
                "x",
                t.x.as_ref(),
                geom_required,
                t.source_span,
                diagnostics,
            );
            check_optional_dim(
                &t.id,
                "y",
                t.y.as_ref(),
                geom_required,
                t.source_span,
                diagnostics,
            );
            check_optional_dim(
                &t.id,
                "w",
                t.w.as_ref(),
                geom_required,
                t.source_span,
                diagnostics,
            );
            check_optional_dim(
                &t.id,
                "h",
                t.h.as_ref(),
                geom_required,
                t.source_span,
                diagnostics,
            );

            // Visual properties.
            check_visual_prop(
                &t.id,
                "fill",
                t.fill.as_ref(),
                VisualExpect::Color,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
            check_visual_prop(
                &t.id,
                "font-family",
                t.font_family.as_ref(),
                VisualExpect::FontFamily,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
            check_visual_prop(
                &t.id,
                "font-size",
                t.font_size.as_ref(),
                VisualExpect::Dimension,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
            check_visual_prop(
                &t.id,
                "font-weight",
                t.font_weight.as_ref(),
                VisualExpect::FontWeight,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
            check_visual_prop(
                &t.id,
                "shadow",
                t.shadow.as_ref(),
                VisualExpect::Shadow,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );

            // Per-span visual properties. Spans inherit the node id as their
            // subject so token refs in `span ... fill=(token)".." font-weight=..`
            // are registered (otherwise the token is falsely flagged unused) and
            // get the same existence/type/raw-literal validation as node props.
            for span in &t.spans {
                check_visual_prop(
                    &t.id,
                    "fill",
                    span.fill.as_ref(),
                    VisualExpect::Color,
                    referenced_token_ids,
                    resolved_tokens,
                    diagnostics,
                );
                check_visual_prop(
                    &t.id,
                    "font-weight",
                    span.font_weight.as_ref(),
                    VisualExpect::FontWeight,
                    referenced_token_ids,
                    resolved_tokens,
                    diagnostics,
                );
            }

            // Unknown properties.
            for prop_name in t.unknown_props.keys() {
                diagnostics.push(Diagnostic::warning(
                    "node.unknown_property",
                    format!(
                        "text '{}': unknown property '{}' (version-relative; \
                         may be valid in a later schema version)",
                        t.id, prop_name
                    ),
                    t.source_span,
                    Some(t.id.clone()),
                ));
            }
        }

        Node::Code(c) => {
            register_id(&c.id, seen_ids, diagnostics);
            check_style_ref(
                &c.id,
                c.style.as_deref(),
                declared_style_ids,
                c.source_span,
                diagnostics,
            );

            // Geometry (advisory box for v0; only unit-checked if present).
            check_optional_dim(
                &c.id,
                "x",
                c.x.as_ref(),
                geom_required,
                c.source_span,
                diagnostics,
            );
            check_optional_dim(
                &c.id,
                "y",
                c.y.as_ref(),
                geom_required,
                c.source_span,
                diagnostics,
            );
            check_optional_dim(
                &c.id,
                "w",
                c.w.as_ref(),
                geom_required,
                c.source_span,
                diagnostics,
            );
            check_optional_dim(
                &c.id,
                "h",
                c.h.as_ref(),
                geom_required,
                c.source_span,
                diagnostics,
            );

            // Visual properties (mirror text; overflow is not enum-validated,
            // matching how text.overflow is currently handled).
            check_visual_prop(
                &c.id,
                "fill",
                c.fill.as_ref(),
                VisualExpect::Color,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
            check_visual_prop(
                &c.id,
                "font-family",
                c.font_family.as_ref(),
                VisualExpect::FontFamily,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
            check_visual_prop(
                &c.id,
                "font-size",
                c.font_size.as_ref(),
                VisualExpect::Dimension,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
            check_visual_prop(
                &c.id,
                "font-weight",
                c.font_weight.as_ref(),
                VisualExpect::FontWeight,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );

            // Unknown properties.
            for prop_name in c.unknown_props.keys() {
                diagnostics.push(Diagnostic::warning(
                    "node.unknown_property",
                    format!(
                        "code '{}': unknown property '{}' (version-relative; \
                         may be valid in a later schema version)",
                        c.id, prop_name
                    ),
                    c.source_span,
                    Some(c.id.clone()),
                ));
            }
        }

        Node::Frame(f) => {
            register_id(&f.id, seen_ids, diagnostics);
            check_style_ref(
                &f.id,
                f.style.as_deref(),
                declared_style_ids,
                f.source_span,
                diagnostics,
            );

            // Frames REQUIRE all four geometry dimensions (unlike groups).
            check_optional_dim(
                &f.id,
                "x",
                f.x.as_ref(),
                geom_required,
                f.source_span,
                diagnostics,
            );
            check_optional_dim(
                &f.id,
                "y",
                f.y.as_ref(),
                geom_required,
                f.source_span,
                diagnostics,
            );
            check_optional_dim(
                &f.id,
                "w",
                f.w.as_ref(),
                geom_required,
                f.source_span,
                diagnostics,
            );
            check_optional_dim(
                &f.id,
                "h",
                f.h.as_ref(),
                geom_required,
                f.source_span,
                diagnostics,
            );

            // Unknown properties.
            for prop_name in f.unknown_props.keys() {
                diagnostics.push(Diagnostic::warning(
                    "node.unknown_property",
                    format!(
                        "frame '{}': unknown property '{}' (version-relative; \
                         may be valid in a later schema version)",
                        f.id, prop_name
                    ),
                    f.source_span,
                    Some(f.id.clone()),
                ));
            }

            // Recurse into children, passing the SAME seen_ids so that
            // nested ids participate in the global uniqueness check. Direct
            // children of a flow frame have flow-supplied geometry.
            let children_in_flow = f.layout.as_deref() == Some("flow");

            // Compute this frame's own px box; children are checked for
            // overflow against it. If any of x/y/w/h is missing or has a bad
            // unit, pass None so no spurious overflow advisory is produced.
            let frame_box = match page_px_bounds {
                Some((page_w, page_h)) => {
                    f.x.as_ref()
                        .and_then(|d| resolve_axis(d, page_w))
                        .zip(f.y.as_ref().and_then(|d| resolve_axis(d, page_h)))
                        .zip(f.w.as_ref().and_then(|d| resolve_axis(d, page_w)))
                        .zip(f.h.as_ref().and_then(|d| resolve_axis(d, page_h)))
                        .map(|(((x, y), w), h)| (x, y, w, h))
                }
                None => None,
            };

            for child in &f.children {
                walk_node(
                    child,
                    seen_ids,
                    referenced_token_ids,
                    resolved_tokens,
                    declared_asset_ids,
                    declared_style_ids,
                    page_px_bounds,
                    children_in_flow,
                    frame_box,
                    diagnostics,
                );
            }
        }

        Node::Group(g) => {
            register_id(&g.id, seen_ids, diagnostics);
            check_style_ref(
                &g.id,
                g.style.as_deref(),
                declared_style_ids,
                g.source_span,
                diagnostics,
            );

            // Groups have NO required geometry — x/y/w/h are all advisory.

            // Unknown properties.
            for prop_name in g.unknown_props.keys() {
                diagnostics.push(Diagnostic::warning(
                    "node.unknown_property",
                    format!(
                        "group '{}': unknown property '{}' (version-relative; \
                         may be valid in a later schema version)",
                        g.id, prop_name
                    ),
                    g.source_span,
                    Some(g.id.clone()),
                ));
            }

            // Recurse into children, passing the SAME seen_ids so that
            // nested ids participate in the global uniqueness check. Groups do
            // not lay out children, so geometry remains required for them.
            // Groups don't clip, so the enclosing frame (if any) is propagated
            // unchanged: a group inside a frame still has the frame as the
            // clipping ancestor.
            for child in &g.children {
                walk_node(
                    child,
                    seen_ids,
                    referenced_token_ids,
                    resolved_tokens,
                    declared_asset_ids,
                    declared_style_ids,
                    page_px_bounds,
                    false,
                    enclosing_frame,
                    diagnostics,
                );
            }
        }

        Node::Image(img) => {
            register_id(&img.id, seen_ids, diagnostics);
            check_style_ref(
                &img.id,
                img.style.as_deref(),
                declared_style_ids,
                img.source_span,
                diagnostics,
            );

            // Required geometry: x, y, w, h must all be present (mirror rect).
            check_optional_dim(
                &img.id,
                "x",
                img.x.as_ref(),
                geom_required,
                img.source_span,
                diagnostics,
            );
            check_optional_dim(
                &img.id,
                "y",
                img.y.as_ref(),
                geom_required,
                img.source_span,
                diagnostics,
            );
            check_optional_dim(
                &img.id,
                "w",
                img.w.as_ref(),
                geom_required,
                img.source_span,
                diagnostics,
            );
            check_optional_dim(
                &img.id,
                "h",
                img.h.as_ref(),
                geom_required,
                img.source_span,
                diagnostics,
            );

            // The referenced asset must exist in the document's assets block.
            if !declared_asset_ids.contains(&img.asset) {
                diagnostics.push(Diagnostic::error(
                    "asset.unknown_reference",
                    format!(
                        "image '{}': references asset '{}' which is not declared in the \
                         assets block",
                        img.id, img.asset
                    ),
                    img.source_span,
                    Some(img.id.clone()),
                ));
            }

            // Validate fit (version-relative; forward-compat warning).
            if let Some(fit) = &img.fit
                && !matches!(fit.as_str(), "contain" | "cover" | "stretch" | "none")
            {
                diagnostics.push(Diagnostic::warning(
                    "image.invalid_fit",
                    format!(
                        "image '{}': unrecognized fit '{}' (version-relative; allowed \
                         values are contain, cover, stretch, none)",
                        img.id, fit
                    ),
                    img.source_span,
                    Some(img.id.clone()),
                ));
            }

            // Visual properties.
            // clip-radius is a dimension token (mirror rect `radius`); only
            // meaningful for clip="rounded" but type-checked whenever present.
            check_visual_prop(
                &img.id,
                "clip-radius",
                img.clip_radius.as_ref(),
                VisualExpect::Dimension,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );
            check_visual_prop(
                &img.id,
                "shadow",
                img.shadow.as_ref(),
                VisualExpect::Shadow,
                referenced_token_ids,
                resolved_tokens,
                diagnostics,
            );

            // Unknown properties.
            for prop_name in img.unknown_props.keys() {
                diagnostics.push(Diagnostic::warning(
                    "node.unknown_property",
                    format!(
                        "image '{}': unknown property '{}' (version-relative; \
                         may be valid in a later schema version)",
                        img.id, prop_name
                    ),
                    img.source_span,
                    Some(img.id.clone()),
                ));
            }
            // Image is a leaf — no child recursion.
        }

        Node::Polygon(poly) => {
            check_polygon(
                poly,
                seen_ids,
                referenced_token_ids,
                resolved_tokens,
                declared_style_ids,
                diagnostics,
            );
        }

        Node::Polyline(poly) => {
            check_polyline(
                poly,
                seen_ids,
                referenced_token_ids,
                resolved_tokens,
                declared_style_ids,
                diagnostics,
            );
        }

        Node::Unknown(u) => {
            diagnostics.push(Diagnostic::warning(
                "node.unknown_kind",
                format!(
                    "unknown node kind '{}' (forward-compatibility; \
                     this kind may be valid in a later schema version)",
                    u.kind
                ),
                u.source_span,
                None,
            ));
            // Unknown nodes have no children in the v0 AST; nothing to recurse.
        }
    }
}

// ── off_canvas geometry helpers ───────────────────────────────────────────────

/// Resolve a single geometry axis dimension to pixels.
///
/// `Pct` is resolved against `basis` (e.g. page_w for x/w, page_h for y/h).
/// All other convertible units delegate to [`dim_to_px`]; `None` on failure.
fn resolve_axis(dim: &Dimension, basis: f64) -> Option<f64> {
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
pub(super) fn node_bbox(node: &Node, page_w: f64, page_h: f64) -> Option<(f64, f64, f64, f64)> {
    match node {
        Node::Rect(n) => {
            let x = resolve_axis(n.x.as_ref()?, page_w)?;
            let y = resolve_axis(n.y.as_ref()?, page_h)?;
            let w = resolve_axis(n.w.as_ref()?, page_w)?;
            let h = resolve_axis(n.h.as_ref()?, page_h)?;
            Some((x, y, w, h))
        }
        Node::Ellipse(n) => {
            let x = resolve_axis(n.x.as_ref()?, page_w)?;
            let y = resolve_axis(n.y.as_ref()?, page_h)?;
            let w = resolve_axis(n.w.as_ref()?, page_w)?;
            let h = resolve_axis(n.h.as_ref()?, page_h)?;
            Some((x, y, w, h))
        }
        Node::Image(n) => {
            let x = resolve_axis(n.x.as_ref()?, page_w)?;
            let y = resolve_axis(n.y.as_ref()?, page_h)?;
            let w = resolve_axis(n.w.as_ref()?, page_w)?;
            let h = resolve_axis(n.h.as_ref()?, page_h)?;
            Some((x, y, w, h))
        }
        Node::Frame(n) => {
            let x = resolve_axis(n.x.as_ref()?, page_w)?;
            let y = resolve_axis(n.y.as_ref()?, page_h)?;
            let w = resolve_axis(n.w.as_ref()?, page_w)?;
            let h = resolve_axis(n.h.as_ref()?, page_h)?;
            Some((x, y, w, h))
        }
        Node::Text(n) => {
            let x = resolve_axis(n.x.as_ref()?, page_w)?;
            let y = resolve_axis(n.y.as_ref()?, page_h)?;
            let w = resolve_axis(n.w.as_ref()?, page_w)?;
            let h = resolve_axis(n.h.as_ref()?, page_h)?;
            Some((x, y, w, h))
        }
        Node::Code(n) => {
            let x = resolve_axis(n.x.as_ref()?, page_w)?;
            let y = resolve_axis(n.y.as_ref()?, page_h)?;
            let w = resolve_axis(n.w.as_ref()?, page_w)?;
            let h = resolve_axis(n.h.as_ref()?, page_h)?;
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
        // Groups have no authoritative bbox in v0 — children are checked individually.
        Node::Group(_) | Node::Unknown(_) => None,
    }
}

/// Compute the bounding box of a slice of [`Point`]s, resolving each coordinate
/// against the given page axis bases.
///
/// Returns `Some((min_x, min_y, w, h))` when at least one point resolves
/// successfully, `None` when no point has resolvable coordinates.
fn points_bbox(
    points: &[crate::ast::node::Point],
    page_w: f64,
    page_h: f64,
) -> Option<(f64, f64, f64, f64)> {
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
            min_x = min_x.min(px_val);
            min_y = min_y.min(py_val);
            max_x = max_x.max(px_val);
            max_y = max_y.max(py_val);
            any = true;
        }
    }
    if any {
        Some((min_x, min_y, max_x - min_x, max_y - min_y))
    } else {
        None
    }
}

/// Extract the string id and source span from any node variant.
pub(super) fn node_id_and_span(node: &Node) -> (&str, Option<crate::ast::Span>) {
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
        Node::Unknown(n) => (&n.kind, n.source_span),
    }
}

// ── polygon / polyline validation ─────────────────────────────────────────────

fn check_polygon(
    poly: &PolygonNode,
    seen_ids: &mut HashSet<String>,
    referenced_token_ids: &mut HashSet<String>,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
    declared_style_ids: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    register_id(&poly.id, seen_ids, diagnostics);
    check_style_ref(
        &poly.id,
        poly.style.as_deref(),
        declared_style_ids,
        poly.source_span,
        diagnostics,
    );

    // Validate each point's x and y (both must be present with a known unit).
    for (idx, pt) in poly.points.iter().enumerate() {
        let x_label = format!("point[{idx}].x");
        let y_label = format!("point[{idx}].y");
        check_optional_dim(
            &poly.id,
            &x_label,
            pt.x.as_ref(),
            true,
            poly.source_span,
            diagnostics,
        );
        check_optional_dim(
            &poly.id,
            &y_label,
            pt.y.as_ref(),
            true,
            poly.source_span,
            diagnostics,
        );
    }

    // polygon requires at least 3 points.
    if poly.points.len() < 3 {
        diagnostics.push(Diagnostic::error(
            "shape.insufficient_points",
            format!(
                "polygon '{}': requires at least 3 points, got {}",
                poly.id,
                poly.points.len()
            ),
            poly.source_span,
            Some(poly.id.clone()),
        ));
    }

    // Visual properties.
    check_visual_prop(
        &poly.id,
        "fill",
        poly.fill.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &poly.id,
        "stroke",
        poly.stroke.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &poly.id,
        "stroke-width",
        poly.stroke_width.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    // fill-rule: only "nonzero" and "evenodd" are valid.
    if let Some(fr) = &poly.fill_rule
        && !matches!(fr.as_str(), "nonzero" | "evenodd")
    {
        diagnostics.push(Diagnostic::warning(
            "node.unknown_property",
            format!(
                "polygon '{}': unrecognized fill-rule '{}' (version-relative; \
                 allowed values are nonzero, evenodd)",
                poly.id, fr
            ),
            poly.source_span,
            Some(poly.id.clone()),
        ));
    }

    // Unknown properties.
    for prop_name in poly.unknown_props.keys() {
        diagnostics.push(Diagnostic::warning(
            "node.unknown_property",
            format!(
                "polygon '{}': unknown property '{}' (version-relative; \
                 may be valid in a later schema version)",
                poly.id, prop_name
            ),
            poly.source_span,
            Some(poly.id.clone()),
        ));
    }
    // polygon is a LEAF: no child-node recursion (points are sub-data).
}

fn check_polyline(
    poly: &PolylineNode,
    seen_ids: &mut HashSet<String>,
    referenced_token_ids: &mut HashSet<String>,
    resolved_tokens: &BTreeMap<String, ResolvedToken>,
    declared_style_ids: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    register_id(&poly.id, seen_ids, diagnostics);
    check_style_ref(
        &poly.id,
        poly.style.as_deref(),
        declared_style_ids,
        poly.source_span,
        diagnostics,
    );

    // Validate each point's x and y.
    for (idx, pt) in poly.points.iter().enumerate() {
        let x_label = format!("point[{idx}].x");
        let y_label = format!("point[{idx}].y");
        check_optional_dim(
            &poly.id,
            &x_label,
            pt.x.as_ref(),
            true,
            poly.source_span,
            diagnostics,
        );
        check_optional_dim(
            &poly.id,
            &y_label,
            pt.y.as_ref(),
            true,
            poly.source_span,
            diagnostics,
        );
    }

    // polyline requires at least 2 points.
    if poly.points.len() < 2 {
        diagnostics.push(Diagnostic::error(
            "shape.insufficient_points",
            format!(
                "polyline '{}': requires at least 2 points, got {}",
                poly.id,
                poly.points.len()
            ),
            poly.source_span,
            Some(poly.id.clone()),
        ));
    }

    // Visual properties.
    check_visual_prop(
        &poly.id,
        "fill",
        poly.fill.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &poly.id,
        "stroke",
        poly.stroke.as_ref(),
        VisualExpect::Color,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );
    check_visual_prop(
        &poly.id,
        "stroke-width",
        poly.stroke_width.as_ref(),
        VisualExpect::Dimension,
        referenced_token_ids,
        resolved_tokens,
        diagnostics,
    );

    // fill-rule: only "nonzero" and "evenodd" are valid.
    if let Some(fr) = &poly.fill_rule
        && !matches!(fr.as_str(), "nonzero" | "evenodd")
    {
        diagnostics.push(Diagnostic::warning(
            "node.unknown_property",
            format!(
                "polyline '{}': unrecognized fill-rule '{}' (version-relative; \
                 allowed values are nonzero, evenodd)",
                poly.id, fr
            ),
            poly.source_span,
            Some(poly.id.clone()),
        ));
    }

    // Unknown properties.
    for prop_name in poly.unknown_props.keys() {
        diagnostics.push(Diagnostic::warning(
            "node.unknown_property",
            format!(
                "polyline '{}': unknown property '{}' (version-relative; \
                 may be valid in a later schema version)",
                poly.id, prop_name
            ),
            poly.source_span,
            Some(poly.id.clone()),
        ));
    }
    // polyline is a LEAF: no child-node recursion (points are sub-data).
}

// ── Geometry helpers ──────────────────────────────────────────────────────────

/// Check a single optional geometry dimension (`x`, `y`, `w`, `h`):
/// - absent AND `required` → `node.missing_geometry` (Error).
/// - absent AND NOT `required` (e.g. a direct child of a `layout="flow"`
///   frame, whose position/size is supplied by the flow algorithm) → no
///   diagnostic.
/// - present but `Unit::Unknown` → `node.invalid_geometry` (Error) regardless
///   of `required`.
fn check_optional_dim(
    node_id: &str,
    prop: &str,
    dim: Option<&crate::ast::value::Dimension>,
    required: bool,
    span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match dim {
        None if required => {
            diagnostics.push(Diagnostic::error(
                "node.missing_geometry",
                format!(
                    "node '{}': required geometry property '{}' is missing",
                    node_id, prop
                ),
                span,
                Some(node_id.to_owned()),
            ));
        }
        None => {
            // Flow-positioned child: geometry is supplied by the parent.
        }
        Some(d) if matches!(d.unit, Unit::Unknown(_)) => {
            diagnostics.push(Diagnostic::error(
                "node.invalid_geometry",
                format!(
                    "node '{}': geometry property '{}' has an unrecognized unit; \
                     allowed units are px, pt, pct, deg",
                    node_id, prop
                ),
                span,
                Some(node_id.to_owned()),
            ));
        }
        Some(_) => {
            // valid
        }
    }
}

// ── Style helpers ─────────────────────────────────────────────────────────────

/// Check that a node's `style` attribute references a declared style id.
///
/// Called for every node kind that carries a `style` field.
fn check_style_ref(
    node_id: &str,
    style_opt: Option<&str>,
    declared_style_ids: &HashSet<String>,
    span: Option<crate::ast::Span>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(sid) = style_opt
        && !declared_style_ids.contains(sid)
    {
        diagnostics.push(Diagnostic::error(
            "style.unknown_reference",
            format!(
                "node '{}': references style '{}' which is not declared in the styles block",
                node_id, sid
            ),
            span,
            Some(node_id.to_owned()),
        ));
    }
}
