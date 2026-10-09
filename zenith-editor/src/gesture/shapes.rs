//! Gestures on the kinds without a box: lines, polygons / polylines, and
//! paths. All math runs in authored px; the node's own rotation (about the
//! bounds centre of its points or anchors) is compensated so the points a
//! gesture does not drag keep their page position.

use zenith_scene::Affine2;
use zenith_tx::{Op, OpPathHandle, OpPathTransform, OpPoint};

use super::flags::HandleRef;
use crate::doc::shape::{AnchorPx, Shape, bounds};
use crate::error::EditorError;
use crate::geom::{Drag, Grip, Pt, Rect, add, linear, linear_part, resize, sub};

/// The drag a shape gesture applies, already mapped from page px.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ShapeDrag {
    /// The node's own rotation (`CompiledBox::spin`).
    pub(crate) spin: Affine2,
    /// The pointer delta for a translation, in authored px (through the
    /// ancestors only).
    pub(crate) shift: Pt,
    /// The pointer delta in the node's own unrotated frame.
    pub(crate) own: Pt,
    pub(crate) constrain: bool,
    pub(crate) from_center: bool,
}

/// The ops for `handle` on a shape node `id`.
///
/// # Errors
///
/// `editor.unknown_handle` when the handle does not exist on this shape.
pub(crate) fn plan_shape(
    id: &str,
    shape: &Shape,
    handle: HandleRef,
    drag: ShapeDrag,
) -> Result<Vec<Op>, EditorError> {
    let missing = || {
        EditorError::new(
            "editor.unknown_handle",
            format!(
                "handle '{}' does not exist on '{id}'; use an id node.handles returns",
                handle.id()
            ),
        )
    };
    match (shape, handle) {
        (Shape::Line { .. }, HandleRef::Move) => Ok(vec![Op::NudgeLinePoints {
            node: id.to_owned(),
            dx1: Some(drag.shift.0),
            dy1: Some(drag.shift.1),
            dx2: Some(drag.shift.0),
            dy2: Some(drag.shift.1),
        }]),
        (Shape::Line { start, end }, HandleRef::LineStart | HandleRef::LineEnd) => {
            let start_moves = handle == HandleRef::LineStart;
            let (moving, fixed) = if start_moves {
                (*start, *end)
            } else {
                (*end, *start)
            };
            let d = if drag.constrain {
                snap_45(moving, fixed, drag.shift)
            } else {
                drag.shift
            };
            let (a, b) = if start_moves {
                (Some(d), None)
            } else {
                (None, Some(d))
            };
            Ok(vec![Op::NudgeLinePoints {
                node: id.to_owned(),
                dx1: a.map(|d| d.0),
                dy1: a.map(|d| d.1),
                dx2: b.map(|d| d.0),
                dy2: b.map(|d| d.1),
            }])
        }
        (Shape::Points(points), HandleRef::Move) => {
            Ok(set_points(id, points.iter().map(|p| add(*p, drag.shift))))
        }
        (Shape::Points(points), HandleRef::Grip(grip)) => {
            let (from, to) = resized_frame(points, grip, drag);
            Ok(set_points(id, points.iter().map(|p| remap(*p, from, to))))
        }
        (Shape::Points(points), HandleRef::Vertex(i)) => {
            let at = points.get(i).copied().ok_or_else(missing)?;
            let neighbour = if i == 0 {
                points.get(1)
            } else {
                points.get(i - 1)
            };
            let d = match neighbour {
                Some(n) if drag.constrain => snap_45(at, *n, drag.own),
                Some(_) | None => drag.own,
            };
            let mut next = points.clone();
            if let Some(p) = next.get_mut(i) {
                *p = add(*p, d);
            }
            let t = pivot_shift(points, &next, drag.spin);
            Ok(set_points(id, next.into_iter().map(|p| add(p, t))))
        }
        (Shape::Path { .. }, HandleRef::Move) => Ok(vec![translate(id, drag.shift)]),
        (Shape::Path { contours, .. }, HandleRef::Grip(grip)) => {
            let points: Vec<Pt> = contours.iter().flatten().map(|a| a.at).collect();
            let (from, to) = resized_frame(&points, grip, drag);
            let fixed = if drag.from_center {
                (0.5, 0.5)
            } else {
                grip.opposite().fractions()
            };
            let f = from.at(fixed);
            let sx = if from.w > 0.0 { to.w / from.w } else { 1.0 };
            let sy = if from.h > 0.0 { to.h / from.h } else { 1.0 };
            let mut ops = Vec::new();
            if sx != 1.0 || sy != 1.0 {
                ops.push(Op::TransformPathAnchors {
                    node: id.to_owned(),
                    transform: OpPathTransform::Scale {
                        sx,
                        sy,
                        cx: f.0,
                        cy: f.1,
                    },
                });
            }
            // Scaling about `f` lands the frame here; shift it to `to`.
            let scaled = (f.0 - fixed.0 * from.w * sx, f.1 - fixed.1 * from.h * sy);
            let t = sub((to.x, to.y), scaled);
            if t != (0.0, 0.0) {
                ops.push(translate(id, t));
            }
            Ok(ops)
        }
        (Shape::Path { contours, compound }, HandleRef::Anchor { subpath, index }) => {
            contours
                .get(subpath)
                .and_then(|c| c.get(index))
                .ok_or_else(missing)?;
            let before: Vec<Pt> = contours.iter().flatten().map(|a| a.at).collect();
            let after: Vec<Pt> = contours
                .iter()
                .enumerate()
                .flat_map(|(s, c)| {
                    c.iter().enumerate().map(move |(i, a)| {
                        if s == subpath && i == index {
                            add(a.at, drag.own)
                        } else {
                            a.at
                        }
                    })
                })
                .collect();
            let mut ops = vec![Op::MovePathAnchor {
                node: id.to_owned(),
                subpath_index: compound.then_some(subpath),
                anchor_index: index,
                dx: drag.own.0,
                dy: drag.own.1,
            }];
            let t = pivot_shift(&before, &after, drag.spin);
            if t != (0.0, 0.0) {
                ops.push(translate(id, t));
            }
            Ok(ops)
        }
        (
            Shape::Path { contours, compound },
            HandleRef::Control {
                subpath,
                index,
                which,
            },
        ) => {
            let anchor: &AnchorPx = contours
                .get(subpath)
                .and_then(|c| c.get(index))
                .ok_or_else(missing)?;
            let present = match which {
                OpPathHandle::In => anchor.handle_in.is_some(),
                OpPathHandle::Out => anchor.handle_out.is_some(),
            };
            if !present {
                return Err(missing());
            }
            Ok(vec![Op::MovePathHandle {
                node: id.to_owned(),
                subpath_index: compound.then_some(subpath),
                anchor_index: index,
                handle: which,
                dx: drag.own.0,
                dy: drag.own.1,
            }])
        }
        (
            Shape::Line { .. },
            HandleRef::Grip(_)
            | HandleRef::Rotate
            | HandleRef::Vertex(_)
            | HandleRef::Anchor { .. }
            | HandleRef::Control { .. },
        )
        | (
            Shape::Points(_),
            HandleRef::Rotate
            | HandleRef::LineStart
            | HandleRef::LineEnd
            | HandleRef::Anchor { .. }
            | HandleRef::Control { .. },
        )
        | (
            Shape::Path { .. },
            HandleRef::Rotate | HandleRef::LineStart | HandleRef::LineEnd | HandleRef::Vertex(_),
        ) => Err(missing()),
    }
}

/// A selection resize as it reaches one polygon, polyline, or path: its
/// frame (the bounds of its points or anchors, authored px) stretches by
/// `scale` about its centre, then moves by `shift` (authored px).
#[derive(Debug, Clone, Copy)]
pub(crate) struct ShapeFit {
    pub(crate) from: Rect,
    pub(crate) scale: Pt,
    pub(crate) shift: Pt,
}

/// The ops of a selection resize of shape `id`. Lines take their endpoint
/// deltas directly and are not handled here; they get no op.
pub(crate) fn plan_fit(id: &str, shape: &Shape, fit: ShapeFit) -> Vec<Op> {
    let ShapeFit { from, scale, shift } = fit;
    let centre = (from.x + from.w / 2.0, from.y + from.h / 2.0);
    match shape {
        Shape::Points(points) => {
            let to = Rect {
                x: centre.0 + shift.0 - from.w * scale.0 / 2.0,
                y: centre.1 + shift.1 - from.h * scale.1 / 2.0,
                w: from.w * scale.0,
                h: from.h * scale.1,
            };
            if scale == (1.0, 1.0) {
                return set_points(id, points.iter().map(|p| add(*p, shift)));
            }
            set_points(id, points.iter().map(|p| remap(*p, from, to)))
        }
        Shape::Path { .. } => {
            let mut ops = Vec::new();
            if scale != (1.0, 1.0) {
                ops.push(Op::TransformPathAnchors {
                    node: id.to_owned(),
                    transform: OpPathTransform::Scale {
                        sx: scale.0,
                        sy: scale.1,
                        cx: centre.0,
                        cy: centre.1,
                    },
                });
            }
            if shift != (0.0, 0.0) {
                ops.push(translate(id, shift));
            }
            ops
        }
        Shape::Line { .. } => Vec::new(),
    }
}

fn set_points(id: &str, points: impl Iterator<Item = Pt>) -> Vec<Op> {
    vec![Op::SetPoints {
        node: id.to_owned(),
        points: points.map(|(x, y)| OpPoint { x, y }).collect(),
    }]
}

fn translate(id: &str, (dx, dy): Pt) -> Op {
    Op::TransformPathAnchors {
        node: id.to_owned(),
        transform: OpPathTransform::Translate { dx, dy },
    }
}

/// The bounds frame of `points` before and after the grip drag. A zero
/// extent stays zero: scaling cannot grow it.
fn resized_frame(points: &[Pt], grip: Grip, drag: ShapeDrag) -> (Rect, Rect) {
    let (x, y, w, h) = bounds(points).unwrap_or_default();
    let from = Rect { x, y, w, h };
    let delta = (
        if w > 0.0 { drag.own.0 } else { 0.0 },
        if h > 0.0 { drag.own.1 } else { 0.0 },
    );
    let to = resize(
        from,
        drag.spin,
        Drag {
            grip,
            delta,
            constrain: drag.constrain && w > 0.0 && h > 0.0,
            from_center: drag.from_center,
        },
    )
    .apply(from);
    (from, to)
}

/// `p` in frame `from` moved to the same place in frame `to`.
fn remap(p: Pt, from: Rect, to: Rect) -> Pt {
    let sx = if from.w > 0.0 { to.w / from.w } else { 1.0 };
    let sy = if from.h > 0.0 { to.h / from.h } else { 1.0 };
    (to.x + (p.0 - from.x) * sx, to.y + (p.1 - from.y) * sy)
}

/// The shift that keeps undragged points in place on the page when the
/// pivot (the bounds centre) moves from that of `before` to that of
/// `after` under the rotation `spin`: `(I - R)(c - c')`. Zero when not
/// rotated.
fn pivot_shift(before: &[Pt], after: &[Pt], spin: Affine2) -> Pt {
    let centre =
        |pts: &[Pt]| bounds(pts).map_or((0.0, 0.0), |(x, y, w, h)| (x + w / 2.0, y + h / 2.0));
    let d = sub(centre(before), centre(after));
    let r = linear(linear_part(spin), d);
    sub(d, r)
}

/// `delta` adjusted so the segment from `fixed` to `moving + delta` lies
/// at a multiple of 45°, keeping its length.
fn snap_45(moving: Pt, fixed: Pt, delta: Pt) -> Pt {
    let v = sub(add(moving, delta), fixed);
    let len = v.0.hypot(v.1);
    if len == 0.0 {
        return delta;
    }
    let step = std::f64::consts::FRAC_PI_4;
    let angle = (v.1.atan2(v.0) / step).round() * step;
    let target = add(fixed, (len * angle.cos(), len * angle.sin()));
    sub(target, moving)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drag(own: Pt, spin: Affine2) -> ShapeDrag {
        ShapeDrag {
            spin,
            shift: own,
            own,
            constrain: false,
            from_center: false,
        }
    }

    fn points_of(ops: &[Op]) -> Vec<Pt> {
        if let Some(Op::SetPoints { points, .. }) = ops.first() {
            points.iter().map(|p| (p.x, p.y)).collect()
        } else {
            Vec::new()
        }
    }

    #[test]
    fn vertex_drag_keeps_other_vertices_on_the_page() {
        let points = vec![(0.0, 0.0), (100.0, 0.0), (50.0, 80.0)];
        let shape = Shape::Points(points.clone());
        let c = (50.0, 40.0);
        let spin = Affine2::rotate_at(30.0, c.0, c.1);
        let ops =
            plan_shape("p", &shape, HandleRef::Vertex(2), drag((20.0, 30.0), spin)).expect("ops");
        let next = points_of(&ops);
        let (x, y, w, h) = bounds(&next).expect("bounds");
        let spin2 = Affine2::rotate_at(30.0, x + w / 2.0, y + h / 2.0);
        for i in [0, 1] {
            let a = spin.apply(points[i].0, points[i].1);
            let b = spin2.apply(next[i].0, next[i].1);
            assert!((a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9, "{i}");
        }
        let moved = spin2.apply(next[2].0, next[2].1);
        let want = spin.apply(70.0, 110.0);
        assert!((moved.0 - want.0).abs() < 1e-9 && (moved.1 - want.1).abs() < 1e-9);
    }

    #[test]
    fn grip_on_points_scales_into_the_new_frame() {
        let shape = Shape::Points(vec![(0.0, 0.0), (100.0, 0.0), (50.0, 50.0)]);
        let ops = plan_shape(
            "p",
            &shape,
            HandleRef::Grip(Grip::Se),
            drag((100.0, 50.0), Affine2::IDENTITY),
        )
        .expect("ops");
        assert_eq!(
            points_of(&ops),
            vec![(0.0, 0.0), (200.0, 0.0), (100.0, 100.0)]
        );
        let err = plan_shape(
            "p",
            &shape,
            HandleRef::Vertex(9),
            drag((1.0, 1.0), Affine2::IDENTITY),
        )
        .expect_err("missing vertex");
        assert_eq!(err.code, "editor.unknown_handle");
    }

    #[test]
    fn constrained_endpoint_snaps_to_45_degrees() {
        let d = snap_45((10.0, 0.0), (0.0, 0.0), (0.0, 9.0));
        let end = add((10.0, 0.0), d);
        assert!((end.0 - end.1).abs() < 1e-9, "{end:?}");
    }
}
