//! One node's part of a gesture: the ops that move, resize, turn, or edit
//! the points of one [`Target`], or the [`Refusal`] that stops it.

use zenith_core::{Diagnostic, Document, Node};
use zenith_tx::Op;

use super::boxplan::{BoxInput, BoxPlan, Motion, plan_box};
use super::facts::{Axis, box_facts};
use super::flags::{Flags, HandleRef};
use super::kind::Kind;
use super::plan::current_axes;
use super::refusal::Refusal;
use super::reorder::plan_reorder;
use super::rotate::plan_rotate;
use super::shapes::{ShapeDrag, ShapeFit, plan_fit, plan_shape};
use super::target::Target;
use crate::doc::place::pivot_follows_content;
use crate::doc::shape::{Shape, bounds, shape_of};
use crate::error::EditorError;
use crate::geom::{Drag, Pt, Rect, add, linear, resize, sub, unmap_vector};
use crate::wire::DiagnosticOut;

/// Ops and the notes on how the mapping went.
pub(crate) type Built = (Vec<Op>, Vec<DiagnosticOut>);

/// A page px delta mapped into one node's spaces.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Mapped {
    /// The delta in the node's authored (parent) space: through `world`.
    pub(crate) shift: Pt,
    /// The delta in the node's own unrotated frame: through `world ∘ spin`.
    pub(crate) own: Pt,
}

/// `delta` (page px) in the spaces of `t`.
///
/// # Errors
///
/// `editor.singular_transform` under a zero scale.
pub(crate) fn mapped(t: &Target<'_>, delta: Pt) -> Result<Mapped, EditorError> {
    let singular = || {
        EditorError::new(
            "editor.singular_transform",
            format!(
                "'{}' is drawn under a zero scale, so a page delta has no node delta",
                t.id
            ),
        )
    };
    Ok(Mapped {
        shift: unmap_vector(t.bx.world, delta).ok_or_else(singular)?,
        own: unmap_vector(t.bx.transform(), delta).ok_or_else(singular)?,
    })
}

/// The advisories every gesture on `t` carries: an ancestor group turns
/// about its content, or a table sizes the cell.
pub(crate) fn context_notes(t: &Target<'_>) -> Vec<DiagnosticOut> {
    let id = &t.id;
    let mut notes = Vec::new();
    if pivot_follows_content(&t.located) {
        notes.push(DiagnosticOut::advisory(
            "editor.pivot_follows_content",
            format!(
                "an ancestor group of '{id}' turns about its content bounds, so the page \
                 position after this gesture is exact only up to that pivot's shift"
            ),
        ));
    }
    if matches!(t.located.parent(), Some(Node::Table(_))) {
        notes.push(DiagnosticOut::advisory(
            "editor.table_managed",
            format!(
                "'{id}' sits in a table cell; the table sizes its rows from cell content, so \
                 the cell can grow or shrink with this gesture"
            ),
        ));
    }
    notes
}

/// Move `t` by `m` (see [`mapped`]), per its kind.
pub(crate) fn translate(
    doc: &Document,
    t: &Target<'_>,
    m: Mapped,
    flags: Flags,
) -> Result<Built, Refusal> {
    let node = t.located.node;
    let id = t.id.as_str();
    match Kind::of(node) {
        Kind::Box => {
            let facts = box_facts(doc, node, id).ok_or_else(|| unsupported(t, "has no box"))?;
            let input = BoxInput {
                id,
                facts: &facts,
                motion: Motion::Move,
                deltas: [
                    (Axis::X, m.shift.0),
                    (Axis::Y, m.shift.1),
                    (Axis::W, 0.0),
                    (Axis::H, 0.0),
                ],
                current: current_axes(&facts, t),
                flags,
            };
            match plan_box(&input) {
                BoxPlan::Ops { ops, notes } => Ok((ops, notes)),
                BoxPlan::Reorder => {
                    let ops = match &facts.flow {
                        Some(flow) => {
                            let prefix = t
                                .page
                                .raw_id
                                .strip_suffix(id)
                                .unwrap_or_default()
                                .to_owned();
                            plan_reorder(doc, &t.located, id, flow, &t.view.boxes, &prefix, m.shift)
                                .into_iter()
                                .collect()
                        }
                        None => Vec::new(),
                    };
                    Ok((ops, Vec::new()))
                }
                BoxPlan::Rejected(diagnostics) => Err(Refusal::Rejected(diagnostics)),
            }
        }
        Kind::Line | Kind::Points | Kind::Path => {
            let shape = shape(t)?;
            let ops = plan_shape(id, &shape, HandleRef::Move, drag(t, m, false, false))?;
            Ok((ops, Vec::new()))
        }
        Kind::Derived => Err(derived(id)),
        Kind::Fixed => Err(unsupported(t, "has no canvas geometry to edit").into()),
    }
}

/// Drag handle `handle` (a grip or a point handle, not rotate) of `t` by
/// `m`.
pub(crate) fn handle_drag(
    doc: &Document,
    t: &Target<'_>,
    handle: HandleRef,
    m: Mapped,
    constrain: bool,
    from_center: bool,
    flags: Flags,
) -> Result<Built, Refusal> {
    let node = t.located.node;
    let id = t.id.as_str();
    match Kind::of(node) {
        Kind::Box => {
            box_facts(doc, node, id).ok_or_else(|| unsupported(t, "has no box"))?;
            let HandleRef::Grip(grip) = handle else {
                return Err(no_handle(t, handle).into());
            };
            let r = resize(
                rect_of(t),
                t.bx.spin,
                Drag {
                    grip,
                    delta: m.own,
                    constrain,
                    from_center,
                },
            );
            box_resize(doc, t, [r.dx, r.dy, r.dw, r.dh], flags)
        }
        Kind::Line | Kind::Points | Kind::Path => {
            let shape = shape(t)?;
            let ops = plan_shape(id, &shape, handle, drag(t, m, constrain, from_center))?;
            Ok((ops, Vec::new()))
        }
        Kind::Derived => Err(derived(id)),
        Kind::Fixed => Err(unsupported(t, "has no canvas geometry to edit").into()),
    }
}

/// `plan_box` of a resize of box node `t` by authored `[dx, dy, dw, dh]`.
fn box_resize(
    doc: &Document,
    t: &Target<'_>,
    deltas: [f64; 4],
    flags: Flags,
) -> Result<Built, Refusal> {
    let node = t.located.node;
    let id = t.id.as_str();
    let facts = box_facts(doc, node, id).ok_or_else(|| unsupported(t, "has no box"))?;
    let [dx, dy, dw, dh] = deltas;
    let input = BoxInput {
        id,
        facts: &facts,
        motion: Motion::Resize,
        deltas: [(Axis::X, dx), (Axis::Y, dy), (Axis::W, dw), (Axis::H, dh)],
        current: current_axes(&facts, t),
        flags,
    };
    match plan_box(&input) {
        BoxPlan::Ops { ops, notes } => Ok((ops, notes)),
        // A resize never asks for a flow slot.
        BoxPlan::Reorder => Ok((Vec::new(), Vec::new())),
        BoxPlan::Rejected(diagnostics) => Err(Refusal::Rejected(diagnostics)),
    }
}

/// Turn `t` by `angle` degrees about its own pivot (see [`plan_rotate`]).
pub(crate) fn turn(t: &Target<'_>, angle: f64, snap: Option<f64>) -> Result<Vec<Op>, Refusal> {
    plan_rotate(t.located.node, &t.id, t.bx.world, angle, snap).map_err(|e| {
        if e.code.starts_with("tx.") {
            Refusal::Rejected(vec![Diagnostic::error(
                &e.code,
                e.message,
                None,
                Some(t.id.clone()),
            )])
        } else {
            Refusal::Error(e)
        }
    })
}

/// A page-plane map for a selection resize: the box `from` scaled and
/// moved onto `to`, both page px and axis-aligned.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PageFit {
    pub(crate) from: Rect,
    pub(crate) to: Rect,
}

impl PageFit {
    /// The page scale factors.
    pub(crate) fn scale(self) -> Pt {
        let f = |to: f64, from: f64| if from > 0.0 { to / from } else { 1.0 };
        (f(self.to.w, self.from.w), f(self.to.h, self.from.h))
    }

    /// How far page point `p` moves. Exactly 0 on an axis the fit leaves
    /// alone.
    pub(crate) fn shift_of(self, p: Pt) -> Pt {
        let (sx, sy) = self.scale();
        (
            (self.to.x - self.from.x) + (p.0 - self.from.x) * (sx - 1.0),
            (self.to.y - self.from.y) + (p.1 - self.from.y) * (sy - 1.0),
        )
    }

    /// The factors by which the fit stretches the own x and y axes of a
    /// node drawn with `transform`: exact for a node whose axes are page
    /// axes, the stretch of each own axis otherwise (the node keeps its
    /// angle; it is not sheared).
    fn own_scale(self, transform: zenith_scene::Affine2) -> Pt {
        let (sx, sy) = self.scale();
        let stretch = |v: Pt| {
            let len = v.0.hypot(v.1);
            if len > 0.0 {
                (v.0 * sx).hypot(v.1 * sy) / len
            } else {
                1.0
            }
        };
        let fx = if sx == 1.0 && sy == 1.0 {
            1.0
        } else {
            stretch(linear(transform, (1.0, 0.0)))
        };
        let fy = if sx == 1.0 && sy == 1.0 {
            1.0
        } else {
            stretch(linear(transform, (0.0, 1.0)))
        };
        (exact_one(fx), exact_one(fy))
    }
}

/// Resize `t` as part of a selection resize `fit`: its centre follows the
/// page map and its own axes stretch by the map's scale along them.
pub(crate) fn fit(
    doc: &Document,
    t: &Target<'_>,
    fit: PageFit,
    flags: Flags,
) -> Result<Built, Refusal> {
    let node = t.located.node;
    let id = t.id.as_str();
    let (fw, fh) = fit.own_scale(t.bx.transform());
    let world = t.bx.world;
    let parent = |page: Pt| {
        unmap_vector(world, page)
            .map(|(x, y)| (tidy(x), tidy(y)))
            .ok_or_else(|| Refusal::Error(singular(t)))
    };
    match Kind::of(node) {
        Kind::Box => {
            let c = centre(&t.bx.corners());
            let shift = parent(fit.shift_of(c))?;
            let local = t.bx.local;
            let dw = tidy(local.w * (fw - 1.0));
            let dh = tidy(local.h * (fh - 1.0));
            box_resize(
                doc,
                t,
                [tidy(shift.0 - dw / 2.0), tidy(shift.1 - dh / 2.0), dw, dh],
                flags,
            )
        }
        Kind::Line => {
            let Shape::Line { start, end } = shape(t)? else {
                return Err(unsupported(t, "has no endpoints").into());
            };
            let origin = origin(t)?;
            let page = |p: Pt| world.apply(p.0 + origin.0, p.1 + origin.1);
            let d1 = parent(fit.shift_of(page(start)))?;
            let d2 = parent(fit.shift_of(page(end)))?;
            Ok((
                vec![Op::NudgeLinePoints {
                    node: id.to_owned(),
                    dx1: Some(d1.0),
                    dy1: Some(d1.1),
                    dx2: Some(d2.0),
                    dy2: Some(d2.1),
                }],
                Vec::new(),
            ))
        }
        Kind::Points | Kind::Path => {
            let shape = shape(t)?;
            let origin = origin(t)?;
            let (x, y, w, h) = bounds(&shape.frame_points()).unwrap_or_default();
            let frame_centre = (x + w / 2.0, y + h / 2.0);
            let page = world.apply(frame_centre.0 + origin.0, frame_centre.1 + origin.1);
            let shift = parent(fit.shift_of(page))?;
            let ops = plan_fit(
                id,
                &shape,
                ShapeFit {
                    from: Rect { x, y, w, h },
                    scale: (fw, fh),
                    shift,
                },
            );
            Ok((ops, Vec::new()))
        }
        Kind::Derived => Err(derived(id)),
        Kind::Fixed => Err(unsupported(t, "has no canvas geometry to edit").into()),
    }
}

/// The page delta that carries the centre of `t` when the selection turns
/// `angle` degrees (clockwise on screen) about the page point `pivot`.
pub(crate) fn orbit(t: &Target<'_>, pivot: Pt, angle: f64) -> Pt {
    let c = centre(&t.bx.corners());
    let turned = zenith_scene::Affine2::rotate_at(angle, pivot.0, pivot.1).apply(c.0, c.1);
    sub(turned, c)
}

/// Turn a line `t` by `angle` degrees about the page point `pivot`: both
/// endpoints move (a line has no `rotate`).
pub(crate) fn orbit_line(t: &Target<'_>, pivot: Pt, angle: f64) -> Result<Built, Refusal> {
    let Shape::Line { start, end } = shape(t)? else {
        return Err(unsupported(t, "has no endpoints").into());
    };
    let origin = origin(t)?;
    let world = t.bx.world;
    let turn = zenith_scene::Affine2::rotate_at(angle, pivot.0, pivot.1);
    let delta = |p: Pt| {
        let at = world.apply(p.0 + origin.0, p.1 + origin.1);
        let to = turn.apply(at.0, at.1);
        unmap_vector(world, sub(to, at)).ok_or_else(|| Refusal::Error(singular(t)))
    };
    let d1 = delta(start)?;
    let d2 = delta(end)?;
    Ok((
        vec![Op::NudgeLinePoints {
            node: t.id.clone(),
            dx1: Some(d1.0),
            dy1: Some(d1.1),
            dx2: Some(d2.0),
            dy2: Some(d2.1),
        }],
        Vec::new(),
    ))
}

/// The centre of a quadrilateral.
pub(crate) fn centre(corners: &[Pt; 4]) -> Pt {
    let sum = corners.iter().fold((0.0, 0.0), |s, c| add(s, *c));
    (sum.0 / 4.0, sum.1 / 4.0)
}

/// `v`, with values within 1e-9 of 0 made exactly 0 (float noise from the
/// page map would otherwise write `40.00000000000001`).
fn tidy(v: f64) -> f64 {
    if v.abs() < 1e-9 { 0.0 } else { v }
}

/// `v`, with values within 1e-12 of 1 made exactly 1.
fn exact_one(v: f64) -> f64 {
    if (v - 1.0).abs() < 1e-12 { 1.0 } else { v }
}

fn drag(t: &Target<'_>, m: Mapped, constrain: bool, from_center: bool) -> ShapeDrag {
    ShapeDrag {
        spin: t.bx.spin,
        shift: m.shift,
        own: m.own,
        constrain,
        from_center,
    }
}

/// The shape of a line, polygon, polyline, or path target.
fn shape(t: &Target<'_>) -> Result<Shape, Refusal> {
    let id = &t.id;
    match shape_of(t.located.node) {
        Some(Ok(shape)) => Ok(shape),
        Some(Err(why)) => Err(Refusal::Rejected(vec![Diagnostic::error(
            "tx.value_unresolved",
            format!("{} of '{id}' has no px value; edit it in the code", why.0),
            None,
            Some(id.clone()),
        )])),
        None => Err(unsupported(t, "has no editable geometry").into()),
    }
}

fn origin(t: &Target<'_>) -> Result<Pt, Refusal> {
    t.origin.ok_or_else(|| {
        Refusal::Error(EditorError::new(
            "editor.origin_unresolved",
            format!(
                "the px position of '{}' does not resolve, because a container offset is not \
                 px; write it in the code",
                t.id
            ),
        ))
    })
}

fn singular(t: &Target<'_>) -> EditorError {
    EditorError::new(
        "editor.singular_transform",
        format!(
            "'{}' is drawn under a zero scale, so a page delta has no node delta",
            t.id
        ),
    )
}

fn derived(id: &str) -> Refusal {
    Refusal::Rejected(vec![Diagnostic::error(
        "tx.derived_geometry",
        format!(
            "connector '{id}' takes its geometry from its from/to targets; move the targets, \
             or change from / to in the code"
        ),
        None,
        Some(id.to_owned()),
    )])
}

/// The resize frame of a box node: its compiled box in its own space.
fn rect_of(t: &Target<'_>) -> Rect {
    Rect {
        x: t.bx.local.x,
        y: t.bx.local.y,
        w: t.bx.local.w,
        h: t.bx.local.h,
    }
}

pub(crate) fn unsupported(t: &Target<'_>, why: &str) -> EditorError {
    EditorError::new(
        "editor.unsupported",
        format!(
            "{} '{}' {why}; edit it in the code",
            t.located.node.kind_str(),
            t.id
        ),
    )
}

pub(crate) fn no_handle(t: &Target<'_>, handle: HandleRef) -> EditorError {
    EditorError::new(
        "editor.unknown_handle",
        format!(
            "handle '{}' does not exist on {} '{}'; use an id node.handles returns",
            handle.id(),
            t.located.node.kind_str(),
            t.id
        ),
    )
}
