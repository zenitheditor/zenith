//! `text.too_small`: a text's effective font size is under the legibility
//! floor for the page.
//!
//! The floor is `max(9px, 0.9%` of the shorter page side`)`, rounded up to
//! whole px. The size is the largest run size the compile drew, so `autofit`
//! shrinkage counts. Guide and hidden nodes, decoration and background roles,
//! and content drawn under a scale are skipped. Chart strings count too: the
//! subject names the chart and the text role (`chart 'sales' axis text`).

use std::collections::BTreeSet;

use zenith_core::{Diagnostic, FixHint};

use super::super::chart::ChartTextRole;
use super::ledger::{PageLedger, TextItem, TextSource};
use super::text_facts::{PageText, fmt_px};

/// Smallest floor, in px.
const MIN_FLOOR: f64 = 9.0;
/// Floor as a share of the shorter page side.
const FLOOR_SHARE: f64 = 0.009;
/// Rounding slack of a size drawn at exactly the floor.
const EPSILON: f64 = 0.01;

/// Every `text.too_small` of the page. `page` is the page size in px. A
/// chart reports each text role once, at its smallest drawn size.
pub(super) fn too_small(ledger: &PageLedger, text: &PageText, page: (f64, f64)) -> Vec<Diagnostic> {
    let floor = (page.0.min(page.1) * FLOOR_SHARE).max(MIN_FLOOR).ceil();
    let mut chart_seen: BTreeSet<(usize, ChartTextRole)> = BTreeSet::new();
    let mut items: Vec<&TextItem> = ledger.texts.iter().collect();
    // Smallest first, so the kept chart item per role is its smallest.
    items.sort_by(|a, b| {
        a.ink
            .font_size
            .total_cmp(&b.ink.font_size)
            .then(a.id.cmp(&b.id))
    });
    let mut out: Vec<(String, Diagnostic)> = items
        .into_iter()
        .filter(|t| match t.source {
            TextSource::Chart(role) => chart_seen.insert((t.entry, role)),
            TextSource::Node | TextSource::Label => true,
        })
        .filter_map(|t| judge(ledger, text, page, floor, t).map(|d| (t.id.clone(), d)))
        .collect();
    // Report in source-id order, independent of size.
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.into_iter().map(|(_, d)| d).collect()
}

fn judge(
    ledger: &PageLedger,
    text: &PageText,
    page: (f64, f64),
    floor: f64,
    item: &TextItem,
) -> Option<Diagnostic> {
    let entry = ledger.entry(item.entry)?;
    let size = item.ink.font_size;
    if !entry.visible || entry.exempt || entry.unmodeled || size <= 0.0 || size >= floor - EPSILON {
        return None;
    }
    let subject = match item.source {
        TextSource::Node => format!("'{}'", item.id),
        TextSource::Label => format!("{} '{}' label", entry.kind, entry.id),
        TextSource::Chart(role) => format!("chart '{}' {} text", entry.id, role.as_str()),
    };
    let head = format!(
        "{subject} {}px < {}px floor for {}×{}",
        fmt_px(size),
        fmt_px(floor),
        fmt_px(page.0),
        fmt_px(page.1)
    );
    if let TextSource::Chart(role) = item.source {
        // Every chart role scales the chart base size, the style font-size.
        let need = (floor / role.scale()).ceil();
        let message = format!(
            "{head} — set the chart style font-size to at least {}px ({} text draws at {} × font-size)",
            fmt_px(need),
            role.as_str(),
            role.scale()
        );
        return Some(Diagnostic::advisory(
            "text.too_small",
            message,
            entry.span,
            Some(entry.id.clone()),
        ));
    }
    let facts = if item.source == TextSource::Label || entry.expanded {
        None
    } else {
        text.facts.get(&entry.id)
    };
    let to = format!("(px){}", fmt_px(floor));
    let (message, fix) = match facts {
        Some(f) if f.size_literal => (
            format!("{head} — set font-size={to}"),
            Some(FixHint::SetProperty {
                property: "font-size".to_owned(),
                to,
            }),
        ),
        Some(f) => match &f.size_token {
            Some(token) => (
                format!(
                    "{head} — raise token '{token}' to at least {}px, or set font-size={to} on the node",
                    fmt_px(floor)
                ),
                None,
            ),
            None => (format!("{head} — set font-size={to}"), None),
        },
        None => (format!("{head} — set font-size={to}"), None),
    };
    Some(
        Diagnostic::advisory(
            "text.too_small",
            message,
            entry.span,
            Some(entry.id.clone()),
        )
        .with_fix(fix),
    )
}
