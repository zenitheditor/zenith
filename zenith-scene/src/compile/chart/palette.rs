//! Categorical color palette for chart series, plus the shared series-color
//! resolver used by bar, line, area, and sparkline renderers.
//!
//! A deterministic 8-color sequence is used when a series declares no explicit
//! color. Colors are indexed by series position modulo 8.

use std::collections::BTreeMap;

use zenith_core::{ChartSeries, Diagnostic, ResolvedToken};

use crate::ir::Color;

use super::super::paint::resolve_property_color;

/// Deterministic categorical palette, indexed by series position when a series
/// declares no explicit color. Perceptually distinct at typical chart sizes.
pub(super) const SERIES_PALETTE: [Color; 8] = [
    Color::srgb(66, 133, 244, 255), // blue
    Color::srgb(234, 67, 53, 255),  // red
    Color::srgb(52, 168, 83, 255),  // green
    Color::srgb(251, 188, 4, 255),  // yellow
    Color::srgb(255, 109, 0, 255),  // orange
    Color::srgb(103, 58, 183, 255), // purple
    Color::srgb(0, 172, 193, 255),  // cyan
    Color::srgb(233, 30, 99, 255),  // pink
];

/// Resolve the display color for a single series.
///
/// Resolution order:
/// 1. Explicit `series.color` property (token ref or literal) → resolve it.
/// 2. Palette fallback: `SERIES_PALETTE[idx % 8]`.
///
/// This is the canonical resolver shared by all series-bearing chart kinds
/// (`bar`, `line`, `area`, `sparkline`) so the color semantics stay identical
/// across renderers.
pub(super) fn series_color(
    series: &ChartSeries,
    idx: usize,
    resolved: &BTreeMap<String, ResolvedToken>,
    diagnostics: &mut Vec<Diagnostic>,
    chart_id: &str,
) -> Color {
    series
        .color
        .as_ref()
        .and_then(|prop| resolve_property_color(prop, resolved, diagnostics, chart_id))
        .unwrap_or_else(|| {
            // SERIES_PALETTE has a fixed length; modulo keeps the index in
            // bounds. Use .get() so the access is panic-free.
            // SERIES_PALETTE has exactly 8 entries; both .get() calls are
            // always Some, but we express both as infallible fallbacks to
            // satisfy the no-unchecked-indexing rule.
            SERIES_PALETTE
                .get(idx % SERIES_PALETTE.len())
                .copied()
                .unwrap_or(Color::srgb(66, 133, 244, 255))
        })
}
