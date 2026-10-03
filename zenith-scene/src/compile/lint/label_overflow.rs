//! `label.overflow`: a shape label's ink leaves the shape's own outline.
//!
//! The compile lays a `shape` label into the shape box inset by `padding`
//! for every kind, and `text.overflow` already reports label ink that leaves
//! that padded box. For a `process` shape (and any unknown kind) the padded
//! box is the content area, so `text.overflow` is the whole check. An
//! `ellipse`, `decision` (diamond), or `terminator` (pill) shape paints less
//! than its box: its content area is the outline inset by `padding`. This
//! check reports label ink that stays inside the padded box but crosses
//! that outline. A shape with a `text.overflow` (or `text.fit_failed`)
//! diagnostic in the same compile is skipped, so one cause reports once.
//! Connector labels have no enclosing outline and are not checked.

use zenith_core::Diagnostic;

use crate::layout::LayoutBox;

use super::geom::bounds;
use super::ledger::{Entry, PageLedger, TextSource};

/// Ink tolerance in px on each side, matching the overflow check.
const EPSILON: f64 = 0.5;

/// The outline kinds this check judges.
#[derive(Clone, Copy)]
enum Outline {
    Ellipse,
    Diamond,
    Pill,
}

impl Outline {
    fn of(kind: &str) -> Option<Self> {
        match kind {
            "ellipse" => Some(Self::Ellipse),
            "decision" => Some(Self::Diamond),
            "terminator" => Some(Self::Pill),
            _ => None,
        }
    }

    fn noun(self) -> &'static str {
        match self {
            Self::Ellipse => "ellipse",
            Self::Diamond => "diamond",
            Self::Pill => "pill",
        }
    }
}

/// Every `label.overflow` of the page.
pub(super) fn label_overflow(ledger: &PageLedger, compiled: &[Diagnostic]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for item in ledger
        .texts
        .iter()
        .filter(|t| t.source == TextSource::Label)
    {
        let Some(entry) = ledger.entry(item.entry) else {
            continue;
        };
        let (Some(facts), Some(b)) = (&entry.shape, entry.compiled) else {
            continue;
        };
        let Some(outline) = Outline::of(&facts.kind) else {
            continue;
        };
        if entry.unmodeled || entry.exempt {
            continue;
        }
        let reported = compiled.iter().any(|d| {
            matches!(d.code.as_str(), "text.overflow" | "text.fit_failed")
                && d.subject_id.as_deref() == Some(entry.id.as_str())
        });
        if reported {
            continue;
        }
        let Some(ink) = bounds(&item.ink.local) else {
            continue;
        };
        if let Some(d) = judge(entry, outline, b.rect, facts.pad, ink, item.ink.font_size) {
            out.push(d);
        }
    }
    out
}

/// Ink corners relative to the box centre, pulled in by the tolerance.
fn corners(rect: LayoutBox, ink: LayoutBox) -> [(f64, f64); 4] {
    let (cx, cy) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
    let left = ink.x + EPSILON - cx;
    let right = ink.x + ink.w - EPSILON - cx;
    let top = ink.y + EPSILON - cy;
    let bottom = ink.y + ink.h - EPSILON - cy;
    [(left, top), (right, top), (left, bottom), (right, bottom)]
}

fn judge(
    entry: &Entry,
    outline: Outline,
    rect: LayoutBox,
    pad: f64,
    ink: LayoutBox,
    font_size: f64,
) -> Option<Diagnostic> {
    let a = rect.w / 2.0 - pad;
    let b = rect.h / 2.0 - pad;
    if a <= 0.0 || b <= 0.0 {
        return None;
    }
    let pts = corners(rect, ink);
    let noun = outline.noun();
    let id = &entry.id;
    let message = match outline {
        Outline::Ellipse | Outline::Diamond => {
            // The uniform scale of the inner outline that holds every corner.
            let scale = pts
                .iter()
                .map(|&(x, y)| match outline {
                    Outline::Ellipse => ((x / a).powi(2) + (y / b).powi(2)).sqrt(),
                    Outline::Diamond | Outline::Pill => x.abs() / a + y.abs() / b,
                })
                .fold(0.0_f64, f64::max);
            if scale <= 1.0 {
                return None;
            }
            let need_w = (2.0 * a * scale + 2.0 * pad).ceil();
            let need_h = (2.0 * b * scale + 2.0 * pad).ceil();
            let fits = (font_size / scale).floor();
            format!(
                "label of shape '{id}' crosses its {noun} outline: the ink needs a \
                 {need_w:.0}x{need_h:.0}px shape (now {:.0}x{:.0}px) — set w=(px){need_w:.0} \
                 h=(px){need_h:.0}, or lower the label font-size to {fits:.0}px",
                rect.w, rect.h
            )
        }
        Outline::Pill => {
            // Straight half-length the pill needs, keeping its height.
            let r = b;
            let straight = (a - r).max(0.0);
            let mut need = straight;
            for &(x, y) in &pts {
                let rise = r * r - y * y;
                if rise < 0.0 {
                    // Taller than the pill: `text.overflow` territory.
                    return None;
                }
                need = need.max(x.abs() - rise.sqrt());
            }
            if need <= straight {
                return None;
            }
            let need_w = (2.0 * (need + r + pad)).ceil();
            format!(
                "label of shape '{id}' crosses its {noun} outline: the ink needs a \
                 {need_w:.0}px wide shape (now {:.0}px) — set w=(px){need_w:.0}, or shorten \
                 the label",
                rect.w
            )
        }
    };
    Some(Diagnostic::warning(
        "label.overflow",
        message,
        entry.span,
        Some(id.clone()),
    ))
}
