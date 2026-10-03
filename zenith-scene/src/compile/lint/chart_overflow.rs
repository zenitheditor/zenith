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
use super::geom::{bounds, union};
use super::ledger::{Entry, PageLedger, TextSource};
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

/// Every `chart.overflow` of the page.
pub(super) fn chart_overflow(
    ledger: &PageLedger,
    authored: &BTreeMap<String, Authored>,
) -> Vec<Diagnostic> {
    let mut inks: BTreeMap<usize, LayoutBox> = BTreeMap::new();
    for item in ledger.texts.iter() {
        if !matches!(item.source, TextSource::Chart(_)) {
            continue;
        }
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
        if spill == Spill::default() {
            continue;
        }
        out.push(diagnostic(entry, b.rect, spill, authored));
    }
    out
}

fn diagnostic(
    entry: &Entry,
    rect: LayoutBox,
    spill: Spill,
    authored: &BTreeMap<String, Authored>,
) -> Diagnostic {
    let need_h = (rect.h + spill.vertical()).ceil();
    let need_w = (rect.w + spill.horizontal()).ceil();
    let mut sizes = Vec::new();
    if spill.vertical() > 0.0 {
        sizes.push(format!("h=(px){need_h:.0}"));
    }
    if spill.horizontal() > 0.0 {
        sizes.push(format!("w=(px){need_w:.0}"));
    }
    let facts = authored
        .get(&entry.id)
        .filter(|_| !entry.expanded)
        .copied()
        .unwrap_or_default();
    let fix = if spill.vertical() > 0.0 && facts.h_px.is_some() {
        Some(set_px("h", need_h))
    } else if spill.horizontal() > 0.0 && facts.w_px.is_some() {
        Some(set_px("w", need_w))
    } else {
        None
    };
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
