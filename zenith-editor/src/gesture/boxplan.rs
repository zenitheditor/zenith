//! Box-kind gestures (move and resize) as ops: each axis goes to the op
//! that keeps its authored form, or is rejected with the `tx.*` code the
//! engine would give and an offer to go ahead.

use zenith_core::Diagnostic;
use zenith_tx::{LayoutEdit, Op, SizeInput};

use super::facts::{Anchoring, Axis, AxisValue, BoxFacts};
use super::flags::Flags;
use crate::wire::DiagnosticOut;

/// Which gesture asks for the axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Motion {
    Move,
    Resize,
}

/// What a box gesture becomes.
#[derive(Debug)]
pub(crate) enum BoxPlan {
    /// Run these ops.
    Ops {
        ops: Vec<Op>,
        notes: Vec<DiagnosticOut>,
    },
    /// An in-flow child moved with `reorder`: the caller picks the slot.
    Reorder,
    /// Rejected: the diagnostics carry `tx.*` codes; the caller offers
    /// the flags that go ahead.
    Rejected(Vec<Diagnostic>),
}

/// The inputs of [`plan_box`].
pub(crate) struct BoxInput<'a> {
    pub(crate) id: &'a str,
    pub(crate) facts: &'a BoxFacts,
    pub(crate) motion: Motion,
    /// Authored px deltas. An exact 0 leaves the axis alone.
    pub(crate) deltas: [(Axis, f64); 4],
    /// The current resolved authored px value of each axis, when known.
    pub(crate) current: [(Axis, Option<f64>); 4],
    pub(crate) flags: Flags,
}

impl BoxInput<'_> {
    fn delta(&self, axis: Axis) -> f64 {
        self.deltas
            .iter()
            .find(|(a, _)| *a == axis)
            .map_or(0.0, |(_, d)| *d)
    }

    fn current(&self, axis: Axis) -> Option<f64> {
        self.current
            .iter()
            .find(|(a, _)| *a == axis)
            .and_then(|(_, v)| *v)
    }
}

/// Where one axis goes.
enum Route {
    Nudge,
    NudgeDetached,
    Absolute,
    Gap,
    Drop(String),
}

/// Plan a box gesture.
///
/// Per axis with a non-zero delta:
/// - px / pt value: `nudge_geometry` (unit kept);
/// - token: `nudge_geometry detach` with `detach`, else `tx.token_bound`;
/// - absent `x` / `y` of a group or instance: `nudge_geometry` (counts as
///   0); of another node with no anchor: `set_geometry` with the current
///   px plus the delta (nothing authored is lost);
/// - supplied by an edge anchor: the main axis moves the gap
///   (`nudge_anchor_gap`); the cross axis is dropped with a note;
/// - supplied by another anchor: `[detach_anchor, nudge_geometry]` with
///   `detach_anchor`, else `tx.anchored`;
/// - absent or `hug` / `fill` size: `set_geometry` with the measured px
///   plus the delta after `confirm_size`, else `tx.computed_size`;
/// - no px conversion: `set_geometry` px with `replace`, else
///   `tx.value_unresolved`;
/// - an in-flow child of a row / column / grid frame: a move needs
///   `absolute` (`set_layout position=absolute` then `set_geometry x/y`)
///   or `reorder`, else `tx.layout_managed`; a resize drops its `x` / `y`
///   shift (the frame places the node) with a note.
pub(crate) fn plan_box(input: &BoxInput<'_>) -> BoxPlan {
    let facts = input.facts;
    let flags = input.flags;
    let mut errors: Vec<Diagnostic> = Vec::new();
    let mut notes: Vec<DiagnosticOut> = Vec::new();
    let mut routes: Vec<(Axis, Route)> = Vec::new();
    let error = |code: &str, axis: Axis, message: String| {
        Diagnostic::error(
            code,
            format!("{}: {message}", axis.name()),
            None,
            Some(input.id.to_owned()),
        )
    };
    let moving = [Axis::X, Axis::Y]
        .into_iter()
        .any(|a| input.delta(a) != 0.0);
    let to_absolute = facts.flow.is_some() && input.motion == Motion::Move && flags.absolute;
    if let Some(flow) = &facts.flow
        && moving
        && input.motion == Motion::Move
        && !flags.absolute
    {
        if flags.reorder {
            return BoxPlan::Reorder;
        }
        return BoxPlan::Rejected(vec![Diagnostic::error(
            "tx.layout_managed",
            format!(
                "node '{}' is in the flow of {} frame '{}', which places it; reorder it in the \
                 layout (offer `reorder`) or take it out of the layout (offer `absolute`)",
                input.id, flow.mode, flow.frame
            ),
            None,
            Some(input.id.to_owned()),
        )]);
    }
    for axis in [Axis::X, Axis::Y, Axis::W, Axis::H] {
        let d = input.delta(axis);
        if d == 0.0 && !(to_absolute && !axis.is_size()) {
            continue;
        }
        if !axis.is_size() && facts.flow.is_some() {
            if to_absolute {
                routes.push((axis, Route::Absolute));
            } else {
                routes.push((
                    axis,
                    Route::Drop(format!(
                        "{} shift dropped: the layout frame places the node",
                        axis.name()
                    )),
                ));
            }
            continue;
        }
        let route = match facts.value(axis) {
            AxisValue::Length => Ok(Route::Nudge),
            AxisValue::Token(_) if flags.detach => Ok(Route::NudgeDetached),
            AxisValue::Token(token) => Err(error(
                "tx.token_bound",
                axis,
                format!("is bound to token '{token}'; detach it (offer `detach`) to move it"),
            )),
            AxisValue::Unresolved(_) if flags.replace => Ok(Route::Absolute),
            AxisValue::Unresolved(shown) => Err(error(
                "tx.value_unresolved",
                axis,
                format!("{shown} has no px value; replace it with px (offer `replace_value`)"),
            )),
            AxisValue::Keyword(_) | AxisValue::Absent if axis.is_size() && flags.confirm_size => {
                Ok(Route::Absolute)
            }
            AxisValue::Keyword(k) if axis.is_size() => Err(error(
                "tx.computed_size",
                axis,
                format!("is computed ({k}); set a fixed size (offer `set_size`)"),
            )),
            AxisValue::Keyword(_) | AxisValue::Absent if axis.is_size() => Err(error(
                "tx.computed_size",
                axis,
                "is computed from content; set a fixed size (offer `set_size`)".to_owned(),
            )),
            AxisValue::Keyword(_) | AxisValue::Absent => position_route(input, axis, &error),
        };
        match route {
            Ok(route) => routes.push((axis, route)),
            Err(e) => errors.push(e),
        }
    }
    if !errors.is_empty() {
        return BoxPlan::Rejected(errors);
    }
    let mut ops: Vec<Op> = Vec::new();
    if flags.detach_anchor
        && facts.anchoring != Anchoring::None
        && routes
            .iter()
            .any(|(a, r)| matches!(r, Route::Nudge) && facts.anchor_supplies(*a))
    {
        ops.push(Op::DetachAnchor {
            node: input.id.to_owned(),
        });
    }
    if to_absolute {
        ops.push(Op::SetLayout(LayoutEdit {
            node: input.id.to_owned(),
            position: Some(Some("absolute".to_owned())),
            ..LayoutEdit::default()
        }));
    }
    let mut absolute = [None, None, None, None];
    let mut missing: Vec<&'static str> = Vec::new();
    let (mut nudge, mut detached) = ([None; 4], false);
    let (mut gap_x, mut gap_y) = (None, None);
    for (axis, route) in routes {
        let d = input.delta(axis);
        let slot = match axis {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::W => 2,
            Axis::H => 3,
        };
        match route {
            Route::Nudge | Route::NudgeDetached => {
                detached |= matches!(route, Route::NudgeDetached);
                if let Some(s) = nudge.get_mut(slot) {
                    *s = Some(d);
                }
            }
            Route::Absolute => match input.current(axis) {
                Some(v) => {
                    if let Some(s) = absolute.get_mut(slot) {
                        *s = Some(v + d);
                    }
                }
                None => missing.push(axis.name()),
            },
            Route::Gap => match axis {
                Axis::X => gap_x = Some(d),
                Axis::Y | Axis::W | Axis::H => gap_y = Some(d),
            },
            Route::Drop(why) => notes.push(DiagnosticOut::advisory("editor.axis_dropped", why)),
        }
    }
    if !missing.is_empty() {
        return BoxPlan::Rejected(vec![Diagnostic::error(
            "editor.origin_unresolved",
            format!(
                "the current px position of '{}' ({}) does not resolve, because a container \
                 offset is not px; write it in the code",
                input.id,
                missing.join(", ")
            ),
            None,
            Some(input.id.to_owned()),
        )]);
    }
    let [ax, ay, aw, ah] = absolute;
    if [ax, ay, aw, ah].iter().any(Option::is_some) {
        ops.push(Op::SetGeometry {
            node: input.id.to_owned(),
            x: ax.map(Some),
            y: ay.map(Some),
            w: aw.map(|v| Some(SizeInput::Px(v))),
            h: ah.map(|v| Some(SizeInput::Px(v))),
            rotate: None,
        });
    }
    let [nx, ny, nw, nh] = nudge;
    if [nx, ny, nw, nh].iter().any(Option::is_some) {
        ops.push(Op::NudgeGeometry {
            node: input.id.to_owned(),
            dx: nx,
            dy: ny,
            dw: nw,
            dh: nh,
            detach: detached,
        });
    }
    if gap_x.is_some() || gap_y.is_some() {
        ops.push(Op::NudgeAnchorGap {
            node: input.id.to_owned(),
            dx: gap_x,
            dy: gap_y,
        });
    }
    BoxPlan::Ops { ops, notes }
}

/// The route of an absent `x` / `y`.
fn position_route(
    input: &BoxInput<'_>,
    axis: Axis,
    error: &impl Fn(&str, Axis, String) -> Diagnostic,
) -> Result<Route, Diagnostic> {
    let facts = input.facts;
    match facts.anchoring {
        Anchoring::None if facts.origin_defaults => Ok(Route::Nudge),
        Anchoring::None => Ok(Route::Absolute),
        Anchoring::Edge { .. } | Anchoring::Other if input.flags.detach_anchor => Ok(Route::Nudge),
        Anchoring::Edge { main } if main == axis => Ok(Route::Gap),
        Anchoring::Edge { .. } => Ok(Route::Drop(format!(
            "{} shift dropped: the edge anchor aligns the node on that axis",
            axis.name()
        ))),
        Anchoring::Other => Err(error(
            "tx.anchored",
            axis,
            "comes from the node's anchor; detach it (offer `detach_anchor`) to move it".to_owned(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::tree::locate;
    use crate::gesture::facts::box_facts;
    use zenith_core::{KdlAdapter, KdlSource};

    fn facts(body: &str, id: &str) -> BoxFacts {
        let src = format!(
            r#"zenith version=1 {{ document id="d" {{ page id="p" w=(px)100 h=(px)100 {{
              {body}
            }} }} }}"#
        );
        let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
        let node = locate(&doc, id).expect("node").node;
        box_facts(&doc, node, id).expect("box")
    }

    fn input<'a>(facts: &'a BoxFacts, flags: Flags) -> BoxInput<'a> {
        BoxInput {
            id: "r",
            facts,
            motion: Motion::Move,
            deltas: [
                (Axis::X, 5.0),
                (Axis::Y, 0.0),
                (Axis::W, 0.0),
                (Axis::H, 0.0),
            ],
            current: [
                (Axis::X, Some(10.0)),
                (Axis::Y, Some(0.0)),
                (Axis::W, Some(5.0)),
                (Axis::H, Some(5.0)),
            ],
            flags,
        }
    }

    #[test]
    fn unresolved_value_is_rejected_then_replaced() {
        let f = facts(r#"rect id="r" x=(pct)10 y=(px)0 w=(px)5 h=(px)5"#, "r");
        let BoxPlan::Rejected(d) = plan_box(&input(&f, Flags::default())) else {
            panic!("rejected");
        };
        assert_eq!(
            d.first().map(|d| d.code.as_str()),
            Some("tx.value_unresolved")
        );
        let flags = Flags {
            replace: true,
            ..Flags::default()
        };
        let BoxPlan::Ops { ops, .. } = plan_box(&input(&f, flags)) else {
            panic!("ops");
        };
        assert_eq!(
            ops,
            vec![Op::SetGeometry {
                node: "r".to_owned(),
                x: Some(Some(15.0)),
                y: None,
                w: None,
                h: None,
                rotate: None,
            }]
        );
    }

    #[test]
    fn exact_zero_axes_are_left_alone() {
        let f = facts(r#"rect id="r" x=(px)1 y=(token)"t" w=(px)5 h=(px)5"#, "r");
        let BoxPlan::Ops { ops, notes } = plan_box(&input(&f, Flags::default())) else {
            panic!("ops: the token-bound y does not move");
        };
        assert!(notes.is_empty());
        assert_eq!(
            ops,
            vec![Op::NudgeGeometry {
                node: "r".to_owned(),
                dx: Some(5.0),
                dy: None,
                dw: None,
                dh: None,
                detach: false,
            }]
        );
    }
}
