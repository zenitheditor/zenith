//! Page compile shared by every entry point, and [`compile_pages`]: compiled
//! scenes (and optional node boxes) without raster.

use std::collections::BTreeMap;
use std::path::Path;

use zenith_core::{BytesAssetProvider, BytesFontProvider, Diagnostic, Document};
use zenith_scene::{
    CompileResult, CompiledBox, DocumentPrep, PageCompiler, Scene, append_construction_overlay,
};

use super::options::RenderOptions;
use crate::assets::{
    build_asset_provider_with_imports, build_font_provider_with_imports,
    disk_diagnostics_with_imports, image_sizes, resolve_text_sources,
};
use crate::error::PipelineError;
use crate::host::Host;
use crate::imports::ImportFiles;
use crate::io::map_slice;
use crate::prepare::{
    ValidatedParts, govern_compile_diagnostics, parse_validate_with, resolve_page_index,
};

/// Compile `page_index` with the document diagnostics, then append the
/// construction overlay when `opts` asks for it.
pub(crate) fn compile_for_render(
    doc: &Document,
    compiler: &PageCompiler<'_, BytesFontProvider>,
    page_index: usize,
    opts: RenderOptions<'_>,
) -> CompileResult {
    with_overlay(doc, compiler.compile_page(page_index), page_index, opts)
}

/// [`compile_for_render`] with only the page's own diagnostics. Multi-page
/// paths read [`PageCompiler::document_diagnostics`] once instead.
pub(crate) fn compile_local_for_render(
    doc: &Document,
    compiler: &PageCompiler<'_, BytesFontProvider>,
    page_index: usize,
    opts: RenderOptions<'_>,
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
    opts: RenderOptions<'_>,
) -> CompileResult {
    if opts.construction_overlay
        && let Some(page) = doc.body.pages.get(page_index)
    {
        append_construction_overlay(&mut compile_result.scene, page);
    }
    compile_result
}

/// Which pages a multi-page call covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageSelection {
    /// Every page, in document order.
    All,
    /// One 1-based page number.
    One(usize),
}

/// One compiled page.
#[derive(Debug)]
pub struct CompiledPage {
    /// The 1-based page number.
    pub page: usize,
    /// The compiled scene, construction overlay included when asked.
    pub scene: Scene,
    /// The final geometry of every compiled node, by id (see
    /// [`PageCompiler::compile_page_with_boxes`]), when boxes were asked for.
    pub boxes: Option<BTreeMap<String, CompiledBox>>,
}

/// Compiled pages plus everything a raster or vector backend needs.
#[derive(Debug)]
pub struct CompiledPages {
    /// One entry per selected page, in selection order.
    pub pages: Vec<CompiledPage>,
    /// The number of pages in the document.
    pub page_count: usize,
    /// Validation diagnostics, text-source, import, and disk diagnostics,
    /// document diagnostics once, then each page's own, in page order.
    /// Repeats are removed.
    pub diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports, for locating diagnostic spans.
    pub import_files: ImportFiles,
    /// The font provider the pages compiled with.
    pub fonts: BytesFontProvider,
    /// The image and SVG assets the pages reference.
    pub assets: BytesAssetProvider,
}

/// Parse, validate, and compile the `selection` of pages of `src`, without
/// raster.
///
/// Pages compile through `host.runner`; results stay in page order. With
/// `boxes`, each page also records its node boxes (page lint stays on, so
/// diagnostics match a render).
///
/// # Errors
///
/// Parse failure (exit 2), validation errors (exit 1), an empty document or
/// out-of-range page (exit 2), or an asset or font failure (exit 2).
pub fn compile_pages(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    selection: PageSelection,
    opts: RenderOptions<'_>,
    boxes: bool,
) -> Result<CompiledPages, PipelineError> {
    let ValidatedParts {
        mut doc,
        policy,
        imports,
        diagnostics: validation,
        import_diagnostics,
        import_files,
    } = parse_validate_with(host, src, opts.parsed, project_dir, opts.flags)?.into_parts();
    let scene_imports = imports.to_scene_graph();
    let mut diagnostics: Vec<Diagnostic> = validation;
    resolve_text_sources(host.fs, &mut doc, project_dir, &mut diagnostics);
    let fonts = build_font_provider_with_imports(host, &doc, project_dir, &imports, opts.locked)?;
    let page_count = doc.body.pages.len();
    if page_count == 0 {
        return Err(PipelineError::new(
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
        Some(dir) => build_asset_provider_with_imports(host.fs, &doc, dir, &imports, opts.locked)?,
        None => BytesAssetProvider::new(),
    };
    diagnostics.extend(import_diagnostics);
    diagnostics.extend(disk_diagnostics_with_imports(
        host.fs,
        &doc,
        project_dir,
        &imports,
    ));
    let prep = DocumentPrep::new(&doc, opts.data, Some(&scene_imports))
        .with_image_sizes(image_sizes(&doc, Some(&imports), &assets));
    let compiler = PageCompiler::new(&prep, &fonts);
    diagnostics.extend(govern_compile_diagnostics(
        compiler.document_diagnostics(),
        &policy,
    ));
    let compiled = map_slice(host.runner, &indices, |&page_index| {
        if boxes {
            let (result, recorded) = compiler.compile_page_with_boxes(page_index, true);
            (with_overlay(&doc, result, page_index, opts), Some(recorded))
        } else {
            (
                compile_local_for_render(&doc, &compiler, page_index, opts),
                None,
            )
        }
    });
    let mut pages = Vec::with_capacity(compiled.len());
    for ((result, recorded), page_index) in compiled.into_iter().zip(&indices) {
        diagnostics.extend(govern_compile_diagnostics(result.diagnostics, &policy));
        pages.push(CompiledPage {
            page: page_index + 1,
            scene: result.scene,
            boxes: recorded,
        });
    }
    Ok(CompiledPages {
        pages,
        page_count,
        diagnostics: Diagnostic::dedup(diagnostics),
        import_files,
        fonts,
        assets,
    })
}
