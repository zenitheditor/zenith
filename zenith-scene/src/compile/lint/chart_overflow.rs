//! `chart.overflow`: the ink of a chart's own strings leaves the chart box.
//!
//! The chart `h` and `w` hold every chart string: title, caption, axis and
//! category labels, legend, and value labels. A box too small for its bands
//! clamps the plot to zero, and the strings spill out of the box. The check
//! unions the glyph ink of each chart's strings and compares it with the
//! chart's compiled box, with a 0.5px tolerance per side. It covers every
//! chart kind.
//!
//! A chart that is invisible, `role="decoration"` / `"background"` (own or
//! inherited), or drawn under an unmodeled ancestor is skipped. The fix sets
//! `h` (else `w`) to the size that holds the ink, when that side is a
//! literal `(px)` value on a chart of the authored document.

use std::collections::BTreeMap;

use zenith_core::Diagnostic;

use crate::layout::LayoutBox;

use super::arrange::set_px;
use super::geom::{bounds, intersect, union};
use super::ledger::{Entry, PageLedger, TextItem, TextSource};
use super::overlap::{MIN_OVERLAP_AREA, glyph_overlap};
use super::paint::Authored;

/// Ink tolerance in px on each side.
const EPSILON: f64 = 0.5;

/// How far ink passes each side of a box, in px; `0` inside the tolerance.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Spill {
    top: f64,
    right: f64,
    bottom: f64,
    left: f64,
}

impl Spill {
    fn of(rect: LayoutBox, ink: LayoutBox) -> Self {
        let past = |d: f64| if d > EPSILON { d } else { 0.0 };
        Spill {
            top: past(rect.y - ink.y),
            right: past(ink.x + ink.w - (rect.x + rect.w)),
            bottom: past(ink.y + ink.h - (rect.y + rect.h)),
            left: past(rect.x - ink.x),
        }
    }

    fn vertical(self) -> f64 {
        self.top + self.bottom
    }

    fn horizontal(self) -> f64 {
        self.left + self.right
    }

    /// `top 6px, bottom 12px`: every side the ink passes.
    fn sides(self) -> String {
        [
            ("top", self.top),
            ("right", self.right),
            ("bottom", self.bottom),
            ("left", self.left),
        ]
        .into_iter()
        .filter(|(_, d)| *d > 0.0)
        .map(|(side, d)| format!("{side} {:.0}px", d.ceil()))
        .collect::<Vec<_>>()
        .join(", ")
    }
}

/// Every `chart.overflow` of the page: either chart string ink leaves the
/// compiled chart box, or the chart's own labels collide with each other
/// inside a squashed plot box.
pub(super) fn chart_overflow(
    ledger: &PageLedger,
    authored: &BTreeMap<String, Authored>,
) -> Vec<Diagnostic> {
    let mut inks: BTreeMap<usize, LayoutBox> = BTreeMap::new();
    let mut items_by_chart: BTreeMap<usize, Vec<&TextItem>> = BTreeMap::new();
    for item in ledger.texts.iter() {
        if !matches!(item.source, TextSource::Chart(_)) {
            continue;
        }
        items_by_chart.entry(item.entry).or_default().push(item);
        let Some(ink) = bounds(&item.ink.local) else {
            continue;
        };
        inks.entry(item.entry)
            .and_modify(|b| *b = union(*b, ink))
            .or_insert(ink);
    }
    let mut out = Vec::new();
    for (index, ink) in inks {
        let Some(entry) = ledger.entry(index) else {
            continue;
        };
        let Some(b) = entry.compiled else {
            continue;
        };
        if !entry.visible || entry.exempt || entry.unmodeled {
            continue;
        }
        let spill = Spill::of(b.rect, ink);
        if spill != Spill::default() {
            out.push(diagnostic(entry, b.rect, spill, authored));
            continue;
        }
        if let Some(chart_items) = items_by_chart.get(&index)
            && let Some(collision) = own_label_collision(chart_items)
        {
            out.push(collision_diagnostic(entry, b.rect, collision, authored));
        }
    }
    out
}

/// Summary of colliding own-label pairs inside one chart.
struct LabelCollision {
    pairs: usize,
    extra_h: f64,
    extra_w: f64,
}

/// Detect glyph-level collisions between distinct labels belonging to the same chart.
fn own_label_collision(items: &[&TextItem]) -> Option<LabelCollision> {
    let checked: Vec<&TextItem> = items
        .iter()
        .copied()
        .filter(|item| item.ink.axis_aligned && !item.ink.glyphs.is_empty())
        .collect();
    let mut pairs = 0usize;
    let mut extra_h = 0.0_f64;
    let mut extra_w = 0.0_f64;
    for (i, a) in checked.iter().enumerate() {
        for b in checked.iter().skip(i + 1) {
            if intersect(a.bounds, b.bounds).is_none() {
                continue;
            }
            if let Some((overlap_area, hull)) = glyph_overlap(a, b)
                && overlap_area > MIN_OVERLAP_AREA
            {
                pairs += 1;
                // Stacked labels (similar x, close y) need vertical room; side-by-side need width.
                let dx = ((a.bounds.x + a.bounds.w * 0.5) - (b.bounds.x + b.bounds.w * 0.5)).abs();
                let dy = ((a.bounds.y + a.bounds.h * 0.5) - (b.bounds.y + b.bounds.h * 0.5)).abs();
                if dx <= dy || hull.h >= hull.w {
                    extra_h += hull.h + 4.0;
                } else {
                    extra_w += hull.w + 4.0;
                }
            }
        }
    }
    (pairs > 0).then_some(LabelCollision {
        pairs,
        extra_h,
        extra_w,
    })
}

fn size_fix(
    entry: &Entry,
    authored: &BTreeMap<String, Authored>,
    (need_w, grow_w): (f64, bool),
    (need_h, grow_h): (f64, bool),
) -> (Vec<String>, Option<zenith_core::FixHint>) {
    let mut sizes = Vec::new();
    if grow_h {
        sizes.push(format!("h=(px){need_h:.0}"));
    }
    if grow_w {
        sizes.push(format!("w=(px){need_w:.0}"));
    }
    let facts = authored
        .get(&entry.id)
        .filter(|_| !entry.expanded)
        .copied()
        .unwrap_or_default();
    let fix = if grow_h && facts.h_px.is_some() {
        Some(set_px("h", need_h))
    } else if grow_w && facts.w_px.is_some() {
        Some(set_px("w", need_w))
    } else {
        None
    };
    (sizes, fix)
}

fn collision_diagnostic(
    entry: &Entry,
    rect: LayoutBox,
    collision: LabelCollision,
    authored: &BTreeMap<String, Authored>,
) -> Diagnostic {
    let need_h = (rect.h + collision.extra_h).ceil();
    let need_w = (rect.w + collision.extra_w).ceil();
    let (sizes, fix) = size_fix(
        entry,
        authored,
        (need_w, collision.extra_w > 0.0),
        (need_h, collision.extra_h > 0.0),
    );
    let id = &entry.id;
    Diagnostic::warning(
        "chart.overflow",
        format!(
            "chart '{id}' own labels collide inside its {:.0}x{:.0}px box ({} overlapping label \
             pair(s)). The plot area is too cramped for its labels. Set {} to fit.",
            rect.w,
            rect.h,
            collision.pairs,
            sizes.join(" ")
        ),
        entry.span,
        Some(id.clone()),
    )
    .with_fix(fix)
}

fn diagnostic(
    entry: &Entry,
    rect: LayoutBox,
    spill: Spill,
    authored: &BTreeMap<String, Authored>,
) -> Diagnostic {
    let need_h = (rect.h + spill.vertical()).ceil();
    let need_w = (rect.w + spill.horizontal()).ceil();
    let (sizes, fix) = size_fix(
        entry,
        authored,
        (need_w, spill.horizontal() > 0.0),
        (need_h, spill.vertical() > 0.0),
    );
    let id = &entry.id;
    Diagnostic::warning(
        "chart.overflow",
        format!(
            "chart '{id}' text ink leaves its {:.0}x{:.0}px box ({}). The chart box holds \
             the title, caption, axis labels, and legend. Set {} to fit.",
            rect.w,
            rect.h,
            spill.sides(),
            sizes.join(" ")
        ),
        entry.span,
        Some(id.clone()),
    )
    .with_fix(fix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(x: f64, y: f64, w: f64, h: f64) -> LayoutBox {
        LayoutBox { x, y, w, h }
    }

    #[test]
    fn spill_ignores_the_tolerance() {
        let rect = b(0.0, 0.0, 100.0, 50.0);
        assert_eq!(
            Spill::of(rect, b(-0.4, -0.5, 100.8, 50.9)),
            Spill::default()
        );
        let s = Spill::of(rect, b(-3.0, 10.0, 90.0, 52.0));
        assert_eq!(
            s,
            Spill {
                top: 0.0,
                right: 0.0,
                bottom: 12.0,
                left: 3.0
            }
        );
        assert_eq!(s.sides(), "bottom 12px, left 3px");
    }
}
