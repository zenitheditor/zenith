//! Contrast of text inside table cells.
//!
//! Cell content lays out cell-relative, so each cell is walked on its own
//! backdrop: the cell fill, else the header or table fill, else the page.
//! Cell text is judged by its cell-relative box, not by page-px glyph ink.

use crate::ast::node::TableNode;
use crate::ast::value::PropertyValue;
use crate::diagnostics::Diagnostic;

use super::paint::resolve_fill_paint;
use super::types::{ContrastEnv, PaintCtx};
use super::walk::walk_paint;

/// Judge the text of every cell of `table`, which sits in the place of
/// `ctx`: cell content inherits its translation, scale, opacity, and
/// unmodeled ancestors. Cell content is not clipped to the ancestor clip,
/// since its cell-relative boxes have no page position.
pub(super) fn check_table_text_contrast(
    table: &TableNode,
    ctx: PaintCtx<'_>,
    env: ContrastEnv<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let page_bg_rgb = ctx.page_bg_rgb;
    let parent = ctx;
    let header_rows = table.header_rows.unwrap_or(0);
    let resolve_fill = |pv: &Option<PropertyValue>| -> Option<(u8, u8, u8)> {
        resolve_fill_paint(pv, None, env.style_map, env.resolved_tokens, 1.0)?.as_solid_rgb()
    };

    for (row_idx, row) in table.rows.iter().enumerate() {
        let is_header = (row_idx as u32) < header_rows;
        for cell in &row.cells {
            let cell_bg = if let Some(rgb) = resolve_fill(&cell.fill) {
                Some(rgb)
            } else if is_header {
                resolve_fill(&table.header_fill)
                    .or_else(|| resolve_fill(&table.fill))
                    .or(page_bg_rgb)
            } else {
                resolve_fill(&table.fill).or(page_bg_rgb)
            };
            // The scene paints the table `header_style` on each header-cell
            // text that sets no `style`; the walk applies it as an override
            // on the cell's direct text children.
            let ctx = PaintCtx {
                clip: None,
                page_bg_rgb: cell_bg,
                header_style: table.header_style.as_deref().filter(|_| is_header),
                in_cell: true,
                ..parent
            };
            let mut candidates = Vec::new();
            walk_paint(&cell.children, ctx, &mut candidates, env, diagnostics);
        }
    }
}
