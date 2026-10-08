//! Strict PDF export and ordered raster fallback diagnostics.

use std::path::Path;
use zenith_core::{BytesAssetProvider, DataContext, Diagnostic};
use zenith_render::{PdfExportOptions, PdfRasterizedRegion, render_pdf_multi_report_with_options};
use zenith_scene::{DocumentPrep, PageCompiler};

use super::assets::{
    build_asset_provider_with_imports, build_font_provider_with_imports,
    disk_diagnostics_with_imports, image_sizes,
};
use super::entry::{RenderCmdErr, RenderEntryOptions};
use super::pages::{compile_local_for_render, map_slice};
use super::pipeline::{
    ValidatedParts, govern_compile_diagnostics, parse_validate, resolve_page_index,
};
use super::text_source::resolve_text_sources;
use crate::config::CliPolicyFlags;
use crate::report::ImportFiles;

/// PDF bytes and export diagnostics. Page numbers refer to the source document.
#[derive(Debug)]
pub struct PdfArtifact {
    pub pdf: Vec<u8>,
    pub rasterized_regions: Vec<PdfRasterizedRegion>,
    pub diagnostics: Vec<Diagnostic>,
    pub import_files: ImportFiles,
}

/// Export one document page through the strict PDF renderer.
pub fn to_pdf_with_dir(
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    locked: bool,
    subset: bool,
    flags: &CliPolicyFlags,
    data: Option<&DataContext>,
) -> Result<PdfArtifact, RenderCmdErr> {
    to_pdf_with_dir_options(
        src,
        project_dir,
        page,
        RenderEntryOptions::pdf(flags, locked, subset, data),
    )
}

pub fn to_pdf_with_dir_options(
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    opts: RenderEntryOptions<'_>,
) -> Result<PdfArtifact, RenderCmdErr> {
    render_pages(src, project_dir, Some(page), opts)
}

/// Export every document page into one strict PDF.
pub fn to_pdf_all_pages_with_dir(
    src: &str,
    project_dir: Option<&Path>,
    locked: bool,
    subset: bool,
    flags: &CliPolicyFlags,
    data: Option<&DataContext>,
) -> Result<PdfArtifact, RenderCmdErr> {
    to_pdf_all_pages_with_dir_options(
        src,
        project_dir,
        RenderEntryOptions::pdf(flags, locked, subset, data),
    )
}

pub fn to_pdf_all_pages_with_dir_options(
    src: &str,
    project_dir: Option<&Path>,
    opts: RenderEntryOptions<'_>,
) -> Result<PdfArtifact, RenderCmdErr> {
    render_pages(src, project_dir, None, opts)
}

fn render_pages(
    src: &str,
    project_dir: Option<&Path>,
    page: Option<usize>,
    opts: RenderEntryOptions<'_>,
) -> Result<PdfArtifact, RenderCmdErr> {
    super::scale::check_vector_raster_scale(opts.raster_scale)?;
    let ValidatedParts {
        mut doc,
        policy,
        imports,
        mut diagnostics,
        import_diagnostics,
        import_files,
    } = parse_validate(src, project_dir, opts.flags)?.into_parts();
    let indices = match page {
        Some(page) => vec![
            resolve_page_index(&doc, page)
                .map_err(|e| e.with_import_files(import_files.clone()))?,
        ],
        None => {
            if doc.body.pages.is_empty() {
                return Err(RenderCmdErr::new(
                    "render.no_pages",
                    "document has no pages to export; add a page node",
                    2,
                )
                .with_import_files(import_files));
            }
            (0..doc.body.pages.len()).collect()
        }
    };
    resolve_text_sources(&mut doc, project_dir, &mut diagnostics);
    diagnostics.extend(import_diagnostics);
    let fonts = build_font_provider_with_imports(&doc, project_dir, &imports, opts.locked)
        .map_err(|e| e.with_import_files(import_files.clone()))?;
    let assets = match project_dir {
        Some(dir) => build_asset_provider_with_imports(&doc, dir, &imports, opts.locked)
            .map_err(|e| e.with_import_files(import_files.clone()))?,
        None => BytesAssetProvider::new(),
    };
    diagnostics.extend(disk_diagnostics_with_imports(&doc, project_dir, &imports));
    let scene_imports = imports.to_scene_graph();
    let prep = DocumentPrep::new(&doc, opts.data, Some(&scene_imports))
        .with_image_sizes(image_sizes(&doc, Some(&imports), &assets));
    let compiler = PageCompiler::new(&prep, &fonts);
    diagnostics.extend(govern_compile_diagnostics(
        compiler.document_diagnostics(),
        &policy,
    ));
    let compiled = map_slice(&indices, |&index| {
        compile_local_for_render(&doc, &compiler, index, opts)
    });
    let mut scenes = Vec::with_capacity(compiled.len());
    for result in compiled {
        diagnostics.extend(govern_compile_diagnostics(result.diagnostics, &policy));
        scenes.push(result.scene);
    }
    if Diagnostic::has_errors(&diagnostics) {
        return Err(RenderCmdErr::blocked(diagnostics, 2).with_import_files(import_files));
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
        RenderCmdErr::blocked(all, 2).with_import_files(import_files.clone())
    })?;
    for region in &mut output.rasterized_regions {
        if let Some(&index) = region.page.checked_sub(1).and_then(|n| indices.get(n)) {
            region.page = index + 1;
        }
    }
    diagnostics.extend(govern_compile_diagnostics(output.rasterized_regions.iter().map(|region| Diagnostic::advisory(
        "render.pdf_rasterized", format!("page {}: commands [{}..{}) rasterized: {:?}; text and links in this range lose selection and click targets", region.page, region.command_start, region.command_end, region.reason), None, None,
    )).collect(), &policy));
    let diagnostics = Diagnostic::dedup(diagnostics);
    if Diagnostic::has_errors(&diagnostics) {
        return Err(RenderCmdErr::blocked(diagnostics, 2).with_import_files(import_files));
    }
    Ok(PdfArtifact {
        pdf: output.bytes,
        rasterized_regions: output.rasterized_regions,
        diagnostics,
        import_files,
    })
}
