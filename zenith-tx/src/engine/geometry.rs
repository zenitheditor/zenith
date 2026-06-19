//! Geometry op application: `set_geometry` and `align_nodes`, plus the bbox
//! geometry accessors they share.

use zenith_core::{Diagnostic, Dimension, Document, Node, dim_to_px};

use super::{
    find_node_any_mut, find_node_shared, node_kind_str, px, record_affected, subtree_contains,
};

/// Valid alignment directions for `Op::AlignNodes`.
const VALID_ALIGN_DIRS: &[&str] = &["left", "hcenter", "right", "top", "vcenter", "bottom"];

/// Mutable references to a node's four bbox geometry slots `(x, y, w, h)`.
type GeometryMut<'a> = (
    &'a mut Option<Dimension>,
    &'a mut Option<Dimension>,
    &'a mut Option<Dimension>,
    &'a mut Option<Dimension>,
);

/// Return mutable references to the four bbox geometry fields `(x, y, w, h)`,
/// or `None` for node variants excluded from `set_geometry`.
///
/// The bbox nodes — `Rect`, `Ellipse`, `Frame`, `Image`, `Text`, `Code`, and
/// `Group` — are settable: each carries canonical `x/y/w/h` fields (a text/code
/// node's `x/y/w/h` is its text box; a group's `x/y` is a real translation
/// offset applied to its children at render time).
///
/// `Line` is excluded because it has no bbox — it uses `x1/y1/x2/y2` endpoints.
/// `Polygon` and `Polyline` are excluded because they have no bbox either — their
/// geometry is the `points` list. `Unknown` is excluded because its schema is opaque.
fn node_geometry_mut(node: &mut Node) -> Option<GeometryMut<'_>> {
    match node {
        Node::Rect(r) => Some((&mut r.x, &mut r.y, &mut r.w, &mut r.h)),
        Node::Ellipse(e) => Some((&mut e.x, &mut e.y, &mut e.w, &mut e.h)),
        Node::Frame(f) => Some((&mut f.x, &mut f.y, &mut f.w, &mut f.h)),
        Node::Image(i) => Some((&mut i.x, &mut i.y, &mut i.w, &mut i.h)),
        Node::Text(t) => Some((&mut t.x, &mut t.y, &mut t.w, &mut t.h)),
        Node::Code(c) => Some((&mut c.x, &mut c.y, &mut c.w, &mut c.h)),
        Node::Group(g) => Some((&mut g.x, &mut g.y, &mut g.w, &mut g.h)),
        Node::Line(_) | Node::Polygon(_) | Node::Polyline(_) | Node::Unknown(_) => None,
    }
}

// ── SetGeometry ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_set_geometry(
    node_id: &str,
    x: Option<f64>,
    y: Option<f64>,
    w: Option<f64>,
    h: Option<f64>,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    // Early-out: if every field is None this is a no-op — emit advisory.
    if x.is_none() && y.is_none() && w.is_none() && h.is_none() {
        diagnostics.push(Diagnostic::advisory(
            "tx.noop",
            format!(
                "set_geometry on {:?} specified no fields; document is unchanged",
                node_id
            ),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    }

    match find_node_any_mut(doc, node_id) {
        None => {
            diagnostics.push(Diagnostic::error(
                "tx.unknown_node",
                format!("node {:?} not found in document", node_id),
                None,
                Some(node_id.to_owned()),
            ));
        }
        Some(node) => {
            let kind = node_kind_str(node);
            match node_geometry_mut(node) {
                None => {
                    diagnostics.push(Diagnostic::error(
                        "tx.unsupported_property",
                        format!(
                            "set_geometry is not supported on a {} node (no x/y/w/h)",
                            kind
                        ),
                        None,
                        Some(node_id.to_owned()),
                    ));
                }
                Some((nx, ny, nw, nh)) => {
                    if let Some(v) = x {
                        *nx = Some(px(v));
                    }
                    if let Some(v) = y {
                        *ny = Some(px(v));
                    }
                    if let Some(v) = w {
                        *nw = Some(px(v));
                    }
                    if let Some(v) = h {
                        *nh = Some(px(v));
                    }
                    record_affected(node_id, affected);
                }
            }
        }
    }
}

// ── AlignNodes ────────────────────────────────────────────────────────────────

/// Read the four bbox dimensions of a node as px values, if the node kind
/// supports geometry and all four fields are present and resolvable.
///
/// Returns `Some((x, y, w, h))` or `None` if the node is unsupported, any
/// field is absent, or any unit cannot be converted to px (e.g. `%`, `deg`).
fn read_geometry_px(node: &Node) -> Option<(f64, f64, f64, f64)> {
    let (x, y, w, h) = match node {
        Node::Rect(r) => (r.x.as_ref(), r.y.as_ref(), r.w.as_ref(), r.h.as_ref()),
        Node::Ellipse(e) => (e.x.as_ref(), e.y.as_ref(), e.w.as_ref(), e.h.as_ref()),
        Node::Frame(f) => (f.x.as_ref(), f.y.as_ref(), f.w.as_ref(), f.h.as_ref()),
        Node::Image(i) => (i.x.as_ref(), i.y.as_ref(), i.w.as_ref(), i.h.as_ref()),
        Node::Text(t) => (t.x.as_ref(), t.y.as_ref(), t.w.as_ref(), t.h.as_ref()),
        Node::Code(c) => (c.x.as_ref(), c.y.as_ref(), c.w.as_ref(), c.h.as_ref()),
        Node::Group(g) => (g.x.as_ref(), g.y.as_ref(), g.w.as_ref(), g.h.as_ref()),
        _ => return None,
    };
    let resolve = |d: Option<&Dimension>| -> Option<f64> {
        d.and_then(|dim| dim_to_px(dim.value, &dim.unit))
    };
    Some((resolve(x)?, resolve(y)?, resolve(w)?, resolve(h)?))
}

pub(super) fn apply_align_nodes(
    node_ids: &[String],
    align: &str,
    anchor: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    // Validate align value.
    if !VALID_ALIGN_DIRS.contains(&align) {
        diagnostics.push(Diagnostic::error(
            "tx.unsupported_property",
            format!("align_nodes: unknown align {:?}", align),
            None,
            None,
        ));
        return;
    }

    // `anchor` is "page", "selection", or a node id (align relative to that
    // node's bbox). A node-id anchor is resolved when the reference rectangle is
    // computed below; an unknown id is rejected there.

    // ── Phase 1: shared scan — gather bbox and check existence ────────────────
    //
    // We need a shared borrow to read geometry before taking the exclusive borrow
    // to write back. Collect (id, x, y, w, h) for every alignable node.
    // Nodes that are not found or lack resolvable geometry are skipped with
    // advisories; the loop continues so the remaining nodes are still aligned.

    struct NodeBbox {
        id: String,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }

    let mut alignable: Vec<NodeBbox> = Vec::new();

    for node_id in node_ids {
        // Shared-borrow scan across all pages.
        let found: Option<Option<(f64, f64, f64, f64)>> = 'page_scan: {
            for page in doc.body.pages.iter() {
                if let Some(node) = find_node_shared(&page.children, node_id) {
                    break 'page_scan Some(read_geometry_px(node));
                }
            }
            None // not found in any page
        };

        match found {
            None => {
                // Node not found at all.
                diagnostics.push(Diagnostic::error(
                    "tx.unknown_node",
                    format!("align_nodes: node {:?} not found in document", node_id),
                    None,
                    Some(node_id.clone()),
                ));
            }
            Some(None) => {
                // Node found but geometry is unresolvable (wrong kind or missing/pct field).
                // Use Warning so the caller sees AcceptedWithWarnings and knows a
                // node was silently skipped.
                diagnostics.push(Diagnostic::warning(
                    "tx.unsupported_property",
                    format!(
                        "align_nodes: node {:?} has no resolvable x/y/w/h geometry; skipped",
                        node_id
                    ),
                    None,
                    Some(node_id.clone()),
                ));
            }
            Some(Some((x, y, w, h))) => {
                alignable.push(NodeBbox {
                    id: node_id.clone(),
                    x,
                    y,
                    w,
                    h,
                });
            }
        }
    }

    // Need at least one alignable node to proceed.
    if alignable.is_empty() {
        diagnostics.push(Diagnostic::advisory(
            "tx.noop",
            "align_nodes: no alignable nodes with resolvable geometry; document is unchanged"
                .to_owned(),
            None,
            None,
        ));
        return;
    }

    // ── Compute the reference rectangle ───────────────────────────────────────

    let (ref_left, ref_right, ref_top, ref_bottom) = if anchor == "page" {
        // Find the page that contains the first alignable node.
        let first_id = &alignable[0].id;
        let page_opt = doc
            .body
            .pages
            .iter()
            .find(|page| page.children.iter().any(|n| subtree_contains(n, first_id)));
        match page_opt {
            None => {
                diagnostics.push(Diagnostic::error(
                    "tx.invalid_parent",
                    format!(
                        "align_nodes: could not locate page containing node {:?}",
                        first_id
                    ),
                    None,
                    Some(first_id.clone()),
                ));
                return;
            }
            Some(page) => {
                let pw = dim_to_px(page.width.value, &page.width.unit);
                let ph = dim_to_px(page.height.value, &page.height.unit);
                match (pw, ph) {
                    (Some(w), Some(h)) => (0.0_f64, w, 0.0_f64, h),
                    _ => {
                        diagnostics.push(Diagnostic::error(
                            "tx.invalid_parent",
                            "align_nodes: page width/height cannot be resolved to px".to_owned(),
                            None,
                            None,
                        ));
                        return;
                    }
                }
            }
        }
    } else if anchor == "selection" {
        // union bbox of all alignable nodes.
        let ref_left = alignable.iter().map(|n| n.x).fold(f64::INFINITY, f64::min);
        let ref_right = alignable
            .iter()
            .map(|n| n.x + n.w)
            .fold(f64::NEG_INFINITY, f64::max);
        let ref_top = alignable.iter().map(|n| n.y).fold(f64::INFINITY, f64::min);
        let ref_bottom = alignable
            .iter()
            .map(|n| n.y + n.h)
            .fold(f64::NEG_INFINITY, f64::max);
        (ref_left, ref_right, ref_top, ref_bottom)
    } else {
        // anchor is a NODE ID: align relative to that node's bbox.
        let found: Option<Option<(f64, f64, f64, f64)>> = 'anchor_scan: {
            for page in doc.body.pages.iter() {
                if let Some(node) = find_node_shared(&page.children, anchor) {
                    break 'anchor_scan Some(read_geometry_px(node));
                }
            }
            None
        };
        match found {
            Some(Some((x, y, w, h))) => (x, x + w, y, y + h),
            Some(None) => {
                diagnostics.push(Diagnostic::error(
                    "tx.unsupported_property",
                    format!(
                        "align_nodes: anchor node {:?} has no resolvable x/y/w/h geometry",
                        anchor
                    ),
                    None,
                    Some(anchor.to_owned()),
                ));
                return;
            }
            None => {
                diagnostics.push(Diagnostic::error(
                    "tx.unknown_node",
                    format!(
                        "align_nodes: anchor {:?} is not \"page\", \"selection\", or a known node id",
                        anchor
                    ),
                    None,
                    Some(anchor.to_owned()),
                ));
                return;
            }
        }
    };

    // ── Phase 2: exclusive borrow — write new x or y for each node ───────────
    //
    // Compute the new position per node from the captured bbox, then apply via
    // find_node_any_mut + node_geometry_mut, mirroring apply_set_geometry's
    // write path.

    for bbox in &alignable {
        let new_x = match align {
            "left" => Some(ref_left),
            "hcenter" => Some((ref_left + ref_right) / 2.0 - bbox.w / 2.0),
            "right" => Some(ref_right - bbox.w),
            _ => None,
        };
        let new_y = match align {
            "top" => Some(ref_top),
            "vcenter" => Some((ref_top + ref_bottom) / 2.0 - bbox.h / 2.0),
            "bottom" => Some(ref_bottom - bbox.h),
            _ => None,
        };

        // At least one of new_x/new_y is Some (align was validated above).
        match find_node_any_mut(doc, &bbox.id) {
            None => {
                // Should not happen: we found it in phase 1, but guard anyway.
                diagnostics.push(Diagnostic::error(
                    "tx.unknown_node",
                    format!("align_nodes: node {:?} disappeared between phases", bbox.id),
                    None,
                    Some(bbox.id.clone()),
                ));
            }
            Some(node) => {
                // node_geometry_mut is guaranteed Some here: we filtered on
                // read_geometry_px which uses the same set of node kinds.
                if let Some((nx, ny, _, _)) = node_geometry_mut(node) {
                    if let Some(v) = new_x {
                        *nx = Some(px(v));
                    }
                    if let Some(v) = new_y {
                        *ny = Some(px(v));
                    }
                    record_affected(&bbox.id, affected);
                }
            }
        }
    }
}
