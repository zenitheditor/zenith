//! `node.handles {ids}` for a selection of two or more nodes: the
//! selection box (the page bounds of every node's drawn box), its 8 grips
//! and rotate grip, each node's outline, and what blocks a plain drag of
//! the selection.

use serde::Serialize;
use serde_json::Value;

use super::blocks::{Blocked, Handle, all_actions, box_blocks};
use crate::ctx::Ctx;
use crate::doc::place::pivot_follows_content;
use crate::doc::shape::shape_of;
use crate::error::EditorError;
use crate::geom::{Grip, Pt};
use crate::gesture::facts::box_facts;
use crate::gesture::kind::{Kind, rotate_blocked};
use crate::gesture::multi::{rides_along, union};
use crate::gesture::target::{Target, hidden_by, locked_by, resolve_all};
use crate::wire::to_json;

#[derive(Debug, Serialize)]
struct Member {
    id: String,
    kind: &'static str,
    /// The node's drawn box, page px.
    corners: [Pt; 4],
}

#[derive(Debug, Serialize)]
struct SelectionHandles {
    ids: Vec<String>,
    /// Always `selection`.
    kind: &'static str,
    page: usize,
    stale: bool,
    /// The selection box in page px: top-left, top-right, bottom-right,
    /// bottom-left.
    corners: [Pt; 4],
    center: Pt,
    /// Always 0: the selection box is axis-aligned.
    angle: f64,
    handles: Vec<Handle>,
    /// Blocks, each naming the `node` it comes from.
    disabled: Vec<Blocked>,
    members: Vec<Member>,
    /// Some node's ancestor group turns about its content bounds.
    pivot_follows_content: bool,
}

/// The handles of the selection `ids` (two or more distinct nodes of one
/// page; one gives that node's handles, repeats count once).
///
/// A grip or the rotate grip is disabled by any node's block on a move or a
/// resize (a selection resize or turn moves every node), except
/// `tx.layout_managed`, which a selection resize drops with a note; rotate
/// also by a node that takes no rotation. A node inside another selected
/// node, a connector, and a footnote or light add no block of their own
/// beyond a lock or hide.
///
/// # Errors
///
/// As `node.handles`; `editor.mixed_pages` when the nodes draw on several
/// pages.
pub(super) fn run(
    ctx: &mut Ctx<'_, '_>,
    ids: Vec<String>,
    rotate_offset: f64,
) -> Result<Value, EditorError> {
    let mut distinct: Vec<String> = Vec::with_capacity(ids.len());
    for id in ids {
        if !distinct.contains(&id) {
            distinct.push(id);
        }
    }
    let ids = match distinct.len() {
        0 => return Err(EditorError::no_selection(ctx.command)),
        1 => return super::node::run(ctx, distinct.pop(), rotate_offset),
        _ => distinct,
    };
    let display = ctx.display()?;
    let doc = &display.doc;
    let targets = resolve_all(ctx, doc, &ids)?;
    let bounds = union(&targets).ok_or_else(|| {
        EditorError::new(
            "editor.no_box",
            "the selection has no finite page box; edit it in the code",
        )
    })?;
    let mut disabled: Vec<Blocked> = Vec::new();
    if display.stale {
        disabled.extend(all_actions(
            "editor.buffer_invalid",
            "the text has errors; the canvas shows the last valid text",
        ));
    }
    for t in &targets {
        disabled.extend(member_blocks(doc, t, &ids).into_iter().map(|b| Blocked {
            node: Some(t.id.clone()),
            ..b
        }));
    }
    let blocks = |actions: &[&str]| {
        disabled
            .iter()
            .find(|b| actions.contains(&b.action) && b.code != "tx.layout_managed")
            .map(|b| b.code.clone())
    };
    let grip_reason = blocks(&["move", "resize"]);
    let rotate_reason = blocks(&["rotate", "move"]);
    let corners = bounds.corners();
    let rect = bounds.rect();
    let mut handles: Vec<Handle> = Grip::ALL
        .into_iter()
        .map(|grip| {
            let (x, y) = rect.at(grip.fractions());
            Handle {
                id: grip.id().to_owned(),
                role: "resize",
                x,
                y,
                enabled: grip_reason.is_none(),
                reason: grip_reason.clone(),
            }
        })
        .collect();
    let (top_x, top_y) = rect.at((0.5, 0.0));
    handles.push(Handle {
        id: "rotate".to_owned(),
        role: "rotate",
        x: top_x,
        y: top_y - rotate_offset,
        enabled: rotate_reason.is_none(),
        reason: rotate_reason,
    });
    let page = targets.first().map_or(1, |t| t.page.index + 1);
    let reply = SelectionHandles {
        ids: targets.iter().map(|t| t.id.clone()).collect(),
        kind: "selection",
        page,
        stale: display.stale,
        corners,
        center: rect.at((0.5, 0.5)),
        angle: 0.0,
        handles,
        disabled,
        members: targets
            .iter()
            .map(|t| Member {
                id: t.id.clone(),
                kind: t.located.node.kind_str(),
                corners: t.bx.corners(),
            })
            .collect(),
        pivot_follows_content: targets.iter().any(|t| pivot_follows_content(&t.located)),
    };
    to_json(&reply)
}

/// What blocks a selection gesture on node `t` of the selection `ids`.
fn member_blocks(doc: &zenith_core::Document, t: &Target<'_>, ids: &[String]) -> Vec<Blocked> {
    let mut out: Vec<Blocked> = Vec::new();
    if let Some(by) = locked_by(&t.located) {
        out.extend(all_actions("editor.locked", &format!("locked by '{by}'")));
    }
    if let Some(by) = hidden_by(&t.located) {
        out.extend(all_actions("editor.hidden", &format!("hidden by '{by}'")));
    }
    if rides_along(t, ids) {
        return out;
    }
    let node = t.located.node;
    match Kind::of(node) {
        Kind::Box => {
            if let Some(facts) = box_facts(doc, node, &t.id) {
                out.extend(box_blocks(&facts));
            }
        }
        Kind::Line | Kind::Points | Kind::Path => match (shape_of(node), t.origin) {
            (Some(Err(why)), _) => out.extend(all_actions("tx.value_unresolved", &why.0)),
            (Some(Ok(_)), None) => out.extend(all_actions(
                "editor.origin_unresolved",
                "a container offset is not px",
            )),
            (Some(Ok(_)) | None, _) => {}
        },
        Kind::Derived | Kind::Fixed => return out,
    }
    if Kind::of(node) != Kind::Line
        && let Some((code, why)) = rotate_blocked(node)
    {
        out.push(Blocked {
            action: "rotate",
            code: code.to_owned(),
            axes: Vec::new(),
            detail: why,
            node: None,
        });
    }
    out
}
