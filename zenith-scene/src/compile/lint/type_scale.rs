//! `type.near_duplicate_size`: two distinct authored font sizes inside one
//! top-level group or frame sit too close to read as different steps.
//!
//! Sizes are grouped by resolved px, scoped per top-level `frame` / `group`
//! (an artboard) so alternate layouts do not compare against each other.
//! Neighbouring sizes in one scope that differ by 1px or less, or by under
//! 8%, report once per pair. Chart, table, and code internals never enter
//! the list.

use std::collections::BTreeMap;

use zenith_core::Diagnostic;

use super::text_facts::{PageText, SizeUse, fmt_px};

/// Largest px gap that always reads as one size.
const MAX_GAP: f64 = 1.0;
/// Size ratio under which two sizes read as one size.
const MIN_RATIO: f64 = 1.08;
/// Two sizes closer than this are one size.
const SAME: f64 = 1e-6;

/// `true` when two distinct sizes `lo_px` ≤ `hi_px` read as one size: they
/// differ by 1px or less, or by under 8%. This is the
/// `type.near_duplicate_size` test.
pub fn sizes_read_as_one(lo_px: f64, hi_px: f64) -> bool {
    hi_px - lo_px <= MAX_GAP || hi_px / lo_px < MIN_RATIO
}

/// Every `type.near_duplicate_size` of the page.
pub(super) fn near_duplicate_size(text: &PageText) -> Vec<Diagnostic> {
    let mut scopes: BTreeMap<&str, Vec<&SizeUse>> = BTreeMap::new();
    for s in &text.sizes {
        scopes.entry(s.scope.as_str()).or_default().push(s);
    }
    let mut out = Vec::new();
    for uses in scopes.values() {
        // One entry per distinct px: the first use in source order.
        let mut distinct: Vec<&SizeUse> = Vec::new();
        for s in uses {
            if !distinct.iter().any(|d| (d.px - s.px).abs() < SAME) {
                distinct.push(s);
            }
        }
        distinct.sort_by(|a, b| a.px.total_cmp(&b.px).then(a.node.cmp(&b.node)));
        for pair in distinct.windows(2) {
            if let [lo, hi] = pair
                && sizes_read_as_one(lo.px, hi.px)
            {
                out.push(diagnostic(lo, hi));
            }
        }
    }
    out
}

fn diagnostic(lo: &SizeUse, hi: &SizeUse) -> Diagnostic {
    let ratio = hi.px / lo.px;
    let by = if ratio < MIN_RATIO {
        format!("{:.0}%", (ratio - 1.0) * 100.0)
    } else {
        format!("{}px", fmt_px(hi.px - lo.px))
    };
    Diagnostic::advisory(
        "type.near_duplicate_size",
        format!(
            "sizes {} ({}) and {} ({}) differ by {by} — merge into one token",
            fmt_px(lo.px),
            lo.source,
            fmt_px(hi.px),
            hi.source
        ),
        hi.span,
        Some(hi.node.clone()),
    )
}
