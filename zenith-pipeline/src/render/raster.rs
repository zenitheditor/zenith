//! Multi-page raster: compile the selected pages, choose a scale, rasterize.

use std::path::Path;

use zenith_core::Diagnostic;
use zenith_render::{RasterImage, render_image_scaled};
use zenith_scene::Scene;

use super::compile::{PageSelection, compile_pages};
use super::options::RenderOptions;
use crate::error::PipelineError;
use crate::host::Host;
use crate::imports::ImportFiles;
use crate::io::map_slice;

/// Rasterized pages plus the diagnostics of the whole render.
pub(crate) struct PageRasters {
    /// One image per selected page, in selection order.
    pub(crate) images: Vec<RasterImage>,
    /// 1-based page numbers of `images`, in the same order.
    pub(crate) page_numbers: Vec<usize>,
    /// The output scale every page was rasterized at.
    pub(crate) scale: f64,
    /// Validation diagnostics, document diagnostics once, then each page's
    /// own, in page order. Repeats are removed.
    pub(crate) diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports, for locating diagnostic spans.
    pub(crate) import_files: ImportFiles,
}

/// Compile and rasterize the `selection` of pages of `src`.
///
/// Pages compile through `host.runner`, then `choose_scale` sees every
/// compiled scene and returns the output scale, then pages rasterize through
/// `host.runner` at that scale. Results stay in page order, so the first
/// error reported is the lowest failing page.
///
/// # Errors
///
/// Every [`compile_pages`] error, a `choose_scale` error, or
/// `render.raster_failed` (exit 2).
pub(crate) fn rasterize_pages(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    selection: PageSelection,
    opts: RenderOptions<'_>,
    choose_scale: &dyn Fn(&[&Scene]) -> Result<f64, PipelineError>,
) -> Result<PageRasters, PipelineError> {
    let compiled = compile_pages(host, src, project_dir, selection, opts, false)?;
    let scenes: Vec<&Scene> = compiled.pages.iter().map(|p| &p.scene).collect();
    let scale = choose_scale(&scenes)?;
    let fonts = &compiled.fonts;
    let assets = &compiled.assets;
    let rastered = map_slice(host.runner, &compiled.pages, |p| {
        render_image_scaled(&p.scene, scale, fonts, assets)
    });
    let mut images = Vec::with_capacity(rastered.len());
    for (image, page) in rastered.into_iter().zip(&compiled.pages) {
        images.push(image.map_err(|e| {
            PipelineError::new(
                "render.raster_failed",
                format!("render error on page {}: {e}", page.page),
                2,
            )
        })?);
    }
    Ok(PageRasters {
        images,
        page_numbers: compiled.pages.iter().map(|p| p.page).collect(),
        scale,
        diagnostics: compiled.diagnostics,
        import_files: compiled.import_files,
    })
}
