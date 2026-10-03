//! Arrangement checks over sibling geometry: `align.near_miss`,
//! `spacing.uneven_gap`, and `connector.crosses_node`, plus the sibling
//! selection and fix helpers the first two share.
//!
//! A node takes part in the sibling checks when it compiled to an unrotated
//! box, is visible, is not `role="decoration"` / `"background"` (own or
//! inherited), draws under no unmodeled ancestor, sits outside any table or
//! chart, and is not placed by a `row` / `column` / `grid` frame (the frame
//! owns its children's alignment and gaps). Connectors, groups, instances,
//! lights, and footnotes are left out: their boxes derive from other nodes.
//! Lines, polygons, polylines, and paths are left out: their box is the
//! painted extent, stroke included, not an authored edge.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, FixHint};

use crate::layout::LayoutBox;

use super::align::near_miss;
use super::connector::crosses_node;
use super::ledger::{Entry, PageLedger, TextItem};
use super::paint::Authored;
use super::spacing::uneven_gaps;

/// One node of a sibling set.
#[derive(Clone, Copy)]
pub(super) struct Sibling<'a> {
    pub(super) index: usize,
    pub(super) entry: &'a Entry,
    /// The final box.
    pub(super) rect: LayoutBox,
    /// The glyph ink of a text node.
    pub(super) text: Option<&'a TextItem>,
}

/// The arrangement diagnostics of one page. `routes` holds each stroked
/// connector's drawn route in page px.
pub(super) fn arrangement(
    ledger: &PageLedger,
    authored: &BTreeMap<String, Authored>,
    routes: &BTreeMap<String, Vec<(f64, f64)>>,
) -> Vec<Diagnostic> {
    let sets = sibling_sets(ledger);
    let mut out = Vec::new();
    for set in sets.values() {
        out.extend(near_miss(set, authored));
        out.extend(uneven_gaps(set, authored));
    }
    out.extend(crosses_node(ledger, routes));
    out
}

/// The checked nodes of the page, grouped by parent entry.
fn sibling_sets(ledger: &PageLedger) -> BTreeMap<Option<usize>, Vec<Sibling<'_>>> {
    let texts: BTreeMap<usize, &TextItem> = ledger
        .texts
        .iter()
        .filter(|t| !t.label && t.ink.axis_aligned && !t.ink.glyphs.is_empty())
        .map(|t| (t.entry, t))
        .collect();
    let mut sets: BTreeMap<Option<usize>, Vec<Sibling<'_>>> = BTreeMap::new();
    for (index, entry) in ledger.entries.iter().enumerate() {
        let Some(b) = entry.compiled else {
            continue;
        };
        let derived = matches!(
            entry.kind,
            "connector"
                | "group"
                | "instance"
                | "light"
                | "footnote"
                | "line"
                | "polygon"
                | "polyline"
                | "path"
        );
        if derived
            || !entry.visible
            || entry.exempt
            || entry.unmodeled
            || entry.in_flow
            || entry.scope.is_some()
            || b.rotate.is_some()
            || !(b.rect.w > 0.0 || b.rect.h > 0.0)
        {
            continue;
        }
        let text = texts.get(&index).copied();
        if matches!(entry.kind, "text" | "code" | "field" | "toc") && text.is_none() {
            // A text without measured ink has no edge to judge.
            continue;
        }
        sets.entry(entry.parent).or_default().push(Sibling {
            index,
            entry,
            rect: b.rect,
            text,
        });
    }
    sets
}

/// The authored axis value after moving the node by `delta` px, when a fix
/// can set it: an authored literal, not anchored, not placed by a layout
/// frame, not expanded content.
pub(super) fn moved(
    s: &Sibling<'_>,
    authored: &BTreeMap<String, Authored>,
    horizontal: bool,
    delta: f64,
) -> Option<f64> {
    if s.entry.expanded || s.entry.in_flow {
        return None;
    }
    let facts = authored.get(&s.entry.id)?;
    if facts.anchored || facts.in_flow {
        return None;
    }
    let at = if horizontal { facts.x_px } else { facts.y_px }?;
    Some(round2(at + delta))
}

/// The word for a move along an axis: `positive` is right or down.
pub(super) fn direction(horizontal: bool, positive: bool) -> &'static str {
    match (horizontal, positive) {
        (true, true) => "right",
        (true, false) => "left",
        (false, true) => "down",
        (false, false) => "up",
    }
}

/// The `SetProperty` hint for `property` = `value` px.
pub(super) fn set_px(property: &str, value: f64) -> FixHint {
    FixHint::SetProperty {
        property: property.to_owned(),
        to: format!("(px){}", num(value)),
    }
}

/// `value` rounded to two decimals.
fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// A px number for a message: whole numbers bare, else up to two decimals.
pub(super) fn num(value: f64) -> String {
    // `-0` prints as `0`.
    let r = round2(value) + 0.0;
    if (r - r.round()).abs() < 1e-9 {
        format!("{:.0}", r)
    } else {
        let s = format!("{r:.2}");
        s.trim_end_matches('0').to_owned()
    }
}

/// `'a','b','c'`, with at most `max` ids and a `+N more` tail.
pub(super) fn id_list<'a>(ids: impl IntoIterator<Item = &'a str>, max: usize) -> String {
    let all: Vec<&str> = ids.into_iter().collect();
    let shown: Vec<String> = all.iter().take(max).map(|id| format!("'{id}'")).collect();
    let mut out = shown.join(",");
    if all.len() > max {
        out.push_str(&format!(" +{} more", all.len() - max));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn num_trims_trailing_zeros() {
        assert_eq!(num(142.0), "142");
        assert_eq!(num(142.5), "142.5");
        assert_eq!(num(142.123), "142.12");
        assert_eq!(num(-0.004), "0");
    }

    #[test]
    fn id_list_caps_the_names() {
        assert_eq!(id_list(["a", "b"], 3), "'a','b'");
        assert_eq!(id_list(["a", "b", "c", "d"], 2), "'a','b' +2 more");
    }
}
