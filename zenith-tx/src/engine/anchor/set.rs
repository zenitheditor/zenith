//! `set_anchor` application: check every field, then write.

use zenith_core::{Diagnostic, Dimension, Document, Unit, parse_anchor, parse_anchor_edge};

use super::super::geometry::reject_unplaced;
use super::super::layout::reject_layout_managed;
use super::super::structure::parse_dimension_str;
use super::super::{find_node_any_mut, px, record_affected};
use super::fields::anchor_fields_mut;
use crate::op::{AnchorEdit, LayoutDim};

const OP: &str = "set_anchor";

const ANCHORS: &str = "top-left, top-center, top-right, center-left, center, center-right, \
                       bottom-left, bottom-center, or bottom-right";

/// The checked values of one edit, in the same tri-state shape.
struct Checked {
    gap: Option<Option<Dimension>>,
}

pub(in crate::engine) fn apply_set_anchor(
    edit: &AnchorEdit,
    doc: &mut Document,
    diagnostics: &mut Vec<Diagnostic>,
    affected: &mut Vec<String>,
) {
    let node_id = edit.node.as_str();
    if edit.is_empty() {
        diagnostics.push(Diagnostic::advisory(
            "tx.noop",
            format!("{OP} on {node_id:?} specified no fields. The document is unchanged."),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    }
    let Some(checked) = check(edit, diagnostics) else {
        return;
    };
    if edit.writes_value() && reject_layout_managed(doc, [node_id], OP, diagnostics) {
        return;
    }
    let Some(node) = find_node_any_mut(doc, node_id) else {
        diagnostics.push(Diagnostic::error(
            "tx.unknown_node",
            format!("node {node_id:?} not found in document"),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    };
    let kind = node.kind_str();
    let Some(fields) = anchor_fields_mut(node) else {
        diagnostics.push(Diagnostic::error(
            "tx.unsupported_property",
            format!("{OP} is not supported on {kind} {node_id:?}: this kind takes no anchor"),
            None,
            Some(node_id.to_owned()),
        ));
        return;
    };
    if let Some(v) = &edit.anchor {
        *fields.anchor = v.clone();
    }
    if let Some(v) = &edit.anchor_zone {
        *fields.zone = v.clone();
    }
    if let Some(v) = &edit.anchor_sibling {
        *fields.sibling = v.clone();
    }
    if let Some(v) = edit.anchor_parent {
        *fields.parent = v;
    }
    if let Some(v) = &edit.anchor_edge {
        *fields.edge = v.clone();
    }
    if let Some(v) = checked.gap {
        *fields.gap = v;
    }
    if reject_unplaced(doc, node_id, OP, diagnostics) {
        return;
    }
    record_affected(node_id, affected);
}

/// Check every field. `None` after pushing at least one Error.
fn check(edit: &AnchorEdit, diagnostics: &mut Vec<Diagnostic>) -> Option<Checked> {
    let node_id = edit.node.as_str();
    let before = diagnostics.len();
    let mut invalid = |field: &str, tail: String| {
        diagnostics.push(Diagnostic::error(
            "tx.invalid_value",
            format!("{OP}: {field} on node {node_id:?} {tail}"),
            None,
            Some(node_id.to_owned()),
        ));
    };
    if let Some(Some(a)) = &edit.anchor
        && parse_anchor(a).is_none()
    {
        invalid("anchor", format!("is {a:?}. Pass {ANCHORS}, or null."));
    }
    if let Some(Some(e)) = &edit.anchor_edge
        && parse_anchor_edge(e).is_none()
    {
        invalid(
            "anchor_edge",
            format!("is {e:?}. Pass above, below, before, after, or null."),
        );
    }
    if let Some(Some(s)) = &edit.anchor_sibling {
        if s == node_id {
            invalid(
                "anchor_sibling",
                "names the node itself. A node anchors to a sibling.".to_owned(),
            );
        } else if s.trim().is_empty() {
            invalid(
                "anchor_sibling",
                "is empty. Pass a sibling id, or null.".to_owned(),
            );
        }
    }
    if let Some(Some(z)) = &edit.anchor_zone
        && z.trim().is_empty()
    {
        invalid(
            "anchor_zone",
            "is empty. Pass a safe-zone id, or null.".to_owned(),
        );
    }
    let gap = match &edit.anchor_gap {
        None => None,
        Some(None) => Some(None),
        Some(Some(input)) => match gap_value(input) {
            Some(d) => Some(Some(d)),
            None => {
                let shown = match input {
                    LayoutDim::Px(v) => v.to_string(),
                    LayoutDim::Text(s) => format!("{s:?}"),
                };
                invalid(
                    "anchor_gap",
                    format!(
                        "is {shown}. Pass a finite px number, a \"(px)N\" or \"(pt)N\" string, or null."
                    ),
                );
                None
            }
        },
    };
    (diagnostics.len() == before).then_some(Checked { gap })
}

/// The stored `anchor-gap` of an input: a finite px number, or a px / pt
/// dimension string.
fn gap_value(input: &LayoutDim) -> Option<Dimension> {
    match input {
        LayoutDim::Px(v) if v.is_finite() => Some(px(*v)),
        LayoutDim::Px(_) => None,
        LayoutDim::Text(s) => {
            let d = parse_dimension_str(s)?;
            match d.unit {
                Unit::Px | Unit::Pt => Some(d),
                Unit::Pct | Unit::Deg | Unit::Unknown(_) => None,
            }
        }
    }
}
