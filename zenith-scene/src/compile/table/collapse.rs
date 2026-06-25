//! `border-collapse="collapse"` edge deduplication: a float-safe canonical key
//! per border segment plus the accumulator/tie-break that lets two adjacent cells
//! contribute the same physical edge exactly once.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, PropertyValue, ResolvedToken, ResolvedValue, TableNode, dim_to_px};

use super::super::paint::resolve_property_color;
use super::place::CellRect;

/// Sub-pixel quantization grid used for edge-key comparison: 0.01 px resolution.
/// Two endpoints that differ by less than 0.01 px map to the same integer bucket,
/// preventing floating-point jitter from creating spurious duplicate edges.
const QUANTIZE: f64 = 100.0;

/// Quantize a coordinate to an integer on the 0.01 px grid.
///
/// The `as i64` cast is a Rust 1.45+ saturating cast: `f64::NAN` maps to `0`,
/// `f64::INFINITY` to `i64::MAX`, and `f64::NEG_INFINITY` to `i64::MIN`.
/// Wild coordinates (NaN/inf) therefore land on a degenerate but safe key;
/// they cannot appear in normal layout output since `dim_to_px` only produces
/// finite values and the geometry guards above reject non-finite table bounds.
#[inline]
fn quantize(v: f64) -> i64 {
    (v * QUANTIZE).round() as i64
}

/// A canonical, float-safe key for one border segment in collapse mode.
///
/// Endpoints are quantized to a 0.01 px integer grid and stored in canonical
/// order (`(min, max)` by (ax, ay) first then (bx, by)) so the same physical
/// edge contributed by two adjacent cells maps to a single key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct EdgeKey {
    ax: i64,
    ay: i64,
    bx: i64,
    by: i64,
}

impl EdgeKey {
    /// Build a canonical `EdgeKey` from two (possibly unordered) endpoints.
    /// The pair `(ax,ay)` is always the lesser endpoint so the key is symmetric.
    fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        let (qx1, qy1, qx2, qy2) = (quantize(x1), quantize(y1), quantize(x2), quantize(y2));
        // Canonical order: lexicographic by (ax, ay) ≤ (bx, by).
        if (qx1, qy1) <= (qx2, qy2) {
            EdgeKey {
                ax: qx1,
                ay: qy1,
                bx: qx2,
                by: qy2,
            }
        } else {
            EdgeKey {
                ax: qx2,
                ay: qy2,
                bx: qx1,
                by: qy1,
            }
        }
    }
}

/// The resolved style for one border segment in collapse mode.
///
/// We store the original f64 endpoints alongside the resolved color and width
/// so we can emit the `StrokeLine` with exact coordinates (no rounding drift).
pub(super) struct EdgeStyle {
    pub(super) x1: f64,
    pub(super) y1: f64,
    pub(super) x2: f64,
    pub(super) y2: f64,
    pub(super) color: crate::ir::Color,
    pub(super) stroke_width: f64,
    /// Whether this edge was contributed by a cell with an OWN explicit `border`
    /// property (not inherited from the table). Used for tie-breaking: explicit
    /// wins over inherited; if both are the same kind the later-in-placed-order
    /// writer wins (last-writer rule, BTreeMap insert overwrites).
    is_explicit: bool,
}

/// Try to insert `candidate` for `key` into the accumulator, applying the
/// collapse tie-break rule:
///
/// 1. A cell with its OWN explicit `border`/`border_width` wins over a cell
///    that inherited those values from the table default.
/// 2. If both are explicit OR both are inherited, the later-in-placed-order
///    cell wins (last-writer: simply overwrite the existing entry).
fn try_insert_edge(acc: &mut BTreeMap<EdgeKey, EdgeStyle>, key: EdgeKey, candidate: EdgeStyle) {
    // Determine whether to overwrite without holding a simultaneous immutable
    // borrow on `acc` (which would conflict with the mutable borrow needed for
    // `insert`). We read the `is_explicit` flag we need, then drop the borrow.
    let should_insert = match acc.get(&key) {
        None => true,
        Some(existing) => {
            // Explicit beats inherited; equal explicitness → last writer wins.
            candidate.is_explicit || !existing.is_explicit
        }
    };
    if should_insert {
        acc.insert(key, candidate);
    }
}

/// Resolve a `border-width` property to pixels without requiring an owned
/// `Option<PropertyValue>`. Mirrors the logic in
/// [`super::super::util::resolve_property_dimension_px`] but accepts an
/// `Option<&PropertyValue>` so callers can use `as_ref().or()` to pick
/// cell-over-table without cloning.
pub(super) fn resolve_border_width(
    prop: Option<&PropertyValue>,
    resolved: &BTreeMap<String, ResolvedToken>,
    default: f64,
) -> f64 {
    match prop {
        Some(PropertyValue::TokenRef(token_id)) => match resolved.get(token_id.as_str()) {
            Some(rt) => match &rt.value {
                ResolvedValue::Dimension(dim) => dim_to_px(dim.value, &dim.unit).unwrap_or(default),
                ResolvedValue::Color(_)
                | ResolvedValue::CmykColor { .. }
                | ResolvedValue::Number(_)
                | ResolvedValue::FontFamily(_)
                | ResolvedValue::FontWeight(_)
                | ResolvedValue::Gradient(_)
                | ResolvedValue::Shadow(_)
                | ResolvedValue::Filter(_)
                | ResolvedValue::Mask(_) => default,
            },
            None => default,
        },
        Some(PropertyValue::Dimension(dim)) => dim_to_px(dim.value, &dim.unit).unwrap_or(default),
        Some(PropertyValue::Literal(_)) | Some(PropertyValue::DataRef(_)) | None => default,
    }
}

/// Accumulate the four border edges of one cell into the collapse-mode dedup map.
///
/// Tie-break rule (documented here as the authoritative source):
///   1. A cell with its OWN explicit `border`/`border_width` property (i.e. the
///      `TableCell.border` field is `Some`) wins over a cell that inherited those
///      values from the table-level defaults.
///   2. If both contributing cells are equally explicit or equally inherited,
///      the later-in-placed-order cell wins (last writer overwrites).
///
/// Only inserts edges when the resolved border color is `Some` AND the resolved
/// width is > 0 (same guard as the separate-mode path).
pub(super) fn accumulate_cell_edges(
    table: &TableNode,
    cell: &zenith_core::TableCell,
    rect: &CellRect,
    opacity: f64,
    resolved: &BTreeMap<String, ResolvedToken>,
    diagnostics: &mut Vec<Diagnostic>,
    acc: &mut BTreeMap<EdgeKey, EdgeStyle>,
) {
    let border_prop: Option<&PropertyValue> = cell.border.as_ref().or(table.border.as_ref());
    let Some(prop) = border_prop else { return };
    let Some(mut color) = resolve_property_color(prop, resolved, diagnostics, &table.id) else {
        return;
    };
    color.a = (color.a as f64 * opacity).round() as u8;

    let bw = resolve_border_width(
        cell.border_width.as_ref().or(table.border_width.as_ref()),
        resolved,
        1.0,
    )
    .max(0.0);
    if bw <= 0.0 {
        return;
    }

    // Whether this cell contributes an OWN explicit border (not table fallback).
    let is_explicit = cell.border.is_some();

    let x0 = rect.x;
    let y0 = rect.y;
    let x1 = rect.x + rect.w;
    let y1 = rect.y + rect.h;

    for (ex1, ey1, ex2, ey2) in [
        (x0, y0, x1, y0), // top
        (x0, y1, x1, y1), // bottom
        (x0, y0, x0, y1), // left
        (x1, y0, x1, y1), // right
    ] {
        let key = EdgeKey::new(ex1, ey1, ex2, ey2);
        let candidate = EdgeStyle {
            x1: ex1,
            y1: ey1,
            x2: ex2,
            y2: ey2,
            color,
            stroke_width: bw,
            is_explicit,
        };
        try_insert_edge(acc, key, candidate);
    }
}
