//! SVG render entry points and ordered fallback diagnostics.

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
use std::path::Path;
use zenith_core::{BytesAssetProvider, DataContext, Diagnostic};
use zenith_render::{SvgOptions, SvgRasterizedRegion, render_svg_with_options};
use zenith_scene::{DocumentPrep, PageCompiler};

/// One self-contained RGB SVG with outlined text.
#[derive(Debug)]
pub struct SvgArtifact {
    pub svg: Vec<u8>,
    pub width: f64,
    pub height: f64,
    /// One-based document page number.
    pub page: usize,
    pub rasterized_regions: Vec<SvgRasterizedRegion>,
    pub diagnostics: Vec<Diagnostic>,
    pub import_files: ImportFiles,
}

/// One SVG page without shared document diagnostics or import files.
#[derive(Debug)]
pub struct SvgPageArtifact {
    pub svg: Vec<u8>,
    pub width: f64,
    pub height: f64,
    /// One-based document page number.
    pub page: usize,
    pub rasterized_regions: Vec<SvgRasterizedRegion>,
}

/// SVG pages in document order with the complete diagnostic set.
#[derive(Debug)]
pub struct SvgPagesArtifact {
    pub pages: Vec<SvgPageArtifact>,
    pub diagnostics: Vec<Diagnostic>,
    pub import_files: ImportFiles,
}

pub fn to_svg_with_dir(
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    locked: bool,
    flags: &CliPolicyFlags,
    data: Option<&DataContext>,
) -> Result<SvgArtifact, RenderCmdErr> {
    to_svg_with_dir_options(
        src,
        project_dir,
        page,
        RenderEntryOptions::common(flags, locked, data),
    )
}

pub fn to_svg_all_pages_with_dir(
    src: &str,
    project_dir: Option<&Path>,
    locked: bool,
    flags: &CliPolicyFlags,
    data: Option<&DataContext>,
) -> Result<SvgPagesArtifact, RenderCmdErr> {
    to_svg_all_pages_with_dir_options(
        src,
        project_dir,
        RenderEntryOptions::common(flags, locked, data),
    )
}

pub fn to_svg_with_dir_options(
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    opts: RenderEntryOptions<'_>,
) -> Result<SvgArtifact, RenderCmdErr> {
    let result = render_pages(src, project_dir, Some(page), opts)?;
    let artifact = result.pages.into_iter().next().ok_or_else(|| {
        RenderCmdErr::new(
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

pub fn to_svg_all_pages_with_dir_options(
    src: &str,
    project_dir: Option<&Path>,
    opts: RenderEntryOptions<'_>,
) -> Result<SvgPagesArtifact, RenderCmdErr> {
    render_pages(src, project_dir, None, opts)
}

fn render_pages(
    src: &str,
    project_dir: Option<&Path>,
    page: Option<usize>,
    opts: RenderEntryOptions<'_>,
) -> Result<SvgPagesArtifact, RenderCmdErr> {
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
    let mut compiled = map_slice(&indices, |&index| {
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
        return Err(RenderCmdErr::blocked(diagnostics, 2).with_import_files(import_files));
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
            RenderCmdErr::blocked(all, 2).with_import_files(import_files.clone())
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
        return Err(RenderCmdErr::blocked(diagnostics, 2).with_import_files(import_files));
    }
    Ok(SvgPagesArtifact {
        pages,
        diagnostics,
        import_files,
    })
}

pub(super) fn rasterization_diagnostics(
    regions: &[SvgRasterizedRegion],
    page: usize,
) -> Vec<Diagnostic> {
    regions.iter().map(|region| Diagnostic::advisory(
        "render.svg_rasterized",
        format!("page {page}: commands [{}..{}) rasterized: {:?}; links in this range lose click targets", region.command_start, region.command_end, region.reason),
        None, None,
    )).collect()
}
