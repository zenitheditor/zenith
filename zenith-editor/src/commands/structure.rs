//! Structure commands: `node.remove`, `node.duplicate`, `node.reorder`,
//! `node.group`, and `node.ungroup`.

use std::collections::BTreeSet;

use serde::Deserialize;
use serde_json::Value;
use zenith_core::Node;
use zenith_tx::{Op, Permissions};

use super::common::{params, target, targets};
use crate::ctx::Ctx;
use crate::doc::tree::{all_ids, child_lists, locate, unique_id};
use crate::edit::ops::{OpsEdit, apply_ops};
use crate::error::EditorError;

/// Run `ops` as an edit with `selection` after it.
fn edit(
    ctx: &mut Ctx<'_, '_>,
    doc: &zenith_core::Document,
    ops: Vec<Op>,
    selection: Vec<String>,
    raw: Value,
) -> Result<Value, EditorError> {
    apply_ops(
        ctx,
        doc,
        OpsEdit {
            label: None,
            ops,
            permissions: Permissions::default(),
            selection: Some(selection),
            notes: Vec::new(),
            gesture: false,
            raw,
        },
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoveParams {
    #[serde(default)]
    ids: Option<Vec<String>>,
}

/// Remove `ids` (default: the selection) with their subtrees. The comment
/// lines directly above a removed node go with it; the reply lists them in
/// `removed_comments`. The selection keeps the ids that remain.
pub(crate) fn remove(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: RemoveParams = params(ctx, raw.clone())?;
    let ids = targets(ctx, p.ids)?;
    let doc = ctx.editable()?;
    let ops = ids
        .iter()
        .map(|id| Op::RemoveNode { node: id.clone() })
        .collect();
    let selection = ctx
        .session
        .selection
        .iter()
        .filter(|s| !ids.contains(s))
        .cloned()
        .collect();
    edit(ctx, &doc, ops, selection, raw)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DuplicateParams {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    ids: Option<Vec<String>>,
    #[serde(default)]
    new_id: Option<String>,
    #[serde(default)]
    dx: f64,
    #[serde(default)]
    dy: f64,
}

/// Duplicate nodes (any kind, containers with their subtree), each right
/// after itself, in one transaction, and select the copies.
///
/// The nodes are `id`, else `ids`, else the selection. A copy's id is the
/// first free of `<id>-copy`, `<id>-copy2`, … (or `new_id`, for one node);
/// descendant ids take the same suffix. A node inside another duplicated
/// node is copied with it, not again. A non-zero `dx` / `dy` (authored px)
/// moves each box-kind copy with `nudge_geometry` in the same transaction.
pub(crate) fn duplicate(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: DuplicateParams = params(ctx, raw.clone())?;
    let ids = match (p.id, p.ids) {
        (Some(_), Some(_)) => {
            return Err(EditorError::new(
                "editor.invalid_params",
                "'node.duplicate' takes id or ids, not both",
            ));
        }
        (Some(id), None) => vec![id],
        (None, ids) => targets(ctx, ids)?,
    };
    if p.new_id.is_some() && ids.len() > 1 {
        return Err(EditorError::new(
            "editor.invalid_params",
            "new_id names the copy of one node; leave it out to duplicate several",
        ));
    }
    let doc = ctx.editable()?;
    let mut taken: BTreeSet<String> = all_ids(&doc).into_iter().map(str::to_owned).collect();
    let mut ops: Vec<Op> = Vec::new();
    let mut copies: Vec<String> = Vec::new();
    for id in &ids {
        let located = locate(&doc, id).ok_or_else(|| EditorError::unknown_node(id))?;
        let inside = located
            .ancestors
            .iter()
            .filter_map(|a| a.id())
            .any(|a| ids.iter().any(|i| i == a));
        if inside {
            continue;
        }
        let new_id = match &p.new_id {
            Some(new_id) => new_id.clone(),
            None => {
                let refs: BTreeSet<&str> = taken.iter().map(String::as_str).collect();
                unique_id(&format!("{id}-copy"), &refs)
            }
        };
        taken.insert(new_id.clone());
        ops.push(Op::DuplicateNode {
            node: id.clone(),
            new_id: new_id.clone(),
        });
        if p.dx != 0.0 || p.dy != 0.0 {
            if located.node.box_view().is_none() && !matches!(located.node, Node::Instance(_)) {
                return Err(EditorError::new(
                    "editor.unsupported",
                    format!(
                        "{} '{id}' has no box, so dx/dy cannot offset its copy; duplicate \
                         without an offset and move the copy with gesture.commit",
                        located.node.kind_str()
                    ),
                ));
            }
            ops.push(Op::NudgeGeometry {
                node: new_id.clone(),
                dx: (p.dx != 0.0).then_some(p.dx),
                dy: (p.dy != 0.0).then_some(p.dy),
                dw: None,
                dh: None,
                detach: false,
            });
        }
        copies.push(new_id);
    }
    edit(ctx, &doc, ops, copies, raw)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReorderParams {
    #[serde(default)]
    id: Option<String>,
    to: ReorderTo,
}

/// Where `node.reorder` moves a node among its siblings.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ReorderTo {
    /// One step toward the front (later in paint order).
    Forward,
    /// One step toward the back.
    Backward,
    /// To the front.
    Front,
    /// To the back.
    Back,
}

/// Move one node in paint order among its siblings: `forward`,
/// `backward`, `front`, or `back`. In a row / column / grid frame this is
/// also the flow order.
pub(crate) fn reorder(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: ReorderParams = params(ctx, raw.clone())?;
    let id = target(ctx, p.id)?;
    let doc = ctx.editable()?;
    let node = id.clone();
    let op = match p.to {
        ReorderTo::Forward => Op::MoveForward { node },
        ReorderTo::Backward => Op::MoveBackward { node },
        ReorderTo::Front => Op::MoveToFront { node },
        ReorderTo::Back => Op::MoveToBack { node },
    };
    edit(ctx, &doc, vec![op], vec![id], raw)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroupParams {
    #[serde(default)]
    ids: Option<Vec<String>>,
    #[serde(default)]
    group_id: Option<String>,
}

/// Wrap `ids` (default: the selection; siblings of one parent) in a new
/// group and select it. The group id defaults to the first free of
/// `group`, `group2`, ….
pub(crate) fn group(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: GroupParams = params(ctx, raw.clone())?;
    let ids = targets(ctx, p.ids)?;
    let doc = ctx.editable()?;
    let group_id = match p.group_id {
        Some(g) => g,
        None => unique_id("group", &all_ids(&doc)),
    };
    let op = Op::Group {
        node_ids: ids,
        group_id: group_id.clone(),
    };
    edit(ctx, &doc, vec![op], vec![group_id], raw)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UngroupParams {
    #[serde(default)]
    id: Option<String>,
}

/// Dissolve one group into its parent and select its former children.
pub(crate) fn ungroup(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: UngroupParams = params(ctx, raw.clone())?;
    let id = target(ctx, p.id)?;
    let doc = ctx.editable()?;
    let located = locate(&doc, &id).ok_or_else(|| EditorError::unknown_node(&id))?;
    let children: Vec<String> = child_lists(located.node)
        .into_iter()
        .flatten()
        .filter_map(|n| n.id().map(str::to_owned))
        .collect();
    let op = Op::Ungroup { group_id: id };
    edit(ctx, &doc, vec![op], children, raw)
}
