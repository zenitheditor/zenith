//! [`plan`]: a gesture as ops, for one node or a selection.

use serde_json::Value;
use zenith_core::Document;
use zenith_tx::Op;

use super::facts::{Anchoring, Axis, AxisValue, BoxFacts};
use super::flags::{GestureParams, HandleRef};
use super::kind::Kind;
use super::member::{context_notes, handle_drag, mapped, translate, turn};
use super::multi::plan_multi;
use super::refusal::refuse;
use super::snap::{Aabb, GripDrag, Scene, Snapped, snap_grip, snap_move};
use super::target::{Target, check_editable, resolve};
use crate::ctx::Ctx;
use crate::doc::tree::locate;
use crate::error::EditorError;
use crate::geom::{Pt, linear_part};
use crate::wire::DiagnosticOut;

/// A gesture mapped to ops.
pub(crate) struct Plan<'d> {
    /// The nodes, in request order: the selection after the gesture.
    pub(crate) targets: Vec<Target<'d>>,
    /// The ops, in order. Empty when the gesture changes nothing.
    pub(crate) ops: Vec<Op>,
    /// How the mapping went, for the reply.
    pub(crate) notes: Vec<DiagnosticOut>,
    /// The snap, when the gesture asked for one (`snap_distance`).
    pub(crate) snap: Option<Snapped>,
}

impl Plan<'_> {
    /// The node ids, in request order.
    pub(crate) fn ids(&self) -> Vec<String> {
        self.targets.iter().map(|t| t.id.clone()).collect()
    }
}

/// Map the gesture `p` on `doc` (the parse of the current, valid text) to
/// ops. `raw` is the request params, for offers.
///
/// The nodes are `nodes`, else `node`, else the selection. One node maps
/// per its kind and handle; several map as one selection (see
/// [`plan_multi`]).
///
/// # Errors
///
/// `editor.invalid_params`, `editor.no_selection`, `editor.locked` /
/// `editor.hidden` (with `unlock` / `show` offers), `editor.unknown_node`,
/// `editor.unknown_handle`, `editor.unsupported`, `editor.rotate_needs_h`,
/// `editor.singular_transform`, `editor.mixed_pages`, or `editor.rejected`
/// with `tx.*` diagnostics and the offers that go ahead.
pub(crate) fn plan<'d>(
    ctx: &mut Ctx<'_, '_>,
    doc: &'d Document,
    p: &GestureParams,
    raw: &Value,
) -> Result<Plan<'d>, EditorError> {
    let finite_snap = p.snap_distance.is_none_or(|s| s.is_finite() && s >= 0.0);
    if !(p.dx.is_finite() && p.dy.is_finite() && p.angle.is_finite() && finite_snap) {
        return Err(EditorError::new(
            "editor.invalid_params",
            "dx, dy, and angle must be finite numbers, and snap_distance finite and >= 0",
        ));
    }
    let ids = members(ctx, p)?;
    match ids.as_slice() {
        [one] => single(ctx, doc, p, raw, one),
        _ => plan_multi(ctx, doc, p, raw, ids),
    }
}

/// The nodes a gesture acts on: `nodes` (repeats dropped), else `node`,
/// else the selection.
fn members(ctx: &Ctx<'_, '_>, p: &GestureParams) -> Result<Vec<String>, EditorError> {
    let ids = match (&p.node, &p.nodes) {
        (Some(_), Some(_)) => {
            return Err(EditorError::new(
                "editor.invalid_params",
                format!("'{}' takes node or nodes, not both", ctx.command),
            ));
        }
        (Some(one), None) => vec![one.clone()],
        (None, Some(list)) => list.clone(),
        (None, None) => ctx.session.selection.clone(),
    };
    let mut out: Vec<String> = Vec::with_capacity(ids.len());
    for id in ids {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    if out.is_empty() {
        return Err(EditorError::no_selection(ctx.command));
    }
    Ok(out)
}

/// The pointer delta, kept to its dominant axis for a constrained move.
pub(crate) fn constrained(p: &GestureParams, handle: HandleRef) -> Pt {
    if p.constrain && handle == HandleRef::Move {
        if p.dx.abs() >= p.dy.abs() {
            (p.dx, 0.0)
        } else {
            (0.0, p.dy)
        }
    } else {
        (p.dx, p.dy)
    }
}

/// The snap threshold in page px, when the gesture snaps: a move or a grip
/// drag with `snap_distance > 0`.
pub(crate) fn snap_threshold(p: &GestureParams, handle: HandleRef) -> Option<f64> {
    let snaps = matches!(handle, HandleRef::Move | HandleRef::Grip(_));
    p.snap_distance.filter(|d| snaps && *d > 0.0)
}

/// Snap the drag `delta` of `moving` (page box) for `handle`.
pub(crate) fn snap_box(
    scene: &Scene,
    moving: Aabb,
    handle: HandleRef,
    p: &GestureParams,
    delta: Pt,
    threshold: f64,
) -> Snapped {
    match handle {
        HandleRef::Grip(grip) => snap_grip(
            scene,
            moving,
            GripDrag {
                grip,
                constrain: p.constrain,
                from_center: p.from_center,
            },
            delta,
            threshold,
        ),
        HandleRef::Move
        | HandleRef::Rotate
        | HandleRef::LineStart
        | HandleRef::LineEnd
        | HandleRef::Vertex(_)
        | HandleRef::Anchor { .. }
        | HandleRef::Control { .. } => snap_move(scene, moving, delta, threshold),
    }
}

/// One node: per its kind and handle.
fn single<'d>(
    ctx: &mut Ctx<'_, '_>,
    doc: &'d Document,
    p: &GestureParams,
    raw: &Value,
    id: &str,
) -> Result<Plan<'d>, EditorError> {
    let located = locate(doc, id).ok_or_else(|| EditorError::unknown_node(id))?;
    check_editable(&located, id)?;
    let handle = HandleRef::parse(p.handle.as_deref())?;
    let t = resolve(ctx, doc, id)?;
    let mut notes = context_notes(&t);
    let mut delta = constrained(p, handle);
    let mut snap = None;
    if let Some(threshold) = snap_threshold(p, handle) {
        let s = if snaps_on_page(&t, handle) {
            let scene = Scene::of(doc, std::slice::from_ref(&t));
            match Aabb::of(t.bx.corners()) {
                Some(moving) => snap_box(&scene, moving, handle, p, delta, threshold),
                None => unsnapped(delta),
            }
        } else {
            unsnapped(delta)
        };
        delta = (s.dx, s.dy);
        snap = Some(s);
    }
    let m = mapped(&t, delta)?;
    let built = if handle == HandleRef::Rotate {
        turn(&t, p.angle, p.snap).map(|ops| (ops, Vec::new()))
    } else if handle == HandleRef::Move {
        translate(doc, &t, m, p.flags())
    } else {
        handle_drag(doc, &t, handle, m, p.constrain, p.from_center, p.flags())
    };
    let (ops, more) = built.map_err(|r| refuse(ctx, raw, vec![r]))?;
    notes.extend(more);
    Ok(Plan {
        targets: vec![t],
        ops,
        notes,
        snap,
    })
}

/// `true` when a drag of `handle` moves the page box of `t` like the
/// pointer: any move, and a grip on a box node drawn with no turn of its own
/// under an unflipped axis-aligned scale. Other grips do not snap.
fn snaps_on_page(t: &Target<'_>, handle: HandleRef) -> bool {
    match handle {
        HandleRef::Move => true,
        HandleRef::Grip(_) => {
            let w = t.bx.world;
            Kind::of(t.located.node) == Kind::Box
                && linear_part(t.bx.spin) == zenith_scene::Affine2::IDENTITY
                && w.b == 0.0
                && w.c == 0.0
                && w.a > 0.0
                && w.d > 0.0
        }
        HandleRef::Rotate
        | HandleRef::LineStart
        | HandleRef::LineEnd
        | HandleRef::Vertex(_)
        | HandleRef::Anchor { .. }
        | HandleRef::Control { .. } => false,
    }
}

/// A snap that changed nothing.
pub(crate) fn unsnapped(delta: Pt) -> Snapped {
    Snapped {
        dx: delta.0,
        dy: delta.1,
        guides: Vec::new(),
    }
}

/// The current resolved authored px value of each box axis of `t`: `x` /
/// `y` in the space that holds them (through the parent origin), `w` /
/// `h` as drawn. An absent `x` / `y` with no anchor outside a layout flow
/// is the default 0 of its space; an anchored one is where the anchor
/// puts it.
pub(crate) fn current_axes(facts: &BoxFacts, t: &Target<'_>) -> [(Axis, Option<f64>); 4] {
    let local = t.bx.local;
    let position = |axis: Axis, at: f64, origin: Option<f64>| {
        if facts.flow.is_none()
            && facts.anchoring == Anchoring::None
            && *facts.value(axis) == AxisValue::Absent
        {
            Some(0.0)
        } else {
            origin.map(|o| at - o)
        }
    };
    [
        (Axis::X, position(Axis::X, local.x, t.origin.map(|o| o.0))),
        (Axis::Y, position(Axis::Y, local.y, t.origin.map(|o| o.1))),
        (Axis::W, Some(local.w)),
        (Axis::H, Some(local.h)),
    ]
}

/// The page corners of the node with compiled id `raw_id` in `boxes`.
pub(crate) fn corners_of(
    boxes: &std::collections::BTreeMap<String, zenith_scene::CompiledBox>,
    raw_id: &str,
) -> Option<[Pt; 4]> {
    boxes.get(raw_id).map(zenith_scene::CompiledBox::corners)
}
