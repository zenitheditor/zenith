//! `tx.geometry_required`: a `set_geometry` `null` that removes `x`, `y`,
//! `w`, or `h` from a node that then has no position or no size.
//!
//! The rule mirrors core validation. A row/column/grid frame that places the
//! node in flow supplies its x/y and size. An anchor supplies x/y. A layout
//! frame hugs its children, so it needs no w/h. A group and the kinds that
//! carry no required box (instance, field, toc, mesh) need nothing. Post-apply
//! validation still runs, so this check only adds a precise message.

use zenith_core::{Diagnostic, Document, LayoutKind, Node};

use super::super::find_node_any_shared;
use super::super::layout::in_layout_flow;

/// Which `set_geometry` box fields are `null` (removed).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Removals {
    pub x: bool,
    pub y: bool,
    pub w: bool,
    pub h: bool,
}

impl Removals {
    fn any(self) -> bool {
        self.x || self.y || self.w || self.h
    }
}

/// What a node needs when no layout frame places it.
struct Needs {
    /// `x` / `y` are required (unless an anchor places the node).
    position: bool,
    /// `w` / `h` are required.
    size: bool,
}

/// Push one `tx.geometry_required` Error per removed field that `node_id`
/// needs, and return `true` when any was pushed. An unknown node passes:
/// the caller reports `tx.unknown_node`.
pub(super) fn reject_required_removals(
    doc: &Document,
    node_id: &str,
    removals: Removals,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if !removals.any() || in_layout_flow(doc, node_id) {
        return false;
    }
    let Some(node) = find_node_any_shared(doc, node_id) else {
        return false;
    };
    let needs = needs(node);
    let anchored = anchored(node);
    let kind = node.kind_str();
    let position_reason = "no row/column/grid frame places it in flow and no anchor places it";
    let size_reason = "no row/column/grid frame sizes it in flow";
    let mut rejected = false;
    for (field, removed, required, reason) in [
        (
            "x",
            removals.x,
            needs.position && !anchored,
            position_reason,
        ),
        (
            "y",
            removals.y,
            needs.position && !anchored,
            position_reason,
        ),
        ("w", removals.w, needs.size, size_reason),
        ("h", removals.h, needs.size, size_reason),
    ] {
        if !(removed && required) {
            continue;
        }
        rejected = true;
        diagnostics.push(Diagnostic::error(
            "tx.geometry_required",
            format!(
                "set_geometry: {field}=null on {kind} {node_id:?} removes a required \
                 attribute: {reason}. Keep {field}, or put the node in the flow of a \
                 row/column/grid frame (set_layout position=null clears \
                 position=\"absolute\")."
            ),
            None,
            Some(node_id.to_owned()),
        ));
    }
    rejected
}

/// After `op` edited node `node_id`: push one `tx.geometry_required` Error
/// per `x` / `y` the node needs but lacks, and return `true` when any was
/// pushed. A node in flow, an anchored node, and an unknown node pass.
pub(in crate::engine) fn reject_unplaced(
    doc: &Document,
    node_id: &str,
    op: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if in_layout_flow(doc, node_id) {
        return false;
    }
    let Some(node) = find_node_any_shared(doc, node_id) else {
        return false;
    };
    if !needs(node).position || anchored(node) {
        return false;
    }
    let Some(view) = node.anchor_view() else {
        return false;
    };
    let kind = node.kind_str();
    let mut rejected = false;
    for (field, value) in [("x", view.x), ("y", view.y)] {
        if value.is_some() {
            continue;
        }
        rejected = true;
        diagnostics.push(Diagnostic::error(
            "tx.geometry_required",
            format!(
                "{op}: {kind} {node_id:?} has no {field} and no anchor places it after this \
                 edit. Run detach_anchor to remove the anchor and keep the position, or set \
                 {field} with set_geometry first."
            ),
            None,
            Some(node_id.to_owned()),
        ));
    }
    rejected
}

/// The box attributes core validation requires of `node` outside flow.
fn needs(node: &Node) -> Needs {
    let both = |required: bool| Needs {
        position: required,
        size: required,
    };
    match node {
        Node::Rect(_)
        | Node::Ellipse(_)
        | Node::Text(_)
        | Node::Code(_)
        | Node::Image(_)
        | Node::Table(_)
        | Node::Shape(_)
        | Node::Pattern(_)
        | Node::Chart(_) => both(true),
        // A layout frame without w/h hugs its children.
        Node::Frame(f) => Needs {
            position: true,
            size: !f
                .layout
                .as_ref()
                .is_some_and(LayoutKind::positions_children),
        },
        Node::Group(_)
        | Node::Instance(_)
        | Node::Field(_)
        | Node::Toc(_)
        | Node::Mesh(_)
        | Node::Light(_)
        | Node::Line(_)
        | Node::Polygon(_)
        | Node::Polyline(_)
        | Node::Path(_)
        | Node::Footnote(_)
        | Node::Connector(_)
        | Node::Unknown(_) => both(false),
    }
}

/// `true` when an anchor supplies the node's x/y: an `anchor` value, or an
/// `anchor-sibling` with an `anchor-edge`.
pub(in crate::engine) fn anchored(node: &Node) -> bool {
    node.anchor_view().is_some_and(|v| {
        v.anchor.is_some() || (v.anchor_sibling.is_some() && v.anchor_edge.is_some())
    })
}
