//! `node.handles {id?, rotate_offset?}` for one node: where to draw its
//! selection handles, and which gestures the plain drag cannot do.

use serde::Serialize;
use serde_json::Value;
use zenith_scene::Affine2;
use zenith_tx::OpPathHandle;

use super::blocks::{Blocked, Handle, all_actions, box_blocks, first_block, grip_block};
use crate::commands::common::target;
use crate::ctx::Ctx;
use crate::doc::place::pivot_follows_content;
use crate::doc::shape::{Shape, bounds, shape_of};
use crate::error::EditorError;
use crate::geom::{Grip, Pt, Rect, add, angle_deg, linear};
use crate::gesture::facts::box_facts;
use crate::gesture::flags::HandleRef;
use crate::gesture::kind::{Kind, rotate_blocked};
use crate::gesture::target::{hidden_by, locked_by, resolve};
use crate::wire::to_json;

#[derive(Debug, Serialize)]
struct Handles {
    id: String,
    kind: &'static str,
    page: usize,
    stale: bool,
    /// The drawn box (or the points / anchors frame) in page px: top-left,
    /// top-right, bottom-right, bottom-left.
    corners: [Pt; 4],
    center: Pt,
    /// The page angle of the box's x axis, degrees clockwise.
    angle: f64,
    handles: Vec<Handle>,
    disabled: Vec<Blocked>,
    /// The master that holds the node: an edit changes every page using
    /// it.
    #[serde(skip_serializing_if = "Option::is_none")]
    master: Option<String>,
    /// An ancestor group turns about its content bounds: gestures are exact
    /// only up to that pivot's shift.
    pivot_follows_content: bool,
}

/// The handles of one node (default: the one selected node) on the page it
/// draws on, from its compiled box, and the gestures a plain drag cannot
/// do with the code that would reject them. A rejected gesture can still
/// go ahead with the offer the rejection lists (detach, set size, …).
///
/// Handle sets per kind:
/// - box kinds: 8 resize grips (`nw` … `w`) and `rotate` when the kind
///   rotates;
/// - line: `start` and `end`;
/// - polygon / polyline: 8 grips on the vertex bounds, `rotate`, and each
///   vertex `p<i>`;
/// - path: 8 grips on the anchor bounds, `rotate`, each anchor `a<s>.<i>`,
///   and each complete control handle `a<s>.<i>.in` / `.out`;
/// - a polygon, polyline, or path whose bounds have no width (or height)
///   omits the grips that move only that axis (`e` / `w`, or `n` / `s`):
///   they could not stretch it.
/// - connector, footnote, light, unknown: none.
///
/// `rotate` sits `rotate_offset` page px (default 24) outside the top edge
/// midpoint; scale it by the zoom for a constant screen distance.
pub(super) fn run(
    ctx: &mut Ctx<'_, '_>,
    id: Option<String>,
    rotate_offset: f64,
) -> Result<Value, EditorError> {
    let id = target(ctx, id)?;
    let display = ctx.display()?;
    let doc = &display.doc;
    let t = resolve(ctx, doc, &id)?;
    let node = t.located.node;
    let kind = Kind::of(node);
    let mut blocked: Vec<Blocked> = Vec::new();
    let all = |code: &str, detail: String| all_actions(code, &detail);
    if display.stale {
        blocked.extend(all(
            "editor.buffer_invalid",
            "the text has errors; the canvas shows the last valid text".to_owned(),
        ));
    }
    if let Some(by) = locked_by(&t.located) {
        blocked.extend(all("editor.locked", format!("locked by '{by}'")));
    }
    if let Some(by) = hidden_by(&t.located) {
        blocked.extend(all("editor.hidden", format!("hidden by '{by}'")));
    }
    if let Some((code, why)) = rotate_blocked(node) {
        blocked.push(Blocked {
            action: "rotate",
            code: code.to_owned(),
            axes: Vec::new(),
            detail: why,
            node: None,
        });
    }
    let transform = t.bx.transform();
    let local = t.bx.local;
    let mut frame = Rect {
        x: local.x,
        y: local.y,
        w: local.w,
        h: local.h,
    };
    let mut handles: Vec<Handle> = Vec::new();
    let mut grips = false;
    match kind {
        Kind::Box => {
            grips = true;
            if let Some(facts) = box_facts(doc, node, &id) {
                blocked.extend(box_blocks(&facts));
            }
        }
        Kind::Line | Kind::Points | Kind::Path => match (shape_of(node), t.origin) {
            (Some(Ok(shape)), Some(origin)) => {
                if let Some((x, y, w, h)) = bounds(&shape.frame_points()) {
                    frame = Rect {
                        x: x + origin.0,
                        y: y + origin.1,
                        w,
                        h,
                    };
                }
                grips = kind != Kind::Line;
                if kind == Kind::Line {
                    blocked.push(Blocked {
                        action: "resize",
                        code: "editor.unsupported".to_owned(),
                        axes: Vec::new(),
                        detail: "a line resizes by its endpoints".to_owned(),
                        node: None,
                    });
                }
                handles.extend(point_handles(&shape, origin, t.bx.world, transform));
            }
            (Some(Err(why)), _) => blocked.extend(all("tx.value_unresolved", why.0)),
            (Some(Ok(_)), None) => blocked.extend(all(
                "editor.origin_unresolved",
                "a container offset is not px".to_owned(),
            )),
            (None, _) => {}
        },
        Kind::Derived => blocked.extend(all(
            "tx.derived_geometry",
            "a connector follows its targets".to_owned(),
        )),
        Kind::Fixed => blocked.extend(all(
            "editor.unsupported",
            format!("a {} has no canvas geometry to edit", node.kind_str()),
        )),
    }
    let at = |f: Pt| {
        let (x, y) = frame.at(f);
        transform.apply(x, y)
    };
    if grips {
        // A points or anchors frame with no extent on an axis cannot stretch
        // on it: the grips that move only that axis would do nothing (and
        // would sit on the points of a flat shape).
        let flat_x = kind != Kind::Box && frame.w == 0.0;
        let flat_y = kind != Kind::Box && frame.h == 0.0;
        for grip in Grip::ALL {
            let useless = (flat_y && !grip.moves_x()) || (flat_x && !grip.moves_y());
            if useless {
                continue;
            }
            let reason = grip_block(grip, &blocked);
            let (x, y) = at(grip.fractions());
            handles.push(Handle {
                id: grip.id().to_owned(),
                role: "resize",
                x,
                y,
                enabled: reason.is_none(),
                reason,
            });
        }
    }
    if node.takes_rotate() && !matches!(kind, Kind::Derived) {
        let up = linear(transform, (0.0, -1.0));
        let len = up.0.hypot(up.1);
        let top = at((0.5, 0.0));
        let n = if len > 0.0 {
            (up.0 / len, up.1 / len)
        } else {
            (0.0, -1.0)
        };
        let (x, y) = add(top, (n.0 * rotate_offset, n.1 * rotate_offset));
        let reason = first_block("rotate", &blocked);
        handles.push(Handle {
            id: "rotate".to_owned(),
            role: "rotate",
            x,
            y,
            enabled: reason.is_none(),
            reason,
        });
    }
    let edit_block = first_block("edit", &blocked);
    for h in &mut handles {
        if matches!(h.role, "endpoint" | "vertex" | "control") && edit_block.is_some() {
            h.enabled = false;
            h.reason.clone_from(&edit_block);
        }
    }
    let reply = Handles {
        id: id.clone(),
        kind: node.kind_str(),
        page: t.page.index + 1,
        stale: display.stale,
        corners: [
            at((0.0, 0.0)),
            at((1.0, 0.0)),
            at((1.0, 1.0)),
            at((0.0, 1.0)),
        ],
        center: at((0.5, 0.5)),
        angle: angle_deg(transform),
        handles,
        disabled: blocked,
        master: t.page.master.clone(),
        pivot_follows_content: pivot_follows_content(&t.located),
    };
    to_json(&reply)
}

/// Endpoint, vertex, anchor, and control handles in page px.
fn point_handles(shape: &Shape, origin: Pt, world: Affine2, transform: Affine2) -> Vec<Handle> {
    let page = |m: Affine2, p: Pt| m.apply(p.0 + origin.0, p.1 + origin.1);
    let handle = |h: HandleRef, role: &'static str, (x, y): Pt| Handle {
        id: h.id(),
        role,
        x,
        y,
        enabled: true,
        reason: None,
    };
    match shape {
        Shape::Line { start, end } => vec![
            handle(HandleRef::LineStart, "endpoint", page(world, *start)),
            handle(HandleRef::LineEnd, "endpoint", page(world, *end)),
        ],
        Shape::Points(points) => points
            .iter()
            .enumerate()
            .map(|(i, p)| handle(HandleRef::Vertex(i), "vertex", page(transform, *p)))
            .collect(),
        Shape::Path { contours, .. } => contours
            .iter()
            .enumerate()
            .flat_map(|(s, c)| c.iter().enumerate().map(move |(i, a)| (s, i, a)))
            .flat_map(|(subpath, index, a)| {
                let mut out = vec![handle(
                    HandleRef::Anchor { subpath, index },
                    "vertex",
                    page(transform, a.at),
                )];
                for (which, at) in [
                    (OpPathHandle::In, a.handle_in),
                    (OpPathHandle::Out, a.handle_out),
                ] {
                    if let Some(at) = at {
                        out.push(handle(
                            HandleRef::Control {
                                subpath,
                                index,
                                which,
                            },
                            "control",
                            page(transform, at),
                        ));
                    }
                }
                out
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grip_blocks_follow_axes() {
        let blocked = vec![Blocked {
            action: "resize",
            code: "tx.computed_size".to_owned(),
            axes: vec!["h"],
            detail: String::new(),
            node: None,
        }];
        assert_eq!(grip_block(Grip::E, &blocked), None);
        assert_eq!(
            grip_block(Grip::S, &blocked).as_deref(),
            Some("tx.computed_size")
        );
        let token_x = vec![Blocked {
            action: "move",
            code: "tx.token_bound".to_owned(),
            axes: vec!["x"],
            detail: String::new(),
            node: None,
        }];
        assert_eq!(grip_block(Grip::E, &token_x), None);
        assert_eq!(
            grip_block(Grip::W, &token_x).as_deref(),
            Some("tx.token_bound")
        );
    }
}
