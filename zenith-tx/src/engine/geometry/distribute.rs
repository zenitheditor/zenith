//! `distribute_nodes` application. Computes in the space of the deepest
//! container the boxes share and writes each node back in its parent's space.

use zenith_core::{Diagnostic, Document};

use super::super::layout::reject_layout_managed;
use super::super::record_affected;
use super::super::space::resolved_tokens;
use super::boxes::{Placed, SpaceBox, placed, shared_links, space_box, write_space_xy};

/// Valid axes for `Op::DistributeNodes`.
const VALID_DISTRIBUTE_AXES: &[&str] = &["horizontal", "vertical"];

/// A node's captured bbox during distribution, reduced to the active axis.
struct AxisBox {
    /// Leading-edge coordinate on the active axis (`x` for horizontal, `y` for vertical).
    pos: f64,
    /// Extent on the active axis (`w` for horizontal, `h` for vertical).
    size: f64,
    /// The shared-space box, for the write back.
    space: SpaceBox,
}

pub(in crate::engine) fn apply_distribute_nodes(
    node_ids: &[String],
    axis: &str,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    // Validate axis value before touching the tree.
    if !VALID_DISTRIBUTE_AXES.contains(&axis) {
        diagnostics.push(Diagnostic::error(
            "tx.unsupported_property",
            format!("distribute_nodes: unknown axis {:?}", axis),
            None,
            None,
        ));
        return;
    }
    if reject_layout_managed(
        doc,
        node_ids.iter().map(String::as_str),
        "distribute_nodes",
        diagnostics,
    ) {
        return;
    }
    let horizontal = axis == "horizontal";

    // ── Phase 1: shared scan — gather active-axis geometry and check existence ──
    //
    // Mirrors apply_align_nodes' phase 1: a missing node is a hard error, a node
    // found without resolvable geometry is skipped with a warning, and the rest
    // are still distributed.
    // Boxes compare in the space of the deepest container they all share.
    let resolved = resolved_tokens(doc);
    let mut found: Vec<Placed> = Vec::new();
    for node_id in node_ids {
        match placed(doc, node_id, &resolved) {
            Some(p) => found.push(p),
            None => diagnostics.push(Diagnostic::error(
                "tx.unknown_node",
                format!("distribute_nodes: node {:?} not found in document", node_id),
                None,
                Some(node_id.clone()),
            )),
        }
    }
    let skip = shared_links(&found);

    let mut boxes: Vec<AxisBox> = Vec::new();
    for p in &found {
        match space_box(p, skip) {
            Ok(b) => {
                let (pos, size) = if horizontal { (b.x, b.w) } else { (b.y, b.h) };
                boxes.push(AxisBox {
                    pos,
                    size,
                    space: b,
                });
            }
            Err(e) => {
                diagnostics.push(Diagnostic::warning(
                    "tx.geometry_unresolved",
                    format!("distribute_nodes: node {:?} {}; skipped", p.id, e.reason()),
                    None,
                    Some(p.id.clone()),
                ));
            }
        }
    }

    // Distribute-spacing needs ≥ 3 nodes (two fixed endpoints + ≥ 1 interior).
    // Fewer is a no-op, mirroring align_nodes' degenerate-input convention.
    if boxes.len() < 3 {
        diagnostics.push(Diagnostic::advisory(
            "tx.noop",
            format!(
                "distribute_nodes: needs at least 3 alignable nodes but found {}; document is unchanged",
                boxes.len()
            ),
            None,
            None,
        ));
        return;
    }

    // Order by current leading-edge position on the active axis. Ties keep their
    // relative input order (stable sort) for determinism.
    boxes.sort_by(|a, b| a.pos.total_cmp(&b.pos));

    // Endpoints are fixed. Span = last trailing edge − first leading edge.
    // Equal gap = (span − Σ sizes) / (n − 1).
    let (Some(first), Some(last)) = (boxes.first(), boxes.last()) else {
        // Unreachable: len ≥ 3 was checked above.
        return;
    };
    let span = (last.pos + last.size) - first.pos;
    let total_size: f64 = boxes.iter().map(|b| b.size).sum();
    let gap = (span - total_size) / ((boxes.len() - 1) as f64);

    // Walk left-to-right, placing each interior node after the previous one plus
    // the equal gap. Endpoints keep their positions. Collect new positions first,
    // then apply with an exclusive borrow per node (mirrors align's phase 2).
    let mut new_positions: Vec<(&SpaceBox, f64)> = Vec::with_capacity(boxes.len());
    let mut cursor = first.pos;
    for (i, b) in boxes.iter().enumerate() {
        let new_pos = if i == 0 { first.pos } else { cursor + gap };
        new_positions.push((&b.space, new_pos));
        cursor = new_pos + b.size;
    }

    // ── Phase 2: exclusive borrow — write the new active-axis position ──────────
    for (space, new_pos) in new_positions {
        let (new_x, new_y) = if horizontal {
            (Some(new_pos), None)
        } else {
            (None, Some(new_pos))
        };
        match write_space_xy(doc, space, new_x, new_y) {
            None => {
                diagnostics.push(Diagnostic::error(
                    "tx.unknown_node",
                    format!(
                        "distribute_nodes: node {:?} disappeared between phases",
                        space.id
                    ),
                    None,
                    Some(space.id.clone()),
                ));
            }
            Some(true) => record_affected(&space.id, affected),
            Some(false) => {}
        }
    }
}
