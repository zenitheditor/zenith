//! `align_nodes` and `align_to_edge` application. Both compute in a shared
//! space (page space, or the space of a shared ancestor container) and
//! write each node back in its parent's space.

use zenith_core::{Diagnostic, Document, dim_to_px};

use super::super::layout::reject_layout_managed;
use super::super::record_affected;
use super::super::space::resolved_tokens;
use super::super::structure::parse_dimension_str;
use super::boxes::{
    Placed, SpaceBox, page_bounds_for_node, placed, shared_links, space_box, write_space_xy,
};

/// Valid alignment directions for `Op::AlignNodes`.
const VALID_ALIGN_DIRS: &[&str] = &["left", "hcenter", "right", "top", "vcenter", "bottom"];

/// Parse an explicit dimension string of the canonical `"(unit)value"` form
/// (e.g. `"(px)120"`, `"(pt)90"`) into a px magnitude.
///
/// Delegates parsing to [`parse_dimension_str`] (the single canonical
/// `"(unit)value"` parser) then resolves to px via [`dim_to_px`], so the
/// arithmetic matches the rest of the engine. Returns `None` if the string is
/// not parenthesized-unit-prefixed, the numeric tail is not a finite number, or
/// the unit does not resolve to px (e.g. `pct`, `deg`).
fn parse_px_dimension(s: &str) -> Option<f64> {
    let dim = parse_dimension_str(s)?;
    dim_to_px(dim.value, &dim.unit)
}

pub(in crate::engine) fn apply_align_nodes(
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
    if reject_layout_managed(
        doc,
        node_ids.iter().map(String::as_str),
        "align_nodes",
        diagnostics,
    ) {
        return;
    }

    // `anchor` is "page", "selection", an explicit dimension like "(px)120",
    // or a node id (align relative to that node's bbox). The dimension and
    // node-id forms are resolved when the reference rectangle is computed below;
    // an unparseable dimension or unknown id is rejected there.
    //
    // An explicit-dimension anchor names a single absolute coordinate on the
    // active axis. We detect it eagerly (a leading '(') so a node whose id
    // happened to start with '(' cannot shadow it.
    let dimension_anchor: Option<f64> = if anchor.starts_with('(') {
        match parse_px_dimension(anchor) {
            Some(v) => Some(v),
            None => {
                diagnostics.push(Diagnostic::error(
                    "tx.invalid_value",
                    format!(
                        "align_nodes: anchor {:?} is not a resolvable dimension (expected e.g. \"(px)120\")",
                        anchor
                    ),
                    None,
                    None,
                ));
                return;
            }
        }
    } else {
        None
    };

    // ── Phase 1: shared scan — gather boxes in one shared space ───────────────
    //
    // A page or dimension anchor works in page space. A selection or node
    // anchor works in the space of the deepest container every box shares,
    // so siblings inside an unresolved container still align.
    //
    // A missing node is an error. A node without a resolvable box or parent
    // origin is skipped with a warning, and the rest are still aligned.
    let resolved = resolved_tokens(doc);
    let mut found: Vec<Placed> = Vec::new();
    for node_id in node_ids {
        match placed(doc, node_id, &resolved) {
            Some(p) => found.push(p),
            None => diagnostics.push(Diagnostic::error(
                "tx.unknown_node",
                format!("align_nodes: node {:?} not found in document", node_id),
                None,
                Some(node_id.clone()),
            )),
        }
    }
    let page_space = dimension_anchor.is_some() || anchor == "page";
    let anchor_node = if page_space || anchor == "selection" {
        None
    } else {
        match placed(doc, anchor, &resolved) {
            Some(p) => Some(p),
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
    let skip = if page_space {
        0
    } else {
        shared_links(found.iter().chain(anchor_node.as_ref()))
    };

    let mut alignable: Vec<SpaceBox> = Vec::new();
    for p in &found {
        match space_box(p, skip) {
            Ok(b) => alignable.push(b),
            Err(e) => {
                // Warning, so the caller sees AcceptedWithWarnings and knows a
                // node was skipped.
                diagnostics.push(Diagnostic::warning(
                    "tx.geometry_unresolved",
                    format!("align_nodes: node {:?} {}; skipped", p.id, e.reason()),
                    None,
                    Some(p.id.clone()),
                ));
            }
        }
    }

    // Need at least one alignable node to proceed.
    let Some(first_id) = alignable.first().map(|b| b.id.clone()) else {
        diagnostics.push(Diagnostic::advisory(
            "tx.noop",
            "align_nodes: no alignable nodes with resolvable geometry; document is unchanged"
                .to_owned(),
            None,
            None,
        ));
        return;
    };

    // ── Compute the reference rectangle ───────────────────────────────────────

    let (ref_left, ref_right, ref_top, ref_bottom) = if let Some(coord) = dimension_anchor {
        // Explicit-dimension anchor: `coord` is the page-space target coordinate
        // on the active axis. Collapse the reference rectangle to that single
        // coordinate on every edge. Only the active axis (selected by `align`)
        // is ever read, so the inactive-axis edges are harmless.
        (coord, coord, coord, coord)
    } else if anchor == "page" {
        // Find the page that contains the first alignable node, or a page that
        // references the master hosting it (master chrome is authored in page coords).
        match page_bounds_for_node(doc, &first_id) {
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
            Some((w, h)) => (0.0_f64, w, 0.0_f64, h),
        }
    } else if let Some(anchor_node) = &anchor_node {
        // anchor is a NODE ID: align relative to that node's bbox.
        match space_box(anchor_node, skip) {
            Ok(b) => (b.x, b.x + b.w, b.y, b.y + b.h),
            Err(e) => {
                diagnostics.push(Diagnostic::error(
                    "tx.unsupported_property",
                    format!("align_nodes: anchor node {:?} {}", anchor, e.reason()),
                    None,
                    Some(anchor.to_owned()),
                ));
                return;
            }
        }
    } else {
        // "selection": union bbox of all alignable nodes.
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
    };

    // ── Phase 2: exclusive borrow — write new x or y for each node ───────────
    //
    // Compute the new shared-space position per node, then write it back in
    // the node's parent space (covers PropertyValue and Instance slots).

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
        match write_space_xy(doc, bbox, new_x, new_y) {
            None => {
                // Should not happen: we found it in phase 1, but guard anyway.
                diagnostics.push(Diagnostic::error(
                    "tx.unknown_node",
                    format!("align_nodes: node {:?} disappeared between phases", bbox.id),
                    None,
                    Some(bbox.id.clone()),
                ));
            }
            Some(true) => record_affected(&bbox.id, affected),
            Some(false) => {}
        }
    }
}

// ── AlignToEdge ───────────────────────────────────────────────────────────────

/// Valid edge values for `Op::AlignToEdge`.
const VALID_EDGES: &[&str] = &["left", "right", "top", "bottom", "hcenter", "vcenter"];

/// Snap a single node's edge (or centre) to the boundary of the page that
/// contains it, with an optional margin inset.
///
/// Horizontal edges (`left`, `right`, `hcenter`) set x; vertical edges
/// (`top`, `bottom`, `vcenter`) set y. The opposite coordinate is untouched.
pub(in crate::engine) fn apply_align_to_edge(
    node_id: &str,
    edge: &str,
    margin: f64,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    // Validate edge value early, before touching the document.
    if !VALID_EDGES.contains(&edge) {
        diagnostics.push(Diagnostic::error(
            "tx.unsupported_property",
            format!(
                "align_to_edge: edge {:?} must be one of left,right,top,bottom,hcenter,vcenter",
                edge
            ),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    }
    if reject_layout_managed(doc, [node_id], "align_to_edge", diagnostics) {
        return;
    }

    // ── Phase 1 (shared scan): read the node's geometry and find its page ────

    // Read the node's page-space box (pages + masters).
    let resolved = resolved_tokens(doc);
    let node_box = match placed(doc, node_id, &resolved).map(|p| space_box(&p, 0)) {
        None => {
            diagnostics.push(Diagnostic::error(
                "tx.unknown_node",
                format!("align_to_edge: node {:?} not found in document", node_id),
                None,
                Some(node_id.to_owned()),
            ));
            return;
        }
        Some(Err(e)) => {
            diagnostics.push(Diagnostic::error(
                "tx.unsupported_property",
                format!("align_to_edge: node {:?} {}", node_id, e.reason()),
                None,
                Some(node_id.to_owned()),
            ));
            return;
        }
        Some(Ok(b)) => b,
    };
    let (node_w, node_h) = (node_box.w, node_box.h);

    let (page_w, page_h) = match page_bounds_for_node(doc, node_id) {
        Some(bounds) => bounds,
        None => {
            diagnostics.push(Diagnostic::error(
                "tx.unknown_node",
                format!(
                    "align_to_edge: could not locate page containing node {:?}",
                    node_id
                ),
                None,
                Some(node_id.to_owned()),
            ));
            return;
        }
    };

    // ── Compute new coordinate(s) from shared data ─────────────────────────

    let new_x: Option<f64> = match edge {
        "left" => Some(margin),
        "right" => Some(page_w - node_w - margin),
        "hcenter" => Some((page_w - node_w) / 2.0),
        _ => None,
    };
    let new_y: Option<f64> = match edge {
        "top" => Some(margin),
        "bottom" => Some(page_h - node_h - margin),
        "vcenter" => Some((page_h - node_h) / 2.0),
        _ => None,
    };

    // ── Phase 2 (exclusive borrow): write the new coordinate ─────────────────

    match write_space_xy(doc, &node_box, new_x, new_y) {
        None => {
            // Should not happen: we found it in phase 1.
            diagnostics.push(Diagnostic::error(
                "tx.unknown_node",
                format!(
                    "align_to_edge: node {:?} disappeared between phases",
                    node_id
                ),
                None,
                Some(node_id.to_owned()),
            ));
        }
        Some(true) => record_affected(node_id, affected),
        Some(false) => {}
    }
}
