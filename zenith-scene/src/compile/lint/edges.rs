//! `text.edge_crowding`: a text's glyph ink sits too close to the trim edge.
//!
//! The floor is `max(8px, 1.5%` of the shorter page side`)`, rounded up to
//! whole px. Ink that leaves the trim is bleed, not crowding, and is skipped.
//! A page that declares margins or a safe zone is skipped: those checks own
//! the live area. Labels, guide and hidden nodes, decoration and background
//! roles, watermark-weight text, and ink drawn off axis are skipped.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, FixHint};

use super::ledger::{PageLedger, TextItem, TextSource};
use super::paint::Authored;
use super::text_facts::{PageText, TextFacts, fmt_px};

/// Smallest floor, in px.
const MIN_FLOOR: f64 = 8.0;
/// Floor as a share of the shorter page side.
const FLOOR_SHARE: f64 = 0.015;
/// Effective opacity below which a text is a watermark, not body ink.
const MIN_OPACITY: f64 = 0.5;

/// The trim box the ink is measured against, in scene px.
#[derive(Clone, Copy)]
pub(super) struct Trim {
    /// Scene offset of the trim box (the bleed).
    pub(super) origin: f64,
    pub(super) w: f64,
    pub(super) h: f64,
}

#[derive(Clone, Copy)]
enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

impl Edge {
    fn name(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    fn direction(self) -> &'static str {
        match self {
            Self::Top => "down",
            Self::Bottom => "up",
            Self::Left => "right",
            Self::Right => "left",
        }
    }
}

/// Every `text.edge_crowding` of the page. `live_area` is `true` when the
/// page declares margins or a safe zone.
pub(super) fn edge_crowding(
    ledger: &PageLedger,
    authored: &BTreeMap<String, Authored>,
    text: &PageText,
    trim: Trim,
    live_area: bool,
) -> Vec<Diagnostic> {
    if live_area {
        return Vec::new();
    }
    let floor = (trim.w.min(trim.h) * FLOOR_SHARE).max(MIN_FLOOR).ceil();
    ledger
        .texts
        .iter()
        .filter(|t| t.source == TextSource::Node)
        .filter_map(|t| judge(ledger, authored, text, trim, floor, t))
        .collect()
}

fn judge(
    ledger: &PageLedger,
    authored: &BTreeMap<String, Authored>,
    text: &PageText,
    trim: Trim,
    floor: f64,
    item: &TextItem,
) -> Option<Diagnostic> {
    let entry = ledger.entry(item.entry)?;
    if !entry.visible
        || entry.exempt
        || entry.unmodeled
        || entry.opacity < MIN_OPACITY
        || !item.ink.axis_aligned
        || f64::from(item.ink.alpha) / 255.0 < MIN_OPACITY
    {
        return None;
    }
    let b = item.bounds;
    let gaps = [
        (Edge::Top, b.y - trim.origin),
        (Edge::Left, b.x - trim.origin),
        (Edge::Bottom, trim.origin + trim.h - (b.y + b.h)),
        (Edge::Right, trim.origin + trim.w - (b.x + b.w)),
    ];
    // Ink that leaves the trim on any side is bleed.
    if gaps.iter().any(|(_, gap)| *gap < 0.0) {
        return None;
    }
    let (edge, gap) = gaps
        .iter()
        .copied()
        .min_by(|a, c| a.1.total_cmp(&c.1))
        .filter(|(_, gap)| *gap < floor)?;
    let shift = (floor - gap).ceil();
    let head = format!(
        "'{}' ink is {}px from {} edge (floor {}px)",
        item.id,
        fmt_px(gap.floor()),
        edge.name(),
        fmt_px(floor)
    );
    let facts = text.facts.get(&entry.id);
    let placed = !entry.expanded
        && !entry.in_flow
        && authored
            .get(&entry.id)
            .is_some_and(|a| !a.anchored && !a.in_flow);
    let fix = placed.then(|| axis_fix(edge, facts?, shift)).flatten();
    let (message, hint) = match fix {
        Some((property, to)) => (
            format!("{head} — set {property}={to}"),
            Some(FixHint::SetProperty { property, to }),
        ),
        None => (
            format!(
                "{head} — move '{}' {}px {}",
                item.id,
                fmt_px(shift),
                edge.direction()
            ),
            None,
        ),
    };
    Some(
        Diagnostic::advisory(
            "text.edge_crowding",
            message,
            entry.span,
            Some(entry.id.clone()),
        )
        .with_fix(hint),
    )
}

/// The property and value that move the node `shift` px away from `edge`.
fn axis_fix(edge: Edge, facts: &TextFacts, shift: f64) -> Option<(String, String)> {
    let (property, value) = match edge {
        Edge::Top => ("y", facts.y_px? + shift),
        Edge::Bottom => ("y", facts.y_px? - shift),
        Edge::Left => ("x", facts.x_px? + shift),
        Edge::Right => ("x", facts.x_px? - shift),
    };
    let rounded = match edge {
        Edge::Top | Edge::Left => value.ceil(),
        Edge::Bottom | Edge::Right => value.floor(),
    };
    Some((property.to_owned(), format!("(px){}", fmt_px(rounded))))
}
