//! Lock enforcement: which node ids an op targets, and whether a node is locked.

use zenith_core::{Document, Node};

use super::find_node_shared;
use crate::op::Op;

/// Return the node id(s) a *mutating* op would edit, for the lock-guarded ops
/// only. Exempt ops return an empty `Vec`.
///
/// Guarded (return target id(s)): the property/geometry/text setters, removal,
/// the z-order reorders, `reparent` (its `node`), and `align_nodes` (every id,
/// in source order).
///
/// Exempt (empty): `set_locked` (must be able to *unlock* a locked node),
/// `set_visible` (visibility is a view toggle), `add_node`, `add_path`,
/// `duplicate_node` (the source is read-only), `group`, and `ungroup`.
pub(super) fn op_lock_targets(op: &Op) -> Vec<&str> {
    match op {
        Op::SetTextAlign { node, .. }
        | Op::SetFill { node, .. }
        | Op::SetFillRule { node, .. }
        | Op::SetStroke { node, .. }
        | Op::SetStrokeWidth { node, .. }
        | Op::SetGeometry { node, .. }
        | Op::SetPoints { node, .. }
        | Op::SetPathAnchors { node, .. }
        | Op::SetPathAnchorKind { node, .. }
        | Op::RemovePathAnchor { node, .. }
        | Op::InsertPathAnchor { node, .. }
        | Op::InsertPathAnchorAtPoint { node, .. }
        | Op::MovePathAnchor { node, .. }
        | Op::MovePathHandle { node, .. }
        | Op::SimplifyPathAnchors { node, .. }
        | Op::TransformPathAnchors { node, .. }
        | Op::SnapPathAnchors { node, .. }
        | Op::SetOpacity { node, .. }
        | Op::ReplaceText { node, .. }
        | Op::RemoveNode { node }
        | Op::MoveForward { node }
        | Op::MoveBackward { node }
        | Op::MoveToFront { node }
        | Op::MoveToBack { node }
        | Op::Reparent { node, .. }
        | Op::SetTextOverflow { node_id: node, .. }
        | Op::SetTextDirection { node, .. }
        | Op::AlignToEdge { node, .. }
        | Op::DetachPattern { node } => vec![node.as_str()],
        // Doc-wide mode returns empty (lock handling is inside apply_find_replace_text).
        // Scoped mode: guard the named node.
        Op::FindReplaceText { node, .. } => {
            node.as_deref().map(|n| vec![n]).unwrap_or_default()
        }
        Op::AlignNodes { node_ids, .. } | Op::DistributeNodes { node_ids, .. } => {
            node_ids.iter().map(String::as_str).collect()
        }
        Op::SetAsset { node_id, .. } => vec![node_id.as_str()],
        Op::SetLayout(edit) => vec![edit.node.as_str()],
        Op::SetLocked { .. }
        | Op::SetVisible { .. }
        | Op::AddNode { .. }
        | Op::AddPath { .. }
        | Op::DuplicateNode { .. }
        | Op::MakePathSymmetric { .. }
        | Op::PathBoolean { .. }
        | Op::DuplicatePage { .. }
        | Op::Group { .. }
        | Op::Ungroup { .. }
        // Page-structure ops act on `Page` structs, which have no `locked`
        // dimension (locking is a per-`Node` property). There is no node-level
        // lock target to enforce here, so these are exempt (empty).
        | Op::AddPage { .. }
        | Op::DeletePage { .. }
        | Op::ReorderPages { .. }
        | Op::SetPageSize { .. }
        // AddAsset creates new content and never mutates a node; exempt like AddNode.
        | Op::AddAsset { .. }
        // Token ops mutate the token block, not the node tree; no per-node lock target.
        | Op::CreateToken { .. }
        | Op::UpdateTokenValue { .. }
        // Style ops mutate the styles block, not the node tree; no per-node lock target.
        | Op::SetStyleProperty { .. }
        | Op::CreateStyle { .. }
        | Op::DeleteStyle { .. }
        // Master ops mutate the masters block / page attributes, not locked nodes.
        | Op::CreateMaster { .. }
        | Op::DeleteMaster { .. }
        | Op::SetPageMaster { .. }
        // Recipe ops mutate the recipes block, not the node tree; no per-node lock target.
        | Op::CreateRecipe { .. }
        | Op::UpdateRecipe { .. }
        | Op::DeleteRecipe { .. }
        // Defaults ops mutate `defaults` blocks, not the node tree; no per-node lock target.
        | Op::SetDefault { .. }
        | Op::RemoveDefault { .. } => Vec::new(),
    }
}

/// Return `true` if the node with `id` exists and has `locked == Some(true)`.
///
/// Missing nodes and nodes with `locked` absent/`Some(false)` return `false`;
/// the missing-node case is left for the op's own `tx.unknown_node` path.
/// Mirrors the variant coverage of [`node_locked_mut`] via a shared scan.
pub(super) fn node_is_locked(doc: &Document, id: &str) -> bool {
    fn locked_of(node: &Node) -> Option<bool> {
        match node {
            Node::Rect(n) => n.locked,
            Node::Ellipse(n) => n.locked,
            Node::Line(n) => n.locked,
            Node::Text(n) => n.locked,
            Node::Code(n) => n.locked,
            Node::Frame(n) => n.locked,
            Node::Group(n) => n.locked,
            Node::Image(n) => n.locked,
            Node::Polygon(n) => n.locked,
            Node::Polyline(n) => n.locked,
            Node::Path(n) => n.locked,
            Node::Instance(n) => n.locked,
            Node::Field(n) => n.locked,
            Node::Toc(n) => n.locked,
            Node::Table(n) => n.locked,
            Node::Shape(n) => n.locked,
            Node::Connector(n) => n.locked,
            Node::Pattern(n) => n.locked,
            Node::Chart(n) => n.locked,
            Node::Light(n) => n.locked,
            Node::Mesh(n) => n.locked,
            // A footnote has no `locked` field; treat as unlocked.
            Node::Footnote(_) => None,
            Node::Unknown(_) => None,
        }
    }

    doc.body
        .pages
        .iter()
        .find_map(|page| find_node_shared(&page.children, id))
        .or_else(|| {
            doc.masters
                .iter()
                .find_map(|master| find_node_shared(&master.children, id))
        })
        .and_then(locked_of)
        == Some(true)
}
