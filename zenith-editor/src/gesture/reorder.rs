//! Dragging an in-flow child of a row / column / grid frame to another
//! flow slot: `reparent` into the same frame at the new position.

use std::collections::BTreeMap;

use zenith_core::Document;
use zenith_scene::CompiledBox;
use zenith_tx::{FlowPlacement, Op, Position, layout_flow};

use crate::doc::tree::Located;
use crate::geom::Pt;

/// The op that moves node `id` to the flow slot nearest its centre moved
/// by `shift` (authored px), or `None` when the slot does not change.
///
/// The other in-flow siblings keep their order. A row compares centres on
/// x, a column on y, a grid in reading order (rows by y band, then x).
/// `prefix` is what the compiled ids of siblings start with (`<page>/` for
/// master content).
pub(crate) fn plan_reorder(
    doc: &Document,
    located: &Located<'_>,
    id: &str,
    flow: &FlowPlacement,
    boxes: &BTreeMap<String, CompiledBox>,
    prefix: &str,
    shift: Pt,
) -> Option<Op> {
    let centre = |b: &CompiledBox| {
        (
            b.local.x + b.local.w / 2.0,
            b.local.y + b.local.h / 2.0,
            b.local.h,
        )
    };
    let own = boxes.get(&format!("{prefix}{id}")).map(centre)?;
    let moved = (own.0 + shift.0, own.1 + shift.1);
    let mut rank = 0;
    let mut others: Vec<(&str, (f64, f64, f64))> = Vec::new();
    for sibling in located.siblings {
        let Some(sid) = sibling.id() else {
            continue;
        };
        if sid == id {
            rank = others.len();
            continue;
        }
        if layout_flow(doc, sid).is_none() {
            continue;
        }
        if let Some(c) = boxes.get(&format!("{prefix}{sid}")).map(centre) {
            others.push((sid, c));
        }
    }
    let before = |c: (f64, f64, f64)| match flow.mode {
        "row" => c.0 < moved.0,
        "column" => c.1 < moved.1,
        _ => c.1 + c.2 / 2.0 < moved.1 || ((c.1 - moved.1).abs() <= c.2 / 2.0 && c.0 < moved.0),
    };
    let slot = others.iter().filter(|(_, c)| before(*c)).count();
    if slot == rank {
        return None;
    }
    let position = match slot.checked_sub(1).and_then(|i| others.get(i)) {
        Some((after, _)) => Position::After {
            id: (*after).to_owned(),
        },
        None => Position::Before {
            id: others.first().map(|(s, _)| (*s).to_owned())?,
        },
    };
    Some(Op::Reparent {
        node: id.to_owned(),
        new_parent: flow.frame.clone(),
        position,
    })
}
