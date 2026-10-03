//! Intrinsic size of a `table`: the box its rows and columns need before any
//! shrink-to-fit, for the auto-layout engine.

use zenith_core::{Diagnostic, TableNode};

use super::super::text::MeasureEnv;
use super::super::util::resolve_property_dimension_px;
use super::layout::{GridDims, TableLayout, compute_table_layout};
use super::place::place_cells;

/// The natural `(w, h)` of `table`, in px.
///
/// With `width = None` the columns take their natural widths (auto columns at
/// their widest cell). With `Some(w)` the columns lay out in `w`, as on render.
/// The height is the sum of the natural row heights, the row gaps, and the two
/// cell-padding insets. A table without rows measures `(0, 0)`.
pub(in crate::compile) fn table_natural_size(
    table: &TableNode,
    width: Option<f64>,
    env: MeasureEnv,
) -> (f64, f64) {
    let rows = &table.rows;
    let row_count = rows.len();
    if row_count == 0 {
        return (0.0, 0.0);
    }
    let col_count = table.columns.len().max(1);
    let gap = resolve_property_dimension_px(table.gap.as_ref(), env.resolved, 0.0).max(0.0);
    let pad =
        resolve_property_dimension_px(table.cell_padding.as_ref(), env.resolved, 0.0).max(0.0);
    let header_rows = (table.header_rows.unwrap_or(0) as usize).min(row_count);
    let placed = place_cells(rows, col_count, row_count);
    let mut scratch: Vec<Diagnostic> = Vec::new();
    let TableLayout {
        col_widths,
        row_heights,
        row_natural: _,
    } = compute_table_layout(
        &table.columns,
        &placed,
        GridDims {
            col_count,
            row_count,
            gap,
            pad,
            table_w: width.unwrap_or(f64::INFINITY),
            table_h: f64::INFINITY,
        },
        env,
        &mut scratch,
        header_rows,
        table.header_style.as_deref(),
    );
    let gaps = |n: usize| gap * n.saturating_sub(1) as f64;
    let natural_w = col_widths.iter().sum::<f64>() + gaps(col_widths.len()) + 2.0 * pad;
    let h = row_heights.iter().sum::<f64>() + gaps(row_heights.len()) + 2.0 * pad;
    (width.unwrap_or(natural_w), h)
}
