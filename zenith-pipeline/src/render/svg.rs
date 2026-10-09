//! SVG entry points and ordered raster fallback diagnostics.

use std::path::Path;

use zenith_core::{BytesAssetProvider, Diagnostic};
use zenith_render::{SvgOptions, SvgRasterizedRegion, render_svg_with_options};
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

/// One self-contained RGB SVG with outlined text.
#[derive(Debug)]
pub struct SvgArtifact {
    /// The SVG bytes.
    pub svg: Vec<u8>,
    /// Page width in px.
    pub width: f64,
    /// Page height in px.
    pub height: f64,
    /// One-based document page number.
    pub page: usize,
    /// Command ranges rasterized because SVG cannot express them.
    pub rasterized_regions: Vec<SvgRasterizedRegion>,
    /// Every diagnostic of the export.
    pub diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports, for locating diagnostic spans.
    pub import_files: ImportFiles,
}

/// One SVG page without shared document diagnostics or import files.
#[derive(Debug)]
pub struct SvgPageArtifact {
    /// The SVG bytes.
    pub svg: Vec<u8>,
    /// Page width in px.
    pub width: f64,
    /// Page height in px.
    pub height: f64,
    /// One-based document page number.
    pub page: usize,
    /// Command ranges rasterized because SVG cannot express them.
    pub rasterized_regions: Vec<SvgRasterizedRegion>,
}

/// SVG pages in document order with the complete diagnostic set.
#[derive(Debug)]
pub struct SvgPagesArtifact {
    /// One entry per exported page.
    pub pages: Vec<SvgPageArtifact>,
    /// Every diagnostic of the export.
    pub diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports, for locating diagnostic spans.
    pub import_files: ImportFiles,
}

/// Export 1-based `page` of `src` as SVG.
///
/// # Errors
///
/// An invalid `opts.raster_scale`, a config or parse error, validation
/// errors, an out-of-range page, a locked asset failure, any Error
/// diagnostic from compile or export (exit 2), or an SVG export error.
pub fn render_svg(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    opts: RenderOptions<'_>,
) -> Result<SvgArtifact, PipelineError> {
    let result = render_pages(host, src, project_dir, Some(page), opts)?;
    let artifact = result.pages.into_iter().next().ok_or_else(|| {
        PipelineError::new(
            "render.no_pages",
            "document has no pages to export; add a page node",
            2,
        )
    })?;
    Ok(SvgArtifact {
        svg: artifact.svg,
        width: artifact.width,
        height: artifact.height,
        page: artifact.page,
        rasterized_regions: artifact.rasterized_regions,
        diagnostics: result.diagnostics,
        import_files: result.import_files,
    })
}

/// Export every page of `src` as SVG, in document order.
///
/// # Errors
///
/// As [`render_svg`], plus `render.no_pages` for an empty document.
pub fn render_svg_pages(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    opts: RenderOptions<'_>,
) -> Result<SvgPagesArtifact, PipelineError> {
    render_pages(host, src, project_dir, None, opts)
}

fn render_pages(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    page: Option<usize>,
    opts: RenderOptions<'_>,
) -> Result<SvgPagesArtifact, PipelineError> {
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
    let mut compiled = map_slice(host.runner, &indices, |&index| {
        compile_local_for_render(&doc, &compiler, index, opts)
    });
    for result in &mut compiled {
        diagnostics.extend(govern_compile_diagnostics(
            std::mem::take(&mut result.diagnostics),
            &policy,
        ));
    }
    // Stop before export when compilation or imported resources block the document.
    if Diagnostic::has_errors(&diagnostics) {
        return Err(PipelineError::blocked(diagnostics, 2).with_import_files(import_files));
    }
    let mut pages = Vec::with_capacity(indices.len());
    for (result, index) in compiled.into_iter().zip(indices) {
        let page = index + 1;
        let output = render_svg_with_options(
            &result.scene,
            &fonts,
            &assets,
            SvgOptions {
                raster_scale: opts.raster_scale,
            },
        )
        .map_err(|e| {
            let mut all = std::mem::take(&mut diagnostics);
            all.push(Diagnostic::error(
                "render.svg_failed",
                format!(
                    "SVG export failed on page {page}: {e}; check page resources and scene commands"
                ),
                None,
                None,
            ));
            PipelineError::blocked(all, 2).with_import_files(import_files.clone())
        })?;
        diagnostics.extend(govern_compile_diagnostics(
            rasterization_diagnostics(&output.rasterized_regions, page),
            &policy,
        ));
        pages.push(SvgPageArtifact {
            svg: output.bytes,
            width: result.scene.width,
            height: result.scene.height,
            page,
            rasterized_regions: output.rasterized_regions,
        });
    }
    let diagnostics = Diagnostic::dedup(diagnostics);
    if Diagnostic::has_errors(&diagnostics) {
        return Err(PipelineError::blocked(diagnostics, 2).with_import_files(import_files));
    }
    Ok(SvgPagesArtifact {
        pages,
        diagnostics,
        import_files,
    })
}

/// One `render.svg_rasterized` advisory per rasterized command range of
/// `page`, in order.
#[must_use]
pub fn rasterization_diagnostics(regions: &[SvgRasterizedRegion], page: usize) -> Vec<Diagnostic> {
    regions
        .iter()
        .map(|region| {
            Diagnostic::advisory(
                "render.svg_rasterized",
                format!(
                    "page {page}: commands [{}..{}) rasterized: {:?}; links in this range lose click targets",
                    region.command_start, region.command_end, region.reason
                ),
                None,
                None,
            )
        })
        .collect()
}
