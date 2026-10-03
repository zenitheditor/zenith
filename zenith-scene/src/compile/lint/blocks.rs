//! `layout.block_overlap`: two sibling blocks overlap in part.
//!
//! A block is a `frame`, `group`, `chart`, `table`, `image`, `shape`, `rect`,
//! `ellipse`, `instance`, or text-like node (`text`, `code`, `field`, `toc`)
//! with a compiled, unrotated box. Other kinds are left out:
//!
//! - `line`, `polygon`, `polyline`, `path`: the box is the painted extent,
//!   stroke included, not an authored edge.
//! - `connector`: its route derives from the nodes it joins.
//! - `footnote`, `light`, `pattern`, `mesh`: they place no block of their
//!   own on the page.
//!
//! A text-like node is measured by its glyph ink, not its box. A text box
//! wider than its words covers nothing. Every block is cut to the page and
//! to its clipping frames first: paint off the page or clipped away is not
//! seen.
//!
//! These blocks never take part:
//!
//! - hidden, or `role="decoration"` / `"background"` (own or inherited).
//! - under 0.5 effective opacity: a tint or glow, not a layer.
//! - drawn through its own blend, mask, filter, or blur: it composites on
//!   purpose.
//! - a `rect`, `ellipse`, or `shape` with no fill: an outline holds no paint.
//! - under an unmodeled ancestor, expanded content, or inside a table or
//!   chart.
//!
//! The children of a `group` never pair: a group is one drawing, such as an
//! icon. The group itself pairs with its siblings.
//!
//! Only blocks with the same parent pair up. Two blocks placed by the same
//! `row` / `column` / `grid` frame are skipped: the frame owns their places.
//! A pair fires on a partial overlap: the intersection is over 16 px² and
//! over 2% of the smaller box, and under 95% of the smaller box. A block
//! (near) inside another is intended layering and stays silent.
//!
//! Two texts are skipped: `text.ink_overlap` reports them. A text under a
//! later block with an opaque fill is skipped: `text.occluded` reports it.
//! A text over a block, or under a block without an opaque fill, fires: its
//! ink straddles the block edge.

use std::collections::BTreeMap;

use zenith_core::Diagnostic;

use crate::layout::LayoutBox;

use super::geom::{area, intersect};
use super::ledger::{Entry, PageLedger, TextSource};

/// Intersection area in px² at or below which a pair is silent.
const MIN_AREA: f64 = 16.0;
/// Intersection share of the smaller box at or below which a pair is silent.
const MIN_SHARE: f64 = 0.02;
/// Intersection share of the smaller box at or above which the smaller box
/// counts as inside the larger.
const CONTAINED: f64 = 0.95;
/// Effective opacity below which a block is a tint or glow, not a layer.
const MIN_OPACITY: f64 = 0.5;

/// One block of a sibling set.
#[derive(Clone, Copy)]
struct Block<'a> {
    entry: &'a Entry,
    /// The compiled box, or the glyph ink bounds of a text.
    rect: LayoutBox,
    text: bool,
}

/// Every `layout.block_overlap` of the page. `page` is the trim box in page
/// px, when the page size resolves.
pub(super) fn block_overlap(ledger: &PageLedger, page: Option<LayoutBox>) -> Vec<Diagnostic> {
    let inks: BTreeMap<usize, LayoutBox> = ledger
        .texts
        .iter()
        .filter(|t| t.source == TextSource::Node && t.ink.axis_aligned && !t.ink.glyphs.is_empty())
        .map(|t| (t.entry, t.bounds))
        .collect();
    let mut sets: BTreeMap<Option<usize>, Vec<Block<'_>>> = BTreeMap::new();
    for (index, entry) in ledger.entries.iter().enumerate() {
        if drawn_in_group(ledger, entry) {
            continue;
        }
        if let Some(block) = block(index, entry, &inks, page) {
            sets.entry(entry.parent).or_default().push(block);
        }
    }
    let mut out = Vec::new();
    for set in sets.values() {
        // Entries enumerate in paint order, so `b` paints after `a`.
        for (i, a) in set.iter().enumerate() {
            for b in set.iter().skip(i + 1) {
                if let Some(d) = judge(a, b) {
                    out.push(d);
                }
            }
        }
    }
    out
}

/// The children of a `group` form one drawing, such as an icon or a logo.
/// Their overlaps are the drawing, so they never pair.
fn drawn_in_group(ledger: &PageLedger, entry: &Entry) -> bool {
    entry
        .parent
        .and_then(|p| ledger.entries.get(p))
        .is_some_and(|parent| parent.kind == "group")
}

/// The block of one entry, when it takes part.
fn block<'a>(
    index: usize,
    entry: &'a Entry,
    inks: &BTreeMap<usize, LayoutBox>,
    page: Option<LayoutBox>,
) -> Option<Block<'a>> {
    let b = entry.compiled?;
    let text = matches!(entry.kind, "text" | "code" | "field" | "toc");
    let boxed = matches!(
        entry.kind,
        "frame" | "group" | "chart" | "table" | "image" | "shape" | "rect" | "ellipse" | "instance"
    );
    if !(text || boxed)
        || !entry.visible
        || entry.exempt
        || entry.opacity < MIN_OPACITY
        || entry.effects
        || entry.hollow
        || entry.unmodeled
        || entry.expanded
        || entry.scope.is_some()
        || b.rotate.is_some()
    {
        return None;
    }
    let drawn = if text {
        inks.get(&index).copied()?
    } else {
        b.rect
    };
    let on_page = match page {
        Some(p) => intersect(drawn, p)?,
        None => drawn,
    };
    let rect = match entry.clip {
        Some(c) => intersect(on_page, c)?,
        None => on_page,
    };
    (area(rect) > 0.0).then_some(Block { entry, rect, text })
}

/// The diagnostic of one pair, `later` painted after `earlier`.
fn judge(earlier: &Block<'_>, later: &Block<'_>) -> Option<Diagnostic> {
    let hit = intersect(earlier.rect, later.rect)?;
    if earlier.text && later.text {
        return None;
    }
    if earlier.entry.in_flow && later.entry.in_flow {
        return None;
    }
    if earlier.text && later.entry.occluder.is_some() {
        return None;
    }
    let shared = area(hit);
    let smaller = area(earlier.rect).min(area(later.rect));
    if shared <= MIN_AREA || shared <= MIN_SHARE * smaller || shared >= CONTAINED * smaller {
        return None;
    }
    Some(Diagnostic::warning(
        "layout.block_overlap",
        format!(
            "block \"{}\" overlaps \"{}\" by {:.0}x{:.0} at ({:.0},{:.0}); move one, group \
             them as one drawing, or mark the intended layer role=\"decoration\"/\"background\".",
            later.entry.id, earlier.entry.id, hit.w, hit.h, hit.x, hit.y
        ),
        later.entry.span,
        Some(later.entry.id.clone()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, kind: &'static str, in_flow: bool) -> Entry {
        Entry {
            id: id.to_owned(),
            parent: None,
            kind,
            span: None,
            compiled: None,
            opacity: 1.0,
            occluder: None,
            visible: true,
            in_flow,
            exempt: false,
            unmodeled: false,
            effects: false,
            hollow: false,
            expanded: false,
            scope: None,
            clip: None,
            shape: None,
            connector: None,
        }
    }

    fn b(x: f64, y: f64, w: f64, h: f64) -> LayoutBox {
        LayoutBox { x, y, w, h }
    }

    fn pair(
        (ea, ra, ta): (&Entry, LayoutBox, bool),
        (eb, rb, tb): (&Entry, LayoutBox, bool),
    ) -> Option<Diagnostic> {
        judge(
            &Block {
                entry: ea,
                rect: ra,
                text: ta,
            },
            &Block {
                entry: eb,
                rect: rb,
                text: tb,
            },
        )
    }

    #[test]
    fn thresholds_split_partial_from_contained() {
        let a = entry("a", "frame", false);
        let z = entry("z", "rect", false);
        let base = b(0.0, 0.0, 100.0, 100.0);
        // Partial: 20 x 100 of a 100 x 100 box.
        let d = pair((&a, base, false), (&z, b(80.0, 0.0, 100.0, 100.0), false));
        let d = d.map(|d| (d.subject_id, d.message));
        assert_eq!(
            d,
            Some((
                Some("z".to_owned()),
                "block \"z\" overlaps \"a\" by 20x100 at (80,0); move one, group them as one \
                 drawing, or mark the intended layer role=\"decoration\"/\"background\"."
                    .to_owned()
            ))
        );
        // Contained: 96% of the smaller box sits inside.
        assert!(pair((&a, base, false), (&z, b(2.0, 10.0, 50.0, 50.0), false)).is_none());
        // Tiny: 3 x 5 = 15 px².
        assert!(pair((&a, base, false), (&z, b(97.0, 0.0, 50.0, 5.0), false)).is_none());
        // Thin: 2% of a 400 x 400 box.
        let big = b(0.0, 0.0, 400.0, 400.0);
        assert!(pair((&a, big, false), (&z, b(396.0, 0.0, 400.0, 400.0), false)).is_none());
    }

    #[test]
    fn text_pairs_and_flow_pairs_stay_silent() {
        let base = b(0.0, 0.0, 100.0, 100.0);
        let side = b(80.0, 0.0, 100.0, 100.0);
        let t = entry("t", "text", false);
        let u = entry("u", "text", false);
        assert!(pair((&t, base, true), (&u, side, true)).is_none());
        let f = entry("f", "frame", true);
        let g = entry("g", "frame", true);
        assert!(pair((&f, base, false), (&g, side, false)).is_none());
        // A text over a block fires.
        let r = entry("r", "rect", false);
        assert!(pair((&r, base, false), (&t, side, true)).is_some());
    }
}
