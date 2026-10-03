//! Page compile helpers shared by the render entry points.
//!
//! Every entry point builds one [`DocumentPrep`] and one [`PageCompiler`] per
//! invocation and compiles its pages from them. Multi-page paths map pages on
//! a bounded thread pool and collect the results in page order.

use std::path::Path;

use rayon::prelude::*;
use zenith_core::{BytesAssetProvider, BytesFontProvider, Diagnostic, Document};
use zenith_render::{RasterImage, render_image_scaled};
use zenith_scene::{CompileResult, DocumentPrep, PageCompiler, Scene, append_construction_overlay};

use crate::report::ImportFiles;

use super::assets::{
    build_asset_provider_with_imports, build_font_provider_with_imports,
    disk_diagnostics_with_imports, image_sizes,
};
use super::entry::{RenderCmdErr, RenderEntryOptions};
use super::pipeline::{
    ValidatedParts, govern_compile_diagnostics, parse_validate, resolve_page_index,
};
use super::text_source::resolve_text_sources;

/// Upper bound on page worker threads.
const MAX_PAGE_THREADS: usize = 8;

/// Compile `page_index` from `compiler`, then append the construction overlay
/// when `opts` asks for it.
///
/// The result carries the document diagnostics too (see
/// [`PageCompiler::compile_page`]). The overlay reads the page from `doc`, the
/// caller's parsed document.
pub(super) fn compile_for_render(
    doc: &Document,
    compiler: &PageCompiler<'_, BytesFontProvider>,
    page_index: usize,
    opts: RenderEntryOptions<'_>,
) -> CompileResult {
    with_overlay(doc, compiler.compile_page(page_index), page_index, opts)
}

/// [`compile_for_render`] with only the page's own diagnostics.
///
/// Multi-page paths read [`PageCompiler::document_diagnostics`] once instead.
pub(super) fn compile_local_for_render(
    doc: &Document,
    compiler: &PageCompiler<'_, BytesFontProvider>,
    page_index: usize,
    opts: RenderEntryOptions<'_>,
) -> CompileResult {
    with_overlay(
        doc,
        compiler.compile_page_local(page_index),
        page_index,
        opts,
    )
}

fn with_overlay(
    doc: &Document,
    mut compile_result: CompileResult,
    page_index: usize,
    opts: RenderEntryOptions<'_>,
) -> CompileResult {
    if opts.construction_overlay
        && let Some(page) = doc.body.pages.get(page_index)
    {
        append_construction_overlay(&mut compile_result.scene, page);
    }
    compile_result
}

/// Which pages a multi-page raster covers.
#[derive(Clone, Copy, Debug)]
pub(super) enum PageSelection {
    /// Every page, in document order.
    All,
    /// One 1-based page number.
    One(usize),
}

/// Rasterized pages plus the diagnostics of the whole render.
pub(super) struct PageRasters {
    /// One image per selected page, in selection order.
    pub(super) images: Vec<RasterImage>,
    /// 1-based page numbers of `images`, in the same order.
    pub(super) page_numbers: Vec<usize>,
    /// The output scale every page was rasterized at.
    pub(super) scale: f64,
    /// Validation diagnostics, document diagnostics once, then each page's
    /// own, in page order. Repeats are removed.
    pub(super) diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports, for locating diagnostic spans.
    pub(super) import_files: ImportFiles,
}

/// Parse, validate, compile, and rasterize the `selection` of pages of `src`.
///
/// Pages compile in parallel, then `choose_scale` sees every compiled scene
/// and returns the output scale, then pages rasterize in parallel at that
/// scale. Results merge in page order, so the first error reported is the
/// lowest failing page. Document-level diagnostics are reported once.
///
/// Returns `Err` on parse failure (exit 2), validation errors (exit 1), an
/// empty document or out-of-range page (exit 2), an asset/font failure
/// (exit 2), a `choose_scale` error, or a raster failure (exit 2).
pub(super) fn rasterize_pages(
    src: &str,
    project_dir: Option<&Path>,
    selection: PageSelection,
    opts: RenderEntryOptions<'_>,
    choose_scale: &dyn Fn(&[&Scene]) -> Result<f64, RenderCmdErr>,
) -> Result<PageRasters, RenderCmdErr> {
    let ValidatedParts {
        mut doc,
        policy,
        imports,
        diagnostics: validation,
        import_diagnostics,
        import_files,
    } = parse_validate(src, project_dir, opts.flags)?.into_parts();
    let scene_imports = imports.to_scene_graph();
    let mut diagnostics: Vec<Diagnostic> = validation;
    resolve_text_sources(&mut doc, project_dir, &mut diagnostics);
    let fonts = build_font_provider_with_imports(&doc, project_dir, &imports, opts.locked)?;
    let page_count = doc.body.pages.len();
    if page_count == 0 {
        return Err(RenderCmdErr::new(
            "render.no_pages",
            "document has no pages to render; add a page node",
            2,
        ));
    }
    let indices: Vec<usize> = match selection {
        PageSelection::All => (0..page_count).collect(),
        PageSelection::One(page) => vec![resolve_page_index(&doc, page)?],
    };
    let assets = match project_dir {
        Some(dir) => build_asset_provider_with_imports(&doc, dir, &imports, opts.locked)?,
        None => BytesAssetProvider::new(),
    };
    diagnostics.extend(import_diagnostics);
    diagnostics.extend(disk_diagnostics_with_imports(&doc, project_dir, &imports));
    let prep = DocumentPrep::new(&doc, opts.data, Some(&scene_imports))
        .with_image_sizes(image_sizes(&doc, Some(&imports), &assets));
    let compiler = PageCompiler::new(&prep, &fonts);
    diagnostics.extend(govern_compile_diagnostics(
        compiler.document_diagnostics(),
        &policy,
    ));
    let compiled = map_slice(&indices, |&page_index| {
        compile_local_for_render(&doc, &compiler, page_index, opts)
    });
    let scenes: Vec<&Scene> = compiled.iter().map(|c| &c.scene).collect();
    let scale = choose_scale(&scenes)?;
    let rastered = map_slice(&compiled, |c| {
        render_image_scaled(&c.scene, scale, &fonts, &assets)
    });
    let mut images = Vec::with_capacity(indices.len());
    for ((compile_result, image), page_index) in compiled.into_iter().zip(rastered).zip(&indices) {
        let image = image.map_err(|e| {
            RenderCmdErr::new(
                "render.raster_failed",
                format!("render error on page {}: {e}", page_index + 1),
                2,
            )
        })?;
        images.push(image);
        diagnostics.extend(govern_compile_diagnostics(
            compile_result.diagnostics,
            &policy,
        ));
    }
    Ok(PageRasters {
        images,
        page_numbers: indices.iter().map(|i| i + 1).collect(),
        scale,
        diagnostics: Diagnostic::dedup(diagnostics),
        import_files,
    })
}

/// Run `f` on every item of `items` on the page pool (see [`map_pages`]) and
/// return the results in item order.
pub(super) fn map_slice<I, T, F>(items: &[I], f: F) -> Vec<T>
where
    I: Sync,
    T: Send,
    F: Fn(&I) -> T + Sync + Send,
{
    map_pages(items.len(), |i| items.get(i).map(&f))
        .into_iter()
        .flatten()
        .collect()
}

/// Run `f` for every page index in `0..page_count` and return the results in
/// page order.
///
/// Pages run on a scoped pool of `min(available_parallelism, 8, page_count)`
/// threads. The result vector is indexed by page, so output never depends on
/// thread scheduling. One page, one available thread, or a pool that fails to
/// start runs the pages in sequence on this thread with the same result.
pub(super) fn map_pages<T, F>(page_count: usize, f: F) -> Vec<T>
where
    T: Send,
    F: Fn(usize) -> T + Sync + Send,
{
    let threads = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1)
        .min(MAX_PAGE_THREADS)
        .min(page_count);
    if threads <= 1 {
        return (0..page_count).map(f).collect();
    }
    match rayon::ThreadPoolBuilder::new().num_threads(threads).build() {
        Ok(pool) => pool.install(|| (0..page_count).into_par_iter().map(&f).collect()),
        Err(_) => (0..page_count).map(f).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_pages_keeps_page_order() {
        let out = map_pages(64, |i| i * 3);
        let expected: Vec<usize> = (0..64).map(|i| i * 3).collect();
        assert_eq!(out, expected);
    }

    #[test]
    fn map_pages_handles_zero_and_one_page() {
        assert!(map_pages(0, |i| i).is_empty());
        assert_eq!(map_pages(1, |i| i + 7), vec![7]);
    }
}
