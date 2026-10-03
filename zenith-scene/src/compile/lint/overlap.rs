//! Text ink collisions: `text.ink_overlap` (two texts' glyphs collide) and
//! `text.occluded` (opaque paint drawn later hides a text's glyphs).
//!
//! Both checks skip guide and hidden nodes (they never compile) and any node
//! with `role="decoration"` or `role="background"` on itself or an ancestor.
//! Ink drawn under a rotation off a quarter turn is skipped: its glyph boxes
//! are bounds of turned boxes, not the drawn ink.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, FixHint};

use crate::layout::LayoutBox;

use super::geom::{area, covered_fraction, intersect, union};
use super::ledger::{PageLedger, TextItem};
use super::paint::Authored;

/// Glyph-ink intersection area, in px², above which two texts collide.
const MIN_OVERLAP_AREA: f64 = 4.0;
/// Effective opacity below which a text is a watermark, not body ink.
const MIN_OPACITY: f64 = 0.5;
/// Clearance the fix leaves between the upper ink bottom and the moved ink.
const CLEARANCE: f64 = 12.0;
/// Glyph area share a later opaque paint must cover to hide the glyph.
const HIDDEN_SHARE: f64 = 0.5;

/// `true` when the text takes part in the collision checks.
fn checked(ledger: &PageLedger, item: &TextItem) -> bool {
    ledger
        .entry(item.entry)
        .is_some_and(|e| e.visible && !e.exempt)
        && item.ink.axis_aligned
        && !item.ink.glyphs.is_empty()
}

/// How a message names a text: `'title'`, or `shape 'box' label`.
fn subject(ledger: &PageLedger, item: &TextItem) -> String {
    match (item.label, ledger.entry(item.entry)) {
        (true, Some(owner)) => format!("{} '{}' label", owner.kind, owner.id),
        _ => format!("'{}'", item.id),
    }
}

/// The glyph collision of two texts: summed glyph-pair intersection area
/// and the bounds of the intersections.
fn glyph_overlap(a: &TextItem, b: &TextItem) -> Option<(f64, LayoutBox)> {
    let near_b: Vec<LayoutBox> = a
        .ink
        .glyphs
        .iter()
        .copied()
        .filter(|g| intersect(*g, b.bounds).is_some())
        .collect();
    let near_a: Vec<LayoutBox> = b
        .ink
        .glyphs
        .iter()
        .copied()
        .filter(|g| intersect(*g, a.bounds).is_some())
        .collect();
    let mut sum = 0.0;
    let mut hull: Option<LayoutBox> = None;
    for ga in &near_b {
        for gb in &near_a {
            if let Some(i) = intersect(*ga, *gb) {
                sum += area(i);
                hull = Some(hull.map_or(i, |h| union(h, i)));
            }
        }
    }
    hull.map(|h| (sum, h))
}

/// `text.ink_overlap`: the glyph ink of two different texts intersects over
/// more than 4 px². Texts under 0.5 effective opacity and two texts inside
/// the same table or chart are skipped. The lower text (larger ink top) is
/// the subject; the fix moves it 12px below the upper text's ink.
pub(super) fn ink_overlap(
    ledger: &PageLedger,
    authored: &BTreeMap<String, Authored>,
) -> Vec<Diagnostic> {
    let mut items: Vec<&TextItem> = ledger
        .texts
        .iter()
        .filter(|t| checked(ledger, t))
        .filter(|t| f64::from(t.ink.alpha) / 255.0 >= MIN_OPACITY)
        .filter(|t| {
            ledger
                .entry(t.entry)
                .is_some_and(|e| e.opacity >= MIN_OPACITY)
        })
        .collect();
    // Sort and sweep along x on the ink bounds.
    items.sort_by(|a, b| a.bounds.x.total_cmp(&b.bounds.x).then(a.id.cmp(&b.id)));
    let mut out = Vec::new();
    for (i, a) in items.iter().enumerate() {
        let a_right = a.bounds.x + a.bounds.w;
        for b in items.iter().skip(i + 1) {
            if b.bounds.x >= a_right {
                break;
            }
            if a.entry == b.entry || intersect(a.bounds, b.bounds).is_none() {
                continue;
            }
            let scope_a = ledger.entry(a.entry).and_then(|e| e.scope);
            let scope_b = ledger.entry(b.entry).and_then(|e| e.scope);
            if scope_a.is_some() && scope_a == scope_b {
                continue;
            }
            let Some((sum, hull)) = glyph_overlap(a, b) else {
                continue;
            };
            if sum <= MIN_OVERLAP_AREA {
                continue;
            }
            // The upper ink stays; the lower ink moves down.
            let (upper, lower) = if (b.bounds.y, b.entry) < (a.bounds.y, a.entry) {
                (*b, *a)
            } else {
                (*a, *b)
            };
            out.push(overlap_diagnostic(
                ledger, authored, upper, lower, sum, hull,
            ));
        }
    }
    out
}

fn overlap_diagnostic(
    ledger: &PageLedger,
    authored: &BTreeMap<String, Authored>,
    upper: &TextItem,
    lower: &TextItem,
    sum: f64,
    hull: LayoutBox,
) -> Diagnostic {
    let a = subject(ledger, upper);
    let b = subject(ledger, lower);
    let upper_bottom = upper.bounds.y + upper.bounds.h;
    let shift = (upper_bottom + CLEARANCE - lower.bounds.y).ceil().max(0.0);
    let entry = ledger.entry(lower.entry);
    let target_y = entry
        .filter(|e| !lower.label && !e.expanded && !e.unmodeled && !e.in_flow)
        .and_then(|e| authored.get(&e.id))
        .filter(|f| !f.anchored && !f.in_flow)
        .and_then(|f| f.y_px)
        .map(|y| (y + shift).ceil());
    let head = format!(
        "{b} ink overlaps {a} by {:.0}x{:.0}px at ({:.0},{:.0}) ({:.0}px² of glyph ink)",
        hull.w, hull.h, hull.x, hull.y, sum
    );
    let (message, fix) = match target_y {
        Some(y) => (
            format!("{head} — move {b} to y=(px){y:.0} (12px below {a} ink bottom)"),
            Some(FixHint::SetProperty {
                property: "y".to_owned(),
                to: format!("(px){y:.0}"),
            }),
        ),
        None => (
            format!("{head} — move {b} down {shift:.0}px (12px below {a} ink bottom)"),
            None,
        ),
    };
    let subject_id = entry.map(|e| e.id.clone());
    Diagnostic::warning(
        "text.ink_overlap",
        message,
        entry.and_then(|e| e.span),
        subject_id,
    )
    .with_fix(fix)
}

/// `text.occluded`: a node painted later than a text, with an opaque solid
/// fill (or opaque image) at full effective opacity, covers more than half
/// the area of at least one of the text's glyph ink boxes.
pub(super) fn occluded(ledger: &PageLedger) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for item in ledger.texts.iter().filter(|t| checked(ledger, t)) {
        for (index, cover) in ledger.entries.iter().enumerate().skip(item.entry + 1) {
            let Some(occluder) = cover.occluder else {
                continue;
            };
            if !cover.visible
                || ledger.is_ancestor(index, item.entry)
                || intersect(occluder.region, item.bounds).is_none()
            {
                continue;
            }
            let hidden = item
                .ink
                .glyphs
                .iter()
                .filter(|g| {
                    covered_fraction(**g, occluder.region, occluder.shape, cover.clip)
                        > HIDDEN_SHARE
                })
                .count();
            if hidden == 0 {
                continue;
            }
            out.push(occluded_diagnostic(ledger, item, index, hidden));
        }
    }
    out
}

fn occluded_diagnostic(
    ledger: &PageLedger,
    item: &TextItem,
    cover_index: usize,
    hidden: usize,
) -> Diagnostic {
    let text = subject(ledger, item);
    let entry = ledger.entry(item.entry);
    let (cover_kind, cover_id) = ledger
        .entry(cover_index)
        .map_or(("node", ""), |c| (c.kind, c.id.as_str()));
    let total = item.ink.glyphs.len();
    Diagnostic::warning(
        "text.occluded",
        format!(
            "{text} is hidden under {cover_kind} '{cover_id}', which paints later with an \
             opaque fill: {hidden} of {total} glyph(s) are more than half covered — move \
             '{cover_id}' before the text in source order, or move one out of the other's box"
        ),
        entry.and_then(|e| e.span),
        entry.map(|e| e.id.clone()),
    )
}
