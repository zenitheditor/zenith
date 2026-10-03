//! Solve one layout frame: resolve its settings and its flow children, then
//! size and place them (stack modes in [`super::stack`], grid here).

use zenith_core::{FrameNode, Node, PropertyValue};

use super::diag::{Sink, conflicting_min_max};
use super::measure::Engine;
use super::model::{
    Align, Avail, Axis, AxisSpec, ChildRole, FrameSpec, ItemView, Mode, Sizing, axis_sizing,
    child_role, px_of,
};
use super::stack::{StackCx, column, row};

/// One placed flow child: its index in the frame's children and its box in
/// px, in the coordinate space of the frame's own `x` / `y`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Slot {
    pub(super) index: usize,
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) w: f64,
    pub(super) h: f64,
    /// `true` when the width is the child's own hug size.
    pub(super) hug_w: bool,
    /// `true` when the height is the child's own hug size (a text box then
    /// equals its content).
    pub(super) hug_h: bool,
}

/// The solved frame: its outer size and its flow children's slots.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Solution {
    pub(super) w: f64,
    pub(super) h: f64,
    pub(super) slots: Vec<Slot>,
}

/// A flow child with its two resolved axes.
#[derive(Clone, Copy)]
pub(super) struct Item<'n> {
    pub(super) index: usize,
    pub(super) node: &'n Node,
    pub(super) w: AxisSpec,
    pub(super) h: AxisSpec,
}

/// Where a frame is solved: its top-left, and the render translation of its
/// list when `origin` is its final position (`None` while measuring at an
/// unknown position).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct At {
    pub(super) origin: (f64, f64),
    pub(super) dev: Option<(f64, f64)>,
}

impl At {
    /// A measuring solve at an unknown position.
    pub(super) const UNPLACED: At = At {
        origin: (0.0, 0.0),
        dev: None,
    };
}

/// Solve `frame` placed at `at` and offered `aw` × `ah`.
///
/// A fixed axis keeps its size. A hugging axis takes the content size plus
/// the padding. Diagnostics go to `sink`.
pub(super) fn solve(
    engine: Engine<'_>,
    frame: &FrameNode,
    at: At,
    aw: Avail,
    ah: Avail,
    sink: &mut Sink<'_>,
) -> Solution {
    let origin = at.origin;
    let Some(mode) = Mode::of(frame) else {
        let (w, h) = engine.extents(&frame.children);
        return Solution {
            w: aw.definite().unwrap_or(w),
            h: ah.definite().unwrap_or(h),
            slots: Vec::new(),
        };
    };
    let spec = FrameSpec::of(frame, mode, engine.resolved(), engine.env.style_map());
    let ins = spec.insets;
    let cx = StackCx {
        engine,
        frame_id: &frame.id,
        span: frame.source_span,
        spec,
        inner_w: aw.inset(ins.left + ins.right),
        inner_h: ah.inset(ins.top + ins.bottom),
        content: (origin.0 + ins.left, origin.1 + ins.top),
        origin,
        dev: at.dev,
    };
    let items = flow_items(engine, frame, &spec, sink);
    let (used_w, used_h, slots) = match mode {
        Mode::Row => row(&cx, &items, sink),
        Mode::Column => column(&cx, &items, sink),
        Mode::Grid => grid(&cx, frame, &items),
    };
    Solution {
        w: aw.definite().unwrap_or(used_w + (ins.left + ins.right)),
        h: ah.definite().unwrap_or(used_h + (ins.top + ins.bottom)),
        slots,
    }
}

/// The flow children of `frame` in source order, with resolved axes.
fn flow_items<'n>(
    engine: Engine<'_>,
    frame: &'n FrameNode,
    spec: &FrameSpec,
    sink: &mut Sink<'_>,
) -> Vec<Item<'n>> {
    let row_main = spec.mode == Mode::Row;
    let stretch = spec.align == Align::Stretch;
    let mut items = Vec::new();
    for (index, node) in frame.children.iter().enumerate() {
        if child_role(node) != ChildRole::Flow {
            continue;
        }
        let Some(view) = ItemView::of(node, engine.resolved()) else {
            continue;
        };
        let li = view.item;
        let w = AxisSpec {
            sizing: axis_sizing(view.w, li.w_keyword, row_main, stretch),
            min: 0.0,
            max: f64::INFINITY,
        };
        let h = AxisSpec {
            sizing: axis_sizing(view.h, li.h_keyword, !row_main, stretch),
            min: 0.0,
            max: f64::INFINITY,
        };
        let w = with_clamp(engine, node, w, Axis::X, (&li.min_w, &li.max_w), sink);
        let h = with_clamp(engine, node, h, Axis::Y, (&li.min_h, &li.max_h), sink);
        items.push(Item { index, node, w, h });
    }
    items
}

/// `spec` with the resolved `min-*` / `max-*` of one axis.
///
/// A token-backed `min` above its `max` reports `layout.conflicting_size`
/// (validation reports two px literals already).
fn with_clamp(
    engine: Engine<'_>,
    node: &Node,
    spec: AxisSpec,
    axis: Axis,
    (min_pv, max_pv): (&Option<PropertyValue>, &Option<PropertyValue>),
    sink: &mut Sink<'_>,
) -> AxisSpec {
    let min = px_of(min_pv.as_ref(), engine.resolved())
        .unwrap_or(0.0)
        .max(0.0);
    let max = px_of(max_pv.as_ref(), engine.resolved()).unwrap_or(f64::INFINITY);
    let token = |pv: &Option<PropertyValue>| matches!(pv, Some(PropertyValue::TokenRef(_)));
    if min > max && (token(min_pv) || token(max_pv)) {
        sink.push(conflicting_min_max(node, axis, min, max));
    }
    AxisSpec { min, max, ..spec }
}

/// Grid: uniform cells, row-major, `gap` gutters.
///
/// `columns` defaults to 1. `rows` defaults to `ceil(n / columns)`. A fixed
/// content box divides evenly. A hugging axis sizes every cell to the largest
/// child. Every child fills its cell box.
fn grid(cx: &StackCx<'_>, frame: &FrameNode, items: &[Item<'_>]) -> (f64, f64, Vec<Slot>) {
    let gap = cx.spec.gap;
    let n = items.len();
    let cols = frame.columns.unwrap_or(1).max(1) as usize;
    let rows = frame
        .rows
        .map(|r| r.max(1) as usize)
        .unwrap_or_else(|| n.div_ceil(cols).max(1));
    let col_w = match cx.inner_w {
        Avail::Definite(content_w) => {
            ((content_w - (cols - 1) as f64 * gap) / cols as f64).max(0.0)
        }
        Avail::Hug(_) => items
            .iter()
            .map(|it| fixed_or(it.w, || cx.engine.hug_w(it.node, None).unwrap_or(0.0)))
            .fold(0.0, f64::max),
    };
    let row_h = match cx.inner_h {
        Avail::Definite(content_h) => {
            ((content_h - (rows - 1) as f64 * gap) / rows as f64).max(0.0)
        }
        Avail::Hug(_) => items
            .iter()
            .map(|it| fixed_or(it.h, || cx.engine.hug_h(it.node, col_w).unwrap_or(0.0)))
            .fold(0.0, f64::max),
    };
    let (content_left, content_top) = cx.content;
    let slots = items
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let col = i % cols;
            let row = i / cols;
            Slot {
                index: it.index,
                x: content_left + col as f64 * (col_w + gap),
                y: content_top + row as f64 * (row_h + gap),
                w: col_w,
                h: row_h,
                hug_w: false,
                hug_h: false,
            }
        })
        .collect();
    let used_w = cols as f64 * col_w + (cols - 1) as f64 * gap;
    let used_h = rows as f64 * row_h + (rows - 1) as f64 * gap;
    (used_w, used_h, slots)
}

/// The fixed size of `spec`, else `hug()`.
fn fixed_or(spec: AxisSpec, hug: impl FnOnce() -> f64) -> f64 {
    match spec.sizing {
        Sizing::Fixed(v) => v,
        Sizing::Hug | Sizing::Fill => hug(),
    }
}
