//! Strict PDF entry points and ordered raster fallback diagnostics.

use std::path::Path;

use zenith_core::{BytesAssetProvider, Diagnostic};
use zenith_render::{PdfExportOptions, PdfRasterizedRegion, render_pdf_multi_report_with_options};
use zenith_scene::{DocumentPrep, PageCompiler};

use super::compile::compile_local_for_render;
use super::options::RenderOptions;
use super::scale::check_vector_raster_scale;
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

/// PDF bytes and export diagnostics. Page numbers refer to the source
/// document.
#[derive(Debug)]
pub struct PdfArtifact {
    /// The PDF bytes.
    pub pdf: Vec<u8>,
    /// Command ranges rasterized because PDF output cannot express them.
    pub rasterized_regions: Vec<PdfRasterizedRegion>,
    /// Every diagnostic of the export.
    pub diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports, for locating diagnostic spans.
    pub import_files: ImportFiles,
}

/// Export 1-based `page` of `src` through the strict PDF renderer.
///
/// # Errors
///
/// An invalid `opts.raster_scale`, a config or parse error, validation
/// errors, an out-of-range page, a locked asset failure, any Error
/// diagnostic from compile or export (exit 2), or a PDF export error.
pub fn render_pdf(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    opts: RenderOptions<'_>,
) -> Result<PdfArtifact, PipelineError> {
    render_pages(host, src, project_dir, Some(page), opts)
}

/// Export every page of `src` into one strict PDF.
///
/// # Errors
///
/// As [`render_pdf`], plus `render.no_pages` for an empty document.
pub fn render_pdf_pages(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    opts: RenderOptions<'_>,
) -> Result<PdfArtifact, PipelineError> {
    render_pages(host, src, project_dir, None, opts)
}

fn render_pages(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    page: Option<usize>,
    opts: RenderOptions<'_>,
) -> Result<PdfArtifact, PipelineError> {
    check_vector_raster_scale(opts.raster_scale)?;
    let ValidatedParts {
        mut doc,
        policy,
        imports,
        mut diagnostics,
        import_diagnostics,
        import_files,
    } = parse_validate_with(host, src, opts.parsed, project_dir, opts.flags)?.into_parts();
    let indices = match page {
        Some(page) => vec![
            resolve_page_index(&doc, page)
                .map_err(|e| e.with_import_files(import_files.clone()))?,
        ],
        None => {
            if doc.body.pages.is_empty() {
                return Err(PipelineError::new(
                    "render.no_pages",
                    "document has no pages to export; add a page node",
                    2,
                )
                .with_import_files(import_files));
            }
            (0..doc.body.pages.len()).collect()
        }
    };
    resolve_text_sources(host.fs, &mut doc, project_dir, &mut diagnostics);
    diagnostics.extend(import_diagnostics);
    let fonts = build_font_provider_with_imports(host, &doc, project_dir, &imports, opts.locked)
        .map_err(|e| e.with_import_files(import_files.clone()))?;
    let assets = match project_dir {
        Some(dir) => build_asset_provider_with_imports(host.fs, &doc, dir, &imports, opts.locked)
            .map_err(|e| e.with_import_files(import_files.clone()))?,
        None => BytesAssetProvider::new(),
    };
    diagnostics.extend(disk_diagnostics_with_imports(
        host.fs,
        &doc,
        project_dir,
        &imports,
    ));
    let scene_imports = imports.to_scene_graph();
    let prep = DocumentPrep::new(&doc, opts.data, Some(&scene_imports))
        .with_image_sizes(image_sizes(&doc, Some(&imports), &assets));
    let compiler = PageCompiler::new(&prep, &fonts);
    diagnostics.extend(govern_compile_diagnostics(
        compiler.document_diagnostics(),
        &policy,
    ));
    let compiled = map_slice(host.runner, &indices, |&index| {
        compile_local_for_render(&doc, &compiler, index, opts)
    });
    let mut scenes = Vec::with_capacity(compiled.len());
    for result in compiled {
        diagnostics.extend(govern_compile_diagnostics(result.diagnostics, &policy));
        scenes.push(result.scene);
    }
    if Diagnostic::has_errors(&diagnostics) {
        return Err(PipelineError::blocked(diagnostics, 2).with_import_files(import_files));
    }
    let mut output = render_pdf_multi_report_with_options(
        &scenes,
        &fonts,
        &assets,
        PdfExportOptions {
            subset: opts.subset,
            raster_scale: opts.raster_scale,
        },
    )
    .map_err(|e| {
        let mut all = std::mem::take(&mut diagnostics);
        let context = page.map_or_else(
            || "PDF export failed".to_owned(),
            |page| format!("PDF export failed on document page {page} (export page 1)"),
        );
        all.push(Diagnostic::error(
            "render.pdf_failed",
            format!("{context}: {e}; check page resources and scene commands"),
            None,
            None,
        ));
        PipelineError::blocked(all, 2).with_import_files(import_files.clone())
    })?;
    for region in &mut output.rasterized_regions {
        if let Some(&index) = region.page.checked_sub(1).and_then(|n| indices.get(n)) {
            region.page = index + 1;
        }
    }
    diagnostics.extend(govern_compile_diagnostics(
        pdf_rasterization_diagnostics(&output.rasterized_regions),
        &policy,
    ));
    let diagnostics = Diagnostic::dedup(diagnostics);
    if Diagnostic::has_errors(&diagnostics) {
        return Err(PipelineError::blocked(diagnostics, 2).with_import_files(import_files));
    }
    Ok(PdfArtifact {
        pdf: output.bytes,
        rasterized_regions: output.rasterized_regions,
        diagnostics,
        import_files,
    })
}

/// One `render.pdf_rasterized` advisory per rasterized command range, in
/// order. `region.page` is a source document page.
fn pdf_rasterization_diagnostics(regions: &[PdfRasterizedRegion]) -> Vec<Diagnostic> {
    regions
        .iter()
        .map(|region| {
            Diagnostic::advisory(
                "render.pdf_rasterized",
                format!(
                    "page {}: commands [{}..{}) rasterized: {:?}; text and links in this range lose selection and click targets",
                    region.page, region.command_start, region.command_end, region.reason
                ),
                None,
                None,
            )
        })
        .collect()
}
