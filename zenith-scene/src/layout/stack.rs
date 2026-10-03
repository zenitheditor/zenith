//! `row` and `column` frames: size flow children along both axes, break
//! wrapped lines, then justify and align them.
//!
//! Widths resolve before heights, because a text height depends on its width.
//! A row resolves its main axis first; a column resolves its cross axis first.

use zenith_core::Span;

use crate::compile::ProbeAt;

use super::diag::{Sink, child_overflow, fill_in_hug_parent, unsized_child};
use super::flex::{
    OVERFLOW_EPSILON, align_offset, break_lines, distribute_fill, justify_positions, span,
};
use super::measure::Engine;
use super::model::{Align, Avail, Axis, AxisSpec, FrameSpec, Justify, Sizing};
use super::solve::{Item, Slot};

/// The frame-level inputs of a stack or grid solve.
pub(super) struct StackCx<'a> {
    pub(super) engine: Engine<'a>,
    pub(super) frame_id: &'a str,
    pub(super) span: Option<Span>,
    pub(super) spec: FrameSpec,
    /// Content box width (outer width minus left and right padding).
    pub(super) inner_w: Avail,
    /// Content box height (outer height minus top and bottom padding).
    pub(super) inner_h: Avail,
    /// Top-left corner of the content box, in the frame's parent space.
    pub(super) content: (f64, f64),
    /// The frame's top-left in its parent space: the origin its children
    /// count from.
    pub(super) origin: (f64, f64),
    /// The render translation of the frame's list, when the frame is solved
    /// at its final origin (placing, or measuring at a known slot). Measure
    /// probes of position-dependent children then compile where they draw.
    pub(super) dev: Option<(f64, f64)>,
}

impl StackCx<'_> {
    /// Hug width, reporting `layout.unsized_child` when there is none.
    fn hug_w(&self, it: &Item<'_>, cap: Option<f64>, sink: &mut Sink<'_>) -> f64 {
        self.engine.hug_w(it.node, cap).unwrap_or_else(|_| {
            sink.push(unsized_child(
                it.node,
                self.frame_id,
                self.spec.mode,
                Axis::X,
            ));
            0.0
        })
    }

    /// The probe position of a child at `(x, y)` in the frame's parent space,
    /// when both it and the render translation are known. The probe is
    /// frame-local, as lowering places the child: `x - origin` under the
    /// translation `dev + origin`.
    fn probe(&self, at: Option<(f64, f64)>) -> Option<ProbeAt> {
        let ((x, y), (dx, dy)) = at.zip(self.dev)?;
        let (ox, oy) = self.origin;
        Some(ProbeAt {
            x: x - ox,
            y: y - oy,
            dx: dx + ox,
            dy: dy + oy,
        })
    }

    /// Hug height at `w` (measured at `at` when known), reporting
    /// `layout.unsized_child` when there is none.
    fn hug_h(&self, it: &Item<'_>, w: f64, at: Option<(f64, f64)>, sink: &mut Sink<'_>) -> f64 {
        self.engine
            .hug_h_at(it.node, w, self.probe(at))
            .unwrap_or_else(|_| {
                sink.push(unsized_child(
                    it.node,
                    self.frame_id,
                    self.spec.mode,
                    Axis::Y,
                ));
                0.0
            })
    }

    /// Report overflow along `axis` when `need` exceeds a fixed `have`.
    fn check_overflow(&self, axis: Axis, need: f64, have: Option<f64>, sink: &mut Sink<'_>) {
        if let Some(have) = have
            && need > have + OVERFLOW_EPSILON
        {
            sink.push(child_overflow(self.frame_id, self.span, axis, need, have));
        }
    }
}

/// Per-item sizes, built up axis by axis.
struct Sized {
    w: Vec<f64>,
    h: Vec<f64>,
    /// `true` for a main-axis `fill` item that takes a share of free space.
    grow: Vec<bool>,
    /// `true` when the item's width is its own hug size (no clamp, no share).
    hug_w: Vec<bool>,
    /// `true` when the item's height is its own hug size (no clamp, no share).
    hug_h: Vec<bool>,
}

impl Sized {
    fn with_capacity(n: usize) -> Self {
        Self {
            w: Vec::with_capacity(n),
            h: Vec::with_capacity(n),
            grow: Vec::with_capacity(n),
            hug_w: Vec::with_capacity(n),
            hug_h: Vec::with_capacity(n),
        }
    }
}

/// A hug size clamped into `spec`, and whether the clamp left it unchanged.
fn clamp_hug(spec: AxisSpec, raw: f64) -> (f64, bool) {
    let v = spec.clamp(raw);
    (v, v == raw)
}

/// The main-axis size before free space is shared: fixed and hug sizes, or
/// `min` for a growing `fill` item. A `fill` on a hugging main axis hugs and
/// reports `layout.fill_in_hug_parent`.
///
/// Returns `(size, grows, hugs)`.
fn main_base(
    cx: &StackCx<'_>,
    it: &Item<'_>,
    spec: AxisSpec,
    axis: Axis,
    hug: impl FnOnce(&mut Sink<'_>) -> f64,
    sink: &mut Sink<'_>,
) -> (f64, bool, bool) {
    let main_def = match axis {
        Axis::X => cx.inner_w.definite(),
        Axis::Y => cx.inner_h.definite(),
    };
    match spec.sizing {
        Sizing::Fixed(v) => (spec.clamp(v), false, false),
        Sizing::Hug => {
            let (v, hugs) = clamp_hug(spec, hug(sink));
            (v, false, hugs)
        }
        Sizing::Fill if main_def.is_some() => (spec.clamp(0.0), true, false),
        Sizing::Fill => {
            sink.push(fill_in_hug_parent(it.node, cx.frame_id, axis));
            let (v, hugs) = clamp_hug(spec, hug(sink));
            (v, false, hugs)
        }
    }
}

/// Share each line's free main-axis space over its growing items.
fn grow_lines(
    lines: &[(usize, usize)],
    items: &[Item<'_>],
    sized: &mut Sized,
    row: bool,
    cx: &StackCx<'_>,
) {
    let limit = if row {
        cx.inner_w.definite()
    } else {
        cx.inner_h.definite()
    };
    let Some(limit) = limit else {
        return;
    };
    let gap = cx.spec.gap;
    for &(start, end) in lines {
        let main = if row { &mut sized.w } else { &mut sized.h };
        let (Some(line), Some(grow), Some(line_items)) = (
            main.get_mut(start..end),
            sized.grow.get(start..end),
            items.get(start..end),
        ) else {
            continue;
        };
        let fixed: f64 = line
            .iter()
            .zip(grow)
            .filter(|(_, g)| !**g)
            .map(|(s, _)| *s)
            .sum();
        let gaps = gap * (line.len().saturating_sub(1)) as f64;
        let fills: Vec<(f64, f64)> = line_items
            .iter()
            .zip(grow)
            .filter(|(_, g)| **g)
            .map(|(it, _)| {
                let spec = if row { it.w } else { it.h };
                (spec.min, spec.max)
            })
            .collect();
        let mut shares = distribute_fill(limit - fixed - gaps, &fills).into_iter();
        for (size, g) in line.iter_mut().zip(grow) {
            if *g && let Some(share) = shares.next() {
                *size = share;
            }
        }
    }
}

/// Place the sized items line by line. Returns the used main and cross
/// extents and the slots.
fn place(
    cx: &StackCx<'_>,
    items: &[Item<'_>],
    sized: &Sized,
    lines: &[(usize, usize)],
    line_cross: &[f64],
    row: bool,
    sink: &mut Sink<'_>,
) -> (f64, f64, Vec<Slot>) {
    let spec = cx.spec;
    let (main_start, cross_start) = if row {
        cx.content
    } else {
        (cx.content.1, cx.content.0)
    };
    let (main_def, cross_def) = if row {
        (cx.inner_w.definite(), cx.inner_h.definite())
    } else {
        (cx.inner_h.definite(), cx.inner_w.definite())
    };
    let (main_sizes, cross_sizes) = if row {
        (&sized.w, &sized.h)
    } else {
        (&sized.h, &sized.w)
    };
    let mut slots = Vec::with_capacity(items.len());
    let mut used_main: f64 = 0.0;
    let mut used_cross = 0.0;
    let mut need_cross = 0.0;
    let mut cross_cursor = cross_start;
    let last = lines.len().saturating_sub(1);
    for (li, (&(start, end), &line)) in lines.iter().zip(line_cross).enumerate() {
        let (Some(mains), Some(crosses), Some(line_items)) = (
            main_sizes.get(start..end),
            cross_sizes.get(start..end),
            items.get(start..end),
        ) else {
            continue;
        };
        used_main = used_main.max(span(mains, spec.gap));
        let positions = justify_positions(main_start, mains, spec.gap, main_def, spec.justify);
        let mut line_need = line;
        for (k, ((pos, (m, c)), it)) in positions
            .iter()
            .zip(mains.iter().zip(crosses))
            .zip(line_items)
            .enumerate()
        {
            line_need = line_need.max(*c);
            let offset = align_offset(*c, line, spec.align);
            let cross_pos = if offset == 0.0 {
                cross_cursor
            } else {
                cross_cursor + offset
            };
            let (x, y, w, h) = if row {
                (*pos, cross_pos, *m, *c)
            } else {
                (cross_pos, *pos, *c, *m)
            };
            slots.push(Slot {
                index: it.index,
                x,
                y,
                w,
                h,
                hug_w: sized.hug_w.get(start + k).copied().unwrap_or(false),
                hug_h: sized.hug_h.get(start + k).copied().unwrap_or(false),
            });
        }
        used_cross += line;
        need_cross += line_need;
        cross_cursor += line;
        if li != last {
            used_cross += spec.wrap_gap;
            need_cross += spec.wrap_gap;
            cross_cursor += spec.wrap_gap;
        }
    }
    let (main_axis, cross_axis) = if row {
        (Axis::X, Axis::Y)
    } else {
        (Axis::Y, Axis::X)
    };
    cx.check_overflow(main_axis, used_main, main_def, sink);
    cx.check_overflow(cross_axis, need_cross, cross_def, sink);
    if row {
        (used_main, used_cross, slots)
    } else {
        (used_cross, used_main, slots)
    }
}

/// The lines of a stack: greedy breaks when it wraps on a fixed main axis,
/// else one line holding every item.
fn lines_of(main: &[f64], wraps: Option<f64>, gap: f64) -> Vec<(usize, usize)> {
    match wraps {
        Some(limit) => break_lines(main, gap, limit),
        None if main.is_empty() => Vec::new(),
        None => vec![(0, main.len())],
    }
}

/// A line's cross size: the fixed content cross size for a single line,
/// else the largest item.
fn line_cross_size(sizes: &[f64], single: bool, cross_def: Option<f64>) -> f64 {
    match (single, cross_def) {
        (true, Some(c)) => c,
        _ => sizes.iter().copied().fold(0.0, f64::max),
    }
}

/// Lay out a `row` frame. Returns `(used_w, used_h, slots)`.
pub(super) fn row(
    cx: &StackCx<'_>,
    items: &[Item<'_>],
    sink: &mut Sink<'_>,
) -> (f64, f64, Vec<Slot>) {
    let cap = cx.inner_w.cap();
    let mut sized = Sized::with_capacity(items.len());
    for it in items {
        let (w, grow, hugs) = main_base(cx, it, it.w, Axis::X, |s| cx.hug_w(it, cap, s), sink);
        sized.w.push(w);
        sized.grow.push(grow);
        sized.hug_w.push(hugs);
    }
    let wraps = cx.inner_w.definite().filter(|_| cx.spec.wrap);
    let lines = lines_of(&sized.w, wraps, cx.spec.gap);
    grow_lines(&lines, items, &mut sized, true, cx);

    // Heights: fixed and hug first; a `fill` height stretches to its line.
    // On one top-aligned line every item's position is known already.
    let top_aligned = matches!(cx.spec.align, Align::Start | Align::Stretch);
    let xs = (lines.len() == 1 && top_aligned).then(|| {
        justify_positions(
            cx.content.0,
            &sized.w,
            cx.spec.gap,
            cx.inner_w.definite(),
            cx.spec.justify,
        )
    });
    for (k, (it, w)) in items.iter().zip(&sized.w).enumerate() {
        let at = xs
            .as_ref()
            .and_then(|xs| xs.get(k))
            .map(|x| (*x, cx.content.1));
        let (h, hugs) = match it.h.sizing {
            Sizing::Fixed(v) => (it.h.clamp(v), false),
            Sizing::Hug => clamp_hug(it.h, cx.hug_h(it, *w, at, sink)),
            Sizing::Fill => (
                it.h.clamp(cx.engine.hug_h_at(it.node, *w, cx.probe(at)).unwrap_or(0.0)),
                false,
            ),
        };
        sized.h.push(h);
        sized.hug_h.push(hugs);
    }
    let single = lines.len() == 1;
    let mut line_cross = Vec::with_capacity(lines.len());
    for &(start, end) in &lines {
        let size = line_cross_size(
            sized.h.get(start..end).unwrap_or(&[]),
            single,
            cx.inner_h.definite(),
        );
        line_cross.push(size);
        for (it, h) in items
            .get(start..end)
            .unwrap_or(&[])
            .iter()
            .zip(sized.h.get_mut(start..end).unwrap_or(&mut []))
        {
            if it.h.sizing == Sizing::Fill {
                *h = it.h.clamp(size);
            }
        }
    }
    place(cx, items, &sized, &lines, &line_cross, true, sink)
}

/// Lay out a `column` frame. Returns `(used_w, used_h, slots)`.
pub(super) fn column(
    cx: &StackCx<'_>,
    items: &[Item<'_>],
    sink: &mut Sink<'_>,
) -> (f64, f64, Vec<Slot>) {
    let cap = cx.inner_w.cap();
    let cross_def = cx.inner_w.definite();
    let wraps = cx.inner_h.definite().filter(|_| cx.spec.wrap);
    let mut sized = Sized::with_capacity(items.len());

    // Widths before line breaks: fixed, hug, or a `fill` item's own width.
    for it in items {
        let (w, hugs) = match it.w.sizing {
            Sizing::Fixed(v) => (it.w.clamp(v), false),
            Sizing::Hug => clamp_hug(it.w, cx.hug_w(it, cap, sink)),
            Sizing::Fill => (
                it.w.clamp(cx.engine.hug_w(it.node, cap).unwrap_or(0.0)),
                false,
            ),
        };
        sized.w.push(w);
        sized.hug_w.push(hugs);
    }
    // Without wrapping there is one line: `fill` widths stretch to it now.
    let mut line_cross: Vec<f64> = Vec::new();
    if wraps.is_none() && !items.is_empty() {
        let size = line_cross_size(&sized.w, true, cross_def);
        line_cross.push(size);
        for (it, w) in items.iter().zip(sized.w.iter_mut()) {
            if it.w.sizing == Sizing::Fill {
                *w = it.w.clamp(size);
            }
        }
    }

    // Heights in order. On one line packed from the top, each item's
    // position follows from the heights before it (as `place` computes it),
    // until a growing item, whose height is not known yet.
    let packed =
        wraps.is_none() && (cx.spec.justify == Justify::Start || cx.inner_h.definite().is_none());
    let mut cursor = packed.then_some(cx.content.1);
    for (it, w) in items.iter().zip(&sized.w) {
        let at = cursor.zip(line_cross.first()).map(|(y, line)| {
            let offset = align_offset(*w, *line, cx.spec.align);
            let x = if offset == 0.0 {
                cx.content.0
            } else {
                cx.content.0 + offset
            };
            (x, y)
        });
        let (h, grow, hugs) = main_base(cx, it, it.h, Axis::Y, |s| cx.hug_h(it, *w, at, s), sink);
        cursor = match cursor {
            Some(y) if !grow => Some(y + h + cx.spec.gap),
            Some(_) | None => None,
        };
        sized.h.push(h);
        sized.grow.push(grow);
        sized.hug_h.push(hugs);
    }
    let lines = lines_of(&sized.h, wraps, cx.spec.gap);

    // Wrapped columns: each line is as wide as its widest item; `fill` widths
    // stretch to it and hugging heights re-measure at the new width.
    if wraps.is_some() {
        let single = lines.len() == 1;
        for &(start, end) in &lines {
            let size = line_cross_size(sized.w.get(start..end).unwrap_or(&[]), single, cross_def);
            line_cross.push(size);
            for (((it, w), h), hugs) in items
                .get(start..end)
                .unwrap_or(&[])
                .iter()
                .zip(sized.w.get_mut(start..end).unwrap_or(&mut []))
                .zip(sized.h.get_mut(start..end).unwrap_or(&mut []))
                .zip(sized.hug_h.get_mut(start..end).unwrap_or(&mut []))
            {
                if it.w.sizing != Sizing::Fill {
                    continue;
                }
                *w = it.w.clamp(size);
                if it.h.sizing == Sizing::Hug {
                    (*h, *hugs) = clamp_hug(it.h, cx.engine.hug_h(it.node, *w).unwrap_or(0.0));
                }
            }
        }
    }
    grow_lines(&lines, items, &mut sized, false, cx);
    place(cx, items, &sized, &lines, &line_cross, false, sink)
}

#[cfg(test)]
mod tests {
    use super::super::model::Align;
    use super::*;

    #[test]
    fn single_line_takes_fixed_cross() {
        assert_eq!(line_cross_size(&[10.0, 30.0], true, Some(50.0)), 50.0);
        assert_eq!(line_cross_size(&[10.0, 30.0], true, None), 30.0);
        assert_eq!(line_cross_size(&[10.0, 30.0], false, Some(50.0)), 30.0);
    }

    #[test]
    fn lines_without_wrap_hold_every_item() {
        assert_eq!(lines_of(&[1.0, 2.0], None, 0.0), vec![(0, 2)]);
        assert!(lines_of(&[], None, 0.0).is_empty());
        assert_eq!(
            lines_of(&[60.0, 60.0], Some(100.0), 0.0),
            vec![(0, 1), (1, 2)]
        );
    }

    #[test]
    fn align_stretch_does_not_offset() {
        assert_eq!(align_offset(5.0, 20.0, Align::Stretch), 0.0);
    }
}
