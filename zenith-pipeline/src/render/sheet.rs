//! Contact sheet: every selected page tiled into one PNG with page labels.
//!
//! Layout rule (all values in sheet pixels):
//! - `columns = ceil(sqrt(n))`, `rows = ceil(n / columns)` for `n` pages.
//! - Each cell is the largest scaled page: `cell_w = max(page_w)`,
//!   `cell_h = max(page_h)`.
//! - A fixed [`SHEET_GUTTER_PX`] gutter surrounds and separates cells. Under
//!   each cell sits a [`SHEET_LABEL_BAND_PX`] band with the 1-based page
//!   number, centered.
//! - `width = columns × cell_w + (columns + 1) × gutter`.
//! - `height = rows × (cell_h + label_band) + (rows + 1) × gutter`.
//! - Each page is centered in its cell (integer floor of the slack / 2).
//!   Pages fill row-major in page order.
//!
//! Scale: an explicit scale applies as is. Without one the sheet fits
//! [`SHEET_AUTO_MAX_WIDTH_PX`]: `scale = min(1, floor((2048 − (columns + 1) ×
//! gutter) / columns) / max_page_width)`. Pages rasterize at that scale (never
//! a resample), in parallel, and compose in page order, so output bytes are
//! deterministic.
//!
//! The background and labels are a generated one-page `.zen` document
//! compiled and rasterized through the normal pipeline with the bundled font.

use std::fmt::Write as _;
use std::path::Path;

use zenith_core::{BytesAssetProvider, Diagnostic, KdlAdapter, KdlSource};
use zenith_render::{RasterImage, composite_over, encode_png, render_image};
use zenith_scene::{Scene, compile_page};

use super::compile::PageSelection;
use super::options::RenderOptions;
use super::raster::rasterize_pages;
use crate::assets::build_font_provider;
use crate::error::PipelineError;
use crate::host::Host;
use crate::imports::ImportFiles;

/// Gutter between cells and around the sheet edge, in sheet pixels.
pub const SHEET_GUTTER_PX: u32 = 16;
/// Height of the page-number band under each cell, in sheet pixels.
pub const SHEET_LABEL_BAND_PX: u32 = 28;
/// Widest sheet the automatic scale produces, in pixels.
pub const SHEET_AUTO_MAX_WIDTH_PX: u32 = 2048;
/// Largest sheet side the raster backend accepts.
const SHEET_MAX_SIDE_PX: u32 = 16_384;
/// Label font size in sheet pixels.
const LABEL_FONT_PX: u32 = 14;
/// Gap between a cell's bottom edge and its label box.
const LABEL_TOP_GAP_PX: u32 = 4;

/// A rendered contact sheet plus the diagnostics of the whole render.
#[derive(Debug)]
pub struct ContactSheetArtifact {
    /// The encoded PNG bytes.
    pub png: Vec<u8>,
    /// Sheet width in pixels.
    pub width: u32,
    /// Sheet height in pixels.
    pub height: u32,
    /// The output scale every page was rasterized at.
    pub scale: f64,
    /// Grid columns.
    pub columns: u32,
    /// Grid rows.
    pub rows: u32,
    /// 1-based page numbers on the sheet, in tile order.
    pub pages: Vec<usize>,
    /// Validation, document, and page diagnostics, repeats removed.
    pub diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports, for locating diagnostic spans.
    pub import_files: ImportFiles,
}

/// Render the pages of `src` into one contact-sheet PNG.
///
/// `page` selects one 1-based page; `None` covers every page. `scale` is the
/// page raster scale; `None` picks the automatic fit-to-2048 scale (see the
/// module docs). `opts.scale` is ignored here.
///
/// # Errors
///
/// Any [`render_png_pages`](super::render_png_pages) failure, or
/// `render.raster_failed` when the sheet exceeds 16384 px per side or the
/// label layer fails.
pub fn render_contact_sheet(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    page: Option<usize>,
    scale: Option<f64>,
    opts: RenderOptions<'_>,
) -> Result<ContactSheetArtifact, PipelineError> {
    let selection = page.map_or(PageSelection::All, PageSelection::One);
    let choose = |scenes: &[&Scene]| match scale {
        Some(s) => Ok(s),
        None => auto_scale(scenes),
    };
    let rasters = rasterize_pages(host, src, project_dir, selection, opts, &choose)?;
    let sizes: Vec<(u32, u32)> = rasters.images.iter().map(|i| (i.width, i.height)).collect();
    let layout = SheetLayout::new(&sizes).map_err(sheet_err)?;
    let mut sheet = label_layer(host, &layout, &rasters.page_numbers).map_err(sheet_err)?;
    for (k, image) in rasters.images.iter().enumerate() {
        let (x, y) = layout.page_origin(k, image.width, image.height);
        composite_over(&mut sheet, image, x, y).map_err(|e| sheet_err(e.to_string()))?;
    }
    let png = encode_png(&sheet).map_err(|e| sheet_err(e.to_string()))?;
    Ok(ContactSheetArtifact {
        png,
        width: layout.width,
        height: layout.height,
        scale: rasters.scale,
        columns: layout.columns,
        rows: layout.rows,
        pages: rasters.page_numbers,
        diagnostics: rasters.diagnostics,
        import_files: rasters.import_files,
    })
}

fn sheet_err(msg: String) -> PipelineError {
    PipelineError::new(
        "render.raster_failed",
        format!("contact sheet error: {msg}"),
        2,
    )
}

/// `ceil(sqrt(n))` in integers: the smallest `c` with `c × c >= n` (min 1).
fn grid_columns(n: usize) -> u32 {
    let mut c: u32 = 1;
    while (c as usize).saturating_mul(c as usize) < n {
        c += 1;
    }
    c
}

/// The automatic sheet scale: fit `columns` of the widest page plus gutters
/// into [`SHEET_AUTO_MAX_WIDTH_PX`], never above 1.
fn auto_scale(scenes: &[&Scene]) -> Result<f64, PipelineError> {
    let columns = grid_columns(scenes.len());
    let max_w = scenes.iter().map(|s| s.width).fold(0.0_f64, f64::max);
    let gutters = (columns + 1).saturating_mul(SHEET_GUTTER_PX);
    let cell = SHEET_AUTO_MAX_WIDTH_PX.saturating_sub(gutters) / columns;
    if cell == 0 {
        return Err(sheet_err(format!(
            "{} pages do not fit a {SHEET_AUTO_MAX_WIDTH_PX} px sheet; pass --page N or an explicit --scale",
            scenes.len()
        )));
    }
    if !(max_w.is_finite() && max_w > 0.0) {
        // An invalid page size fails in the rasterizer with its own message.
        return Ok(1.0);
    }
    Ok((f64::from(cell) / max_w).min(1.0))
}

/// The resolved grid of a contact sheet (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SheetLayout {
    columns: u32,
    rows: u32,
    cell_w: u32,
    cell_h: u32,
    width: u32,
    height: u32,
}

impl SheetLayout {
    /// Lay out pages of the given scaled `(width, height)` sizes.
    fn new(sizes: &[(u32, u32)]) -> Result<Self, String> {
        let n = sizes.len();
        if n == 0 {
            return Err("no pages selected".to_owned());
        }
        let columns = grid_columns(n);
        let rows = u32::try_from(n.div_ceil(columns as usize))
            .map_err(|_| format!("{n} pages exceed the sheet row limit"))?;
        let cell_w = sizes.iter().map(|s| s.0).max().unwrap_or(1);
        let cell_h = sizes.iter().map(|s| s.1).max().unwrap_or(1);
        let side = |cells: u32, cell: u32, band: u32| -> Option<u32> {
            cells
                .checked_mul(cell.checked_add(band)?)?
                .checked_add((cells + 1).checked_mul(SHEET_GUTTER_PX)?)
        };
        let too_big = || {
            format!(
                "sheet for {n} page(s) of up to {cell_w}x{cell_h} px exceeds \
                 {SHEET_MAX_SIDE_PX} px per side; lower --scale or pass --page N"
            )
        };
        let width = side(columns, cell_w, 0).ok_or_else(too_big)?;
        let height = side(rows, cell_h, SHEET_LABEL_BAND_PX).ok_or_else(too_big)?;
        if width > SHEET_MAX_SIDE_PX || height > SHEET_MAX_SIDE_PX {
            return Err(too_big());
        }
        Ok(Self {
            columns,
            rows,
            cell_w,
            cell_h,
            width,
            height,
        })
    }

    /// Top-left corner of cell `k` (row-major).
    fn cell_origin(&self, k: usize) -> (u32, u32) {
        let k = u32::try_from(k).unwrap_or(u32::MAX);
        let (col, row) = (k % self.columns, k / self.columns);
        let x = SHEET_GUTTER_PX + col * (self.cell_w + SHEET_GUTTER_PX);
        let y = SHEET_GUTTER_PX + row * (self.cell_h + SHEET_LABEL_BAND_PX + SHEET_GUTTER_PX);
        (x, y)
    }

    /// Top-left corner of a `w × h` page centered in cell `k`.
    fn page_origin(&self, k: usize, w: u32, h: u32) -> (u32, u32) {
        let (x, y) = self.cell_origin(k);
        (
            x + self.cell_w.saturating_sub(w) / 2,
            y + self.cell_h.saturating_sub(h) / 2,
        )
    }
}

/// Rasterize the sheet background and page labels at sheet size.
fn label_layer(
    host: Host<'_>,
    layout: &SheetLayout,
    page_numbers: &[usize],
) -> Result<RasterImage, String> {
    let src = label_document(layout, page_numbers);
    let doc = KdlAdapter
        .parse(src.as_bytes())
        .map_err(|e| format!("label layer parse failed: {}", e.message))?;
    let fonts = build_font_provider(host, &doc, None, false).map_err(|e| e.message)?;
    let compiled = compile_page(&doc, &fonts, 0, None);
    if let Some(d) = compiled.diagnostics.iter().find(|d| d.is_error()) {
        return Err(format!(
            "label layer compile failed: [{}] {}",
            d.code, d.message
        ));
    }
    render_image(&compiled.scene, &fonts, &BytesAssetProvider::new())
        .map_err(|e| format!("label layer raster failed: {e}"))
}

/// The generated `.zen` source for the sheet background and labels.
fn label_document(layout: &SheetLayout, page_numbers: &[usize]) -> String {
    let (w, h) = (layout.width, layout.height);
    let mut nodes = String::new();
    for (k, page) in page_numbers.iter().enumerate() {
        let (x, y) = layout.cell_origin(k);
        let label_y = y + layout.cell_h + LABEL_TOP_GAP_PX;
        let label_h = SHEET_LABEL_BAND_PX - LABEL_TOP_GAP_PX;
        // Writing to a String cannot fail.
        let _ = writeln!(
            nodes,
            r#"      text id="sheet.label.{page}" x=(px){x} y=(px){label_y} w=(px){cw} h=(px){label_h} align="center" fill=(token)"color.label" font-family=(token)"font.label" font-size=(token)"size.label" {{ span "{page}" }}"#,
            cw = layout.cell_w,
        );
    }
    format!(
        r##"zenith version=1 {{
  project id="proj.contact_sheet" name="Contact sheet"
  tokens format="zenith-token-v1" {{
    token id="color.sheet" type="color" value="#E5E7EB"
    token id="color.label" type="color" value="#374151"
    token id="font.label" type="fontFamily" value="Noto Sans"
    token id="size.label" type="dimension" value=(px){LABEL_FONT_PX}
  }}
  styles {{}}
  document id="doc.contact_sheet" title="Contact sheet" {{
    page id="sheet" w=(px){w} h=(px){h} {{
      rect id="sheet.bg" x=(px)0 y=(px)0 w=(px){w} h=(px){h} fill=(token)"color.sheet"
{nodes}    }}
  }}
}}
"##
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_are_ceil_sqrt() {
        let got: Vec<u32> = [1, 2, 3, 4, 5, 7, 9, 10, 16, 17]
            .iter()
            .map(|&n| grid_columns(n))
            .collect();
        assert_eq!(got, vec![1, 2, 2, 2, 3, 3, 3, 4, 4, 5]);
    }

    #[test]
    fn layout_sizes_follow_the_rule() {
        // 7 pages of 960x540 → 3 columns x 3 rows.
        let l = SheetLayout::new(&[(960, 540); 7]).unwrap();
        assert_eq!((l.columns, l.rows), (3, 3));
        assert_eq!(l.width, 3 * 960 + 4 * 16);
        assert_eq!(l.height, 3 * (540 + 28) + 4 * 16);
    }

    #[test]
    fn mixed_sizes_center_in_the_largest_cell() {
        let l = SheetLayout::new(&[(100, 50), (60, 80)]).unwrap();
        assert_eq!((l.cell_w, l.cell_h), (100, 80));
        assert_eq!(l.page_origin(0, 100, 50), (16, 16 + 15));
        assert_eq!(l.page_origin(1, 60, 80), (16 + 100 + 16 + 20, 16));
    }

    #[test]
    fn oversized_sheet_is_an_error() {
        let err = SheetLayout::new(&[(9000, 100); 4]).unwrap_err();
        assert!(err.contains("lower --scale"), "{err}");
    }

    #[test]
    fn label_document_parses() {
        let l = SheetLayout::new(&[(200, 100); 3]).unwrap();
        let src = label_document(&l, &[1, 2, 3]);
        assert!(KdlAdapter.parse(src.as_bytes()).is_ok(), "{src}");
    }
}
