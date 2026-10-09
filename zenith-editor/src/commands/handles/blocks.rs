//! What a plain drag cannot do, and the handle records `node.handles`
//! replies with.

use serde::Serialize;

use crate::geom::Grip;
use crate::gesture::facts::{Anchoring, Axis, AxisValue, BoxFacts};

/// One handle to draw.
#[derive(Debug, Serialize)]
pub(super) struct Handle {
    /// The handle id to send as `gesture.* handle`.
    pub(super) id: String,
    /// `resize`, `rotate`, `endpoint`, `vertex`, or `control`.
    pub(super) role: &'static str,
    /// Page px.
    pub(super) x: f64,
    /// Page px.
    pub(super) y: f64,
    /// `false` when the plain drag is rejected (see `disabled`).
    pub(super) enabled: bool,
    /// The `tx.*` / `editor.*` code that disables it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) reason: Option<String>,
}

/// A gesture the plain drag cannot do.
#[derive(Debug, Serialize, Clone)]
pub(super) struct Blocked {
    /// `move`, `resize`, `rotate`, or `edit` (endpoints, vertices, anchors).
    pub(super) action: &'static str,
    /// Why: for example `tx.token_bound`, `tx.computed_size`,
    /// `tx.anchored`, `tx.layout_managed`, `editor.locked`.
    pub(super) code: String,
    /// The attributes involved, when per axis.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) axes: Vec<&'static str>,
    pub(super) detail: String,
    /// The selected node that blocks it, in a selection's reply.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) node: Option<String>,
}

/// The per-axis blocks of a box node.
pub(super) fn box_blocks(facts: &BoxFacts) -> Vec<Blocked> {
    let mut out: Vec<Blocked> = Vec::new();
    let mut add = |action: &'static str, code: &str, axis: Axis, detail: String| {
        if let Some(b) = out
            .iter_mut()
            .find(|b| b.action == action && b.code == code)
        {
            b.axes.push(axis.name());
        } else {
            out.push(Blocked {
                action,
                code: code.to_owned(),
                axes: vec![axis.name()],
                detail,
                node: None,
            });
        }
    };
    if let Some(flow) = &facts.flow {
        for axis in [Axis::X, Axis::Y] {
            add(
                "move",
                "tx.layout_managed",
                axis,
                format!(
                    "{} frame '{}' places it; offers: reorder, absolute",
                    flow.mode, flow.frame
                ),
            );
        }
    }
    for axis in [Axis::X, Axis::Y, Axis::W, Axis::H] {
        let action = if axis.is_size() { "resize" } else { "move" };
        if !axis.is_size() && facts.flow.is_some() {
            continue;
        }
        match facts.value(axis) {
            AxisValue::Length => {}
            AxisValue::Token(token) => add(
                action,
                "tx.token_bound",
                axis,
                format!("bound to token '{token}'; offer: detach"),
            ),
            AxisValue::Unresolved(shown) => add(
                action,
                "tx.value_unresolved",
                axis,
                format!("{shown} has no px value; offer: replace_value"),
            ),
            AxisValue::Keyword(_) | AxisValue::Absent if axis.is_size() => add(
                action,
                "tx.computed_size",
                axis,
                "computed by layout or content; offer: set_size".to_owned(),
            ),
            AxisValue::Keyword(_) | AxisValue::Absent => {
                if facts.anchoring == Anchoring::Other {
                    add(
                        action,
                        "tx.anchored",
                        axis,
                        "placed by its anchor; offer: detach_anchor".to_owned(),
                    );
                }
            }
        }
    }
    out
}

/// Why a grip is disabled: a block on resize for an axis it changes, a
/// block on the left / top position it shifts, or a whole-node block.
pub(super) fn grip_block(grip: Grip, blocked: &[Blocked]) -> Option<String> {
    let (fx, fy) = grip.fractions();
    let needs = |b: &Blocked| {
        if b.axes.is_empty() {
            return b.action == "resize";
        }
        b.axes.iter().any(|a| match *a {
            "w" => grip.moves_x(),
            "h" => grip.moves_y(),
            "x" => fx == 0.0 && b.code != "tx.layout_managed",
            "y" => fy == 0.0 && b.code != "tx.layout_managed",
            _ => false,
        }) && (b.action == "resize" || b.action == "move")
    };
    blocked.iter().find(|b| needs(b)).map(|b| b.code.clone())
}

/// The code of the first whole-node block on `action`.
pub(super) fn first_block(action: &str, blocked: &[Blocked]) -> Option<String> {
    blocked
        .iter()
        .find(|b| b.action == action && b.axes.is_empty())
        .map(|b| b.code.clone())
}

/// The same block on every action (`move`, `resize`, `rotate`, `edit`).
pub(super) fn all_actions(code: &str, detail: &str) -> Vec<Blocked> {
    ["move", "resize", "rotate", "edit"]
        .into_iter()
        .map(|action| Blocked {
            action,
            code: code.to_owned(),
            axes: Vec::new(),
            detail: detail.to_owned(),
            node: None,
        })
        .collect()
}
