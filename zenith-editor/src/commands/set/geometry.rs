//! `node.set` box fields: `x` / `y` / `w` / `h` mapped axis by axis as a
//! gesture is.

use serde_json::Value;
use zenith_core::Diagnostic;
use zenith_tx::Op;

use super::params::SetParams;
use crate::ctx::Ctx;
use crate::edit::offers::for_rejection;
use crate::edit::ops::rejected;
use crate::error::EditorError;
use crate::gesture::boxplan::{BoxInput, BoxPlan, Motion, plan_box};
use crate::gesture::current_axes;
use crate::gesture::facts::{Axis, box_facts};
use crate::gesture::kind::Kind;
use crate::gesture::target::resolve;
use crate::wire::DiagnosticOut;

/// A difference below this many px or degrees is no change.
pub(super) const EPSILON: f64 = 1e-9;

/// The ops that move the box of `id` to the `x` / `y` / `w` / `h` of `p`.
pub(super) fn box_ops(
    ctx: &mut Ctx<'_, '_>,
    doc: &zenith_core::Document,
    id: &str,
    p: &SetParams,
    raw: &Value,
) -> Result<(Vec<Op>, Vec<DiagnosticOut>), EditorError> {
    let t = resolve(ctx, doc, id)?;
    let node = t.located.node;
    let unsupported = || {
        EditorError::new(
            "editor.unsupported",
            format!(
                "{} '{id}' has no x / y / w / h box; drag its points on the canvas or edit the \
                 code",
                node.kind_str()
            ),
        )
    };
    if Kind::of(node) != Kind::Box {
        return Err(unsupported());
    }
    let facts = box_facts(doc, node, id).ok_or_else(unsupported)?;
    let current = current_axes(&facts, &t);
    let mut deltas = [
        (Axis::X, 0.0),
        (Axis::Y, 0.0),
        (Axis::W, 0.0),
        (Axis::H, 0.0),
    ];
    for ((axis, delta), (_, now)) in deltas.iter_mut().zip(current) {
        let wanted = match axis {
            Axis::X => p.x,
            Axis::Y => p.y,
            Axis::W => p.w,
            Axis::H => p.h,
        };
        let Some(wanted) = wanted else { continue };
        let Some(now) = now else {
            return Err(EditorError::new(
                "editor.origin_unresolved",
                format!(
                    "the current px {} of '{id}' does not resolve, because a container offset \
                     is not px; write it in the code",
                    axis.name()
                ),
            ));
        };
        if axis.is_size() && wanted < 0.0 {
            return Err(EditorError::new(
                "editor.invalid_params",
                format!("{} must be 0 or more", axis.name()),
            ));
        }
        if (wanted - now).abs() > EPSILON {
            *delta = wanted - now;
        }
    }
    let resizing = deltas.iter().any(|(a, d)| a.is_size() && *d != 0.0);
    let input = BoxInput {
        id,
        facts: &facts,
        motion: if resizing {
            Motion::Resize
        } else {
            Motion::Move
        },
        deltas,
        current,
        flags: p.flags(),
    };
    match plan_box(&input) {
        BoxPlan::Ops { ops, notes } => Ok((ops, notes)),
        BoxPlan::Rejected(diagnostics) => Err(reject(ctx, raw, diagnostics)),
        // `node.set` never sets the reorder flag.
        BoxPlan::Reorder => Ok((Vec::new(), Vec::new())),
    }
}

/// `editor.rejected` with the offers that resend `node.set` with a flag.
pub(super) fn reject(ctx: &Ctx<'_, '_>, raw: &Value, diagnostics: Vec<Diagnostic>) -> EditorError {
    let offers = for_rejection(ctx.command, raw, &diagnostics, true);
    rejected(ctx.command, &diagnostics, &ctx.session.text, offers)
}
