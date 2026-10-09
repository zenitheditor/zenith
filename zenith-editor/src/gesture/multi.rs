//! Selection gestures: several nodes moved, resized, or turned as one, in
//! one transaction (one history entry).
//!
//! - **Move**: every node moves by the same page delta, each mapped into its
//!   own parent space (mixed parents, rotated or scaled containers).
//! - **Resize** (a grip of the selection box, the page bounds of every
//!   node): the page plane maps the selection box onto the resized one;
//!   each node's centre follows the map and its own axes stretch by the
//!   map's scale along them. Exact for nodes whose axes are page axes; a
//!   turned node keeps its angle (it is not sheared).
//! - **Rotate**: every node turns by the same angle and its centre orbits
//!   the selection box centre. A line turns its endpoints.
//!
//! Each node maps as a single-node gesture would, with the same policy
//! (tokens, anchors, flow, computed sizes, locks). Any node that refuses
//! refuses the whole gesture; the reply lists every refusing node's
//! diagnostics and the offers, which resend the gesture with the flag for
//! every node. A node inside another selected node rides along with it
//! (no op of its own). A connector (geometry from its targets) and a
//! footnote or light (no canvas geometry) ride along too, with a note.

use serde_json::Value;
use zenith_core::Document;

use super::flags::{GestureParams, HandleRef};
use super::kind::Kind;
use super::member::{
    PageFit, centre, context_notes, fit, mapped, orbit, orbit_line, translate, turn,
};
use super::plan::{Plan, constrained, snap_box, snap_threshold, unsnapped};
use super::refusal::{Refusal, refuse};
use super::snap::{Aabb, Scene};
use super::target::{Target, check_editable, resolve_all};
use crate::ctx::Ctx;
use crate::doc::tree::locate;
use crate::error::EditorError;
use crate::geom::{Drag, Pt, resize};
use crate::wire::DiagnosticOut;

/// Plan gesture `p` on the nodes `ids` (two or more) as one selection.
///
/// # Errors
///
/// As [`plan`](super::plan::plan); `editor.unknown_handle` for a point
/// handle (endpoints, vertices, and anchors belong to one node).
pub(crate) fn plan_multi<'d>(
    ctx: &mut Ctx<'_, '_>,
    doc: &'d Document,
    p: &GestureParams,
    raw: &Value,
    ids: Vec<String>,
) -> Result<Plan<'d>, EditorError> {
    let mut refusals: Vec<Refusal> = Vec::new();
    for id in &ids {
        let located = locate(doc, id).ok_or_else(|| EditorError::unknown_node(id))?;
        if let Err(e) = check_editable(&located, id) {
            refusals.push(e.into());
        }
    }
    let handle = HandleRef::parse(p.handle.as_deref())?;
    if !matches!(
        handle,
        HandleRef::Move | HandleRef::Grip(_) | HandleRef::Rotate
    ) {
        return Err(EditorError::new(
            "editor.unknown_handle",
            format!(
                "handle '{}' belongs to one node, and {} are selected; drag the selection \
                 body, a selection grip, or rotate, or select one node",
                handle.id(),
                ids.len()
            ),
        ));
    }
    if !refusals.is_empty() {
        return Err(refuse(ctx, raw, refusals));
    }
    let targets = resolve_all(ctx, doc, &ids)?;
    let selection_box = union(&targets).ok_or_else(|| {
        EditorError::new(
            "editor.no_box",
            "the selection has no finite page box; edit it in the code",
        )
    })?;
    let mut notes: Vec<DiagnosticOut> = Vec::new();
    let mut delta = constrained(p, handle);
    let mut snap = None;
    if let Some(threshold) = snap_threshold(p, handle) {
        let scene = Scene::of(doc, &targets);
        let s = if handle == HandleRef::Rotate {
            unsnapped(delta)
        } else {
            snap_box(&scene, selection_box, handle, p, delta, threshold)
        };
        delta = (s.dx, s.dy);
        snap = Some(s);
    }
    let angle = match p.snap.filter(|s| s.is_finite() && *s > 0.0) {
        Some(step) => (p.angle / step).round() * step,
        None => p.angle,
    };
    let pivot = centre(&selection_box.corners());
    let page_fit = match handle {
        HandleRef::Grip(grip) => {
            let from = selection_box.rect();
            let to = resize(
                from,
                zenith_scene::Affine2::IDENTITY,
                Drag {
                    grip,
                    delta,
                    constrain: p.constrain,
                    from_center: p.from_center,
                },
            )
            .apply(from);
            Some(PageFit { from, to })
        }
        HandleRef::Move
        | HandleRef::Rotate
        | HandleRef::LineStart
        | HandleRef::LineEnd
        | HandleRef::Vertex(_)
        | HandleRef::Anchor { .. }
        | HandleRef::Control { .. } => None,
    };
    let flags = p.flags();
    let mut ops = Vec::new();
    for t in &targets {
        if rides_along(t, &ids) {
            continue;
        }
        if let Some(note) = no_geometry(t) {
            notes.push(note);
            continue;
        }
        notes.extend(context_notes(t));
        let built = match (handle, page_fit) {
            (HandleRef::Grip(_), Some(f)) => fit(doc, t, f, flags),
            (HandleRef::Rotate, _) if Kind::of(t.located.node) == Kind::Line => {
                orbit_line(t, pivot, angle)
            }
            (HandleRef::Rotate, _) => turned(doc, t, pivot, angle, flags),
            _ => mapped(t, delta)
                .map_err(Refusal::from)
                .and_then(|m| translate(doc, t, m, flags)),
        };
        match built {
            Ok((more, more_notes)) => {
                ops.extend(more);
                notes.extend(more_notes);
            }
            Err(r) => refusals.push(r),
        }
    }
    if !refusals.is_empty() {
        return Err(refuse(ctx, raw, refusals));
    }
    Ok(Plan {
        targets,
        ops,
        notes,
        snap,
    })
}

/// Turn `t` by `angle` and carry its centre round `pivot`.
fn turned(
    doc: &Document,
    t: &Target<'_>,
    pivot: Pt,
    angle: f64,
    flags: super::flags::Flags,
) -> Result<super::member::Built, Refusal> {
    let mut ops = turn(t, angle, None)?;
    let m = mapped(t, orbit(t, pivot, angle))?;
    let (moved, notes) = translate(doc, t, m, flags)?;
    ops.extend(moved);
    Ok((ops, notes))
}

/// `true` when an ancestor of `t` is also in the gesture: it moves with
/// that ancestor.
pub(crate) fn rides_along(t: &Target<'_>, ids: &[String]) -> bool {
    t.located
        .ancestors
        .iter()
        .filter_map(|a| a.id())
        .any(|a| ids.iter().any(|id| id == a))
}

/// The note for a node a selection gesture carries without an op: a
/// connector, footnote, light, or unknown node.
fn no_geometry(t: &Target<'_>) -> Option<DiagnosticOut> {
    let id = &t.id;
    let kind = t.located.node.kind_str();
    match Kind::of(t.located.node) {
        Kind::Derived => Some(DiagnosticOut::advisory(
            "editor.follows_targets",
            format!("connector '{id}' follows its from/to targets; it has no move of its own"),
        )),
        Kind::Fixed => Some(DiagnosticOut::advisory(
            "editor.no_geometry",
            format!("{kind} '{id}' has no canvas geometry; the gesture leaves it as it is"),
        )),
        Kind::Box | Kind::Line | Kind::Points | Kind::Path => None,
    }
}

/// The page bounds of every target's drawn box.
pub(crate) fn union(targets: &[Target<'_>]) -> Option<Aabb> {
    Aabb::of(targets.iter().flat_map(|t| t.bx.corners()))
}

/// The page bounds of `boxes` by raw id, for the preview reply.
pub(crate) fn union_of_raw(
    boxes: &std::collections::BTreeMap<String, zenith_scene::CompiledBox>,
    raw_ids: &[&str],
) -> Option<Aabb> {
    Aabb::of(
        raw_ids
            .iter()
            .filter_map(|id| boxes.get(*id))
            .flat_map(zenith_scene::CompiledBox::corners),
    )
}
