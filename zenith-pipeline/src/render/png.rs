//! Scene JSON and PNG entry points: one page, every page, and a spread.

use std::path::Path;

use zenith_core::{BytesAssetProvider, BytesFontProvider, Diagnostic, dim_to_px};
use zenith_render::{composite_spread, encode_png, render_image_scaled};
use zenith_scene::{DocumentPrep, PageCompiler, Scene};

use super::compile::{PageSelection, compile_for_render, compile_local_for_render};
use super::options::{RenderOptions, SpreadOptions};
use super::raster::rasterize_pages;
use crate::assets::{
    build_asset_provider_with_imports, build_font_provider_with_imports,
    disk_diagnostics_with_imports, image_sizes, read_image_sizes, resolve_text_sources,
};
use crate::error::PipelineError;
use crate::host::Host;
use crate::imports::ImportFiles;
use crate::io::map_slice;
use crate::prepare::{
    ValidatedParts, govern_compile_diagnostics, parse_validate, parse_validate_with,
    resolve_page_index,
};

/// Scene JSON plus the diagnostics that produced it.
#[derive(Debug)]
pub struct SceneArtifact {
    /// The serialised scene JSON.
    pub json: String,
    /// Validation diagnostics, then compile-stage diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports, for locating diagnostic spans.
    pub import_files: ImportFiles,
}

/// PNG bytes plus the diagnostics that produced them.
#[derive(Debug)]
pub struct PngArtifact {
    /// The encoded PNG bytes.
    pub png: Vec<u8>,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// The number of pages in the document.
    pub page_count: usize,
    /// Validation diagnostics, then compile-stage diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports, for locating diagnostic spans.
    pub import_files: ImportFiles,
}

/// PNG bytes for every page plus the diagnostics of the whole render.
#[derive(Debug)]
pub struct PngPagesArtifact {
    /// Encoded PNG bytes, one entry per page in document order.
    pub pages: Vec<Vec<u8>>,
    /// `(width, height)` in pixels of each entry of `pages`.
    pub sizes: Vec<(u32, u32)>,
    /// Validation diagnostics, document diagnostics once, then each page's
    /// own, in page order. Repeats are removed.
    pub diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports, for locating diagnostic spans.
    pub import_files: ImportFiles,
}

/// Parse and validate `src` with the merged policy, compile 1-based `page`,
/// and return its scene JSON plus the diagnostics.
///
/// `project_dir` is the document's directory: it locates project fonts,
/// text sources, imports, and images, and starts the local config walk.
/// `None` uses only bundled fonts. Asset hashes are never checked here
/// (`opts.locked` does not apply); `opts.scale` and `opts.raster_scale` are
/// ignored.
///
/// # Errors
///
/// A config or parse error (exit 2), validation errors (exit 1), an
/// out-of-range page (exit 2), or a scene serialisation error (exit 2).
pub fn render_scene_json(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    opts: RenderOptions<'_>,
) -> Result<SceneArtifact, PipelineError> {
    let ValidatedParts {
        mut doc,
        policy,
        imports,
        diagnostics: validation,
        import_diagnostics,
        import_files,
    } = parse_validate_with(host, src, opts.parsed, project_dir, opts.flags)?.into_parts();
    let scene_imports = imports.to_scene_graph();
    let mut text_src_diagnostics: Vec<Diagnostic> = Vec::new();
    resolve_text_sources(host.fs, &mut doc, project_dir, &mut text_src_diagnostics);
    let fonts = build_font_provider_with_imports(host, &doc, project_dir, &imports, false)?;
    let page_index = resolve_page_index(&doc, page)?;
    let prep = DocumentPrep::new(&doc, opts.data, Some(&scene_imports))
        .with_image_sizes(read_image_sizes(host.fs, &doc, project_dir, &imports));
    let compiler = PageCompiler::new(&prep, &fonts);
    let compile_result = compile_for_render(&doc, &compiler, page_index, opts);
    let json = compile_result.scene.to_json().map_err(|e| {
        PipelineError::new(
            "render.scene_serialize_failed",
            format!("scene serialisation error: {e}"),
            2,
        )
    })?;
    let mut diagnostics = validation;
    diagnostics.extend(text_src_diagnostics);
    diagnostics.extend(import_diagnostics);
    diagnostics.extend(disk_diagnostics_with_imports(
        host.fs,
        &doc,
        project_dir,
        &imports,
    ));
    diagnostics.extend(govern_compile_diagnostics(
        compile_result.diagnostics,
        &policy,
    ));
    Ok(SceneArtifact {
        json,
        diagnostics: Diagnostic::dedup(diagnostics),
        import_files,
    })
}

/// Parse and validate `src` with the merged policy, compile 1-based `page`,
/// and rasterize it to PNG at `opts.scale`.
///
/// Image, SVG, and font assets come from `project_dir` through `host.fs`. A
/// missing file is a hard `asset.missing` diagnostic on the artifact. With
/// `opts.locked`, every asset's bytes must match its declared `sha256`.
///
/// # Errors
///
/// A config or parse error (exit 2), validation errors (exit 1), a locked
/// asset failure (exit 2), an out-of-range page (exit 2), or a raster error
/// (exit 2).
pub fn render_png(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    opts: RenderOptions<'_>,
) -> Result<PngArtifact, PipelineError> {
    let compiled = compile_png_page(host, src, project_dir, page, opts)?;
    let raster_err = |e: zenith_render::RenderError| {
        PipelineError::new("render.raster_failed", format!("render error: {e}"), 2)
    };
    let image = render_image_scaled(
        &compiled.scene,
        opts.scale,
        &compiled.fonts,
        &compiled.assets,
    )
    .map_err(raster_err)?;
    let png = encode_png(&image).map_err(raster_err)?;
    Ok(PngArtifact {
        png,
        width: image.width,
        height: image.height,
        page_count: compiled.page_count,
        diagnostics: compiled.diagnostics,
        import_files: compiled.import_files,
    })
}

/// One compiled page with everything its raster needs.
pub(super) struct PngPage {
    pub(super) scene: Scene,
    pub(super) fonts: BytesFontProvider,
    pub(super) assets: BytesAssetProvider,
    pub(super) page_count: usize,
    pub(super) diagnostics: Vec<Diagnostic>,
    pub(super) import_files: ImportFiles,
}

/// The compile half of [`render_png`]: parse, validate, load fonts and
/// assets, and compile 1-based `page`. The diagnostics are final (validation,
/// text sources, imports, disk, then the governed compile diagnostics).
///
/// # Errors
///
/// The non-raster errors of [`render_png`].
pub(super) fn compile_png_page(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    opts: RenderOptions<'_>,
) -> Result<PngPage, PipelineError> {
    let ValidatedParts {
        mut doc,
        policy,
        imports,
        diagnostics: validation,
        import_diagnostics,
        import_files,
    } = parse_validate_with(host, src, opts.parsed, project_dir, opts.flags)?.into_parts();
    let scene_imports = imports.to_scene_graph();
    let mut text_src_diagnostics: Vec<Diagnostic> = Vec::new();
    resolve_text_sources(host.fs, &mut doc, project_dir, &mut text_src_diagnostics);
    let fonts = build_font_provider_with_imports(host, &doc, project_dir, &imports, opts.locked)?;
    let page_index = resolve_page_index(&doc, page)?;
    let assets = match project_dir {
        Some(dir) => build_asset_provider_with_imports(host.fs, &doc, dir, &imports, opts.locked)?,
        None => BytesAssetProvider::new(),
    };
    let prep = DocumentPrep::new(&doc, opts.data, Some(&scene_imports))
        .with_image_sizes(image_sizes(&doc, Some(&imports), &assets));
    let compiler = PageCompiler::new(&prep, &fonts);
    let compile_result = compile_for_render(&doc, &compiler, page_index, opts);
    let mut diagnostics = validation;
    diagnostics.extend(text_src_diagnostics);
    diagnostics.extend(import_diagnostics);
    diagnostics.extend(disk_diagnostics_with_imports(
        host.fs,
        &doc,
        project_dir,
        &imports,
    ));
    diagnostics.extend(govern_compile_diagnostics(
        compile_result.diagnostics,
        &policy,
    ));
    let page_count = doc.body.pages.len();
    Ok(PngPage {
        scene: compile_result.scene,
        fonts,
        assets,
        page_count,
        diagnostics: Diagnostic::dedup(diagnostics),
        import_files,
    })
}

/// Parse and validate `src` with the merged policy and render every page to
/// PNG at `opts.scale`, in document order.
///
/// Assets load once for all pages. Document-level diagnostics are reported
/// once, not once per page.
///
/// # Errors
///
/// A config or parse error (exit 2), validation errors (exit 1), an empty
/// document (exit 2), a locked asset failure (exit 2), or a raster error
/// (exit 2).
pub fn render_png_pages(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    opts: RenderOptions<'_>,
) -> Result<PngPagesArtifact, PipelineError> {
    let rasters = rasterize_pages(host, src, project_dir, PageSelection::All, opts, &|_| {
        Ok(opts.scale)
    })?;
    // Pages encode through the runner; results stay in page order.
    let encoded = map_slice(host.runner, &rasters.images, encode_png);
    let mut pages = Vec::with_capacity(encoded.len());
    for (png, page) in encoded.into_iter().zip(&rasters.page_numbers) {
        pages.push(png.map_err(|e| {
            PipelineError::new(
                "render.raster_failed",
                format!("render error on page {page}: {e}"),
                2,
            )
        })?);
    }
    Ok(PngPagesArtifact {
        pages,
        sizes: rasters.images.iter().map(|i| (i.width, i.height)).collect(),
        diagnostics: rasters.diagnostics,
        import_files: rasters.import_files,
    })
}

/// Parse and validate `src` with the merged policy, compile 1-based pages
/// `page_a` and `page_b`, and composite them side by side (A left, B right)
/// into one PNG.
///
/// The canvas is `page_a + gutter + page_b` wide. The gutter is
/// `gutter_override` when `Some`, else the document's `spread-gutter`, else
/// 0, in page pixels scaled by `opts.scale`. Gutter columns are transparent.
///
/// # Errors
///
/// A config or parse error (exit 2), validation errors (exit 1), an
/// out-of-range page (exit 2), a locked asset failure (exit 2), or a raster or
/// composite error (`render.spread_failed`, exit 2).
pub fn render_png_spread(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    page_a: usize,
    page_b: usize,
    gutter_override: Option<u32>,
    opts: SpreadOptions<'_>,
) -> Result<PngArtifact, PipelineError> {
    let SpreadOptions {
        locked,
        flags,
        data,
        construction_overlay,
        scale,
    } = opts;
    let ValidatedParts {
        mut doc,
        policy,
        imports,
        diagnostics: validation,
        import_diagnostics,
        import_files,
    } = parse_validate(host, src, project_dir, flags)?.into_parts();
    let scene_imports = imports.to_scene_graph();
    let mut text_src_diagnostics: Vec<Diagnostic> = Vec::new();
    resolve_text_sources(host.fs, &mut doc, project_dir, &mut text_src_diagnostics);
    let fonts = build_font_provider_with_imports(host, &doc, project_dir, &imports, locked)?;
    let index_a = resolve_page_index(&doc, page_a)?;
    let index_b = resolve_page_index(&doc, page_b)?;
    let assets = match project_dir {
        Some(dir) => build_asset_provider_with_imports(host.fs, &doc, dir, &imports, locked)?,
        None => BytesAssetProvider::new(),
    };
    // The override wins, then the document's spread gutter, then 0.
    let gutter_px = gutter_override.unwrap_or_else(|| {
        doc.spread_gutter
            .as_ref()
            .and_then(|d| dim_to_px(d.value, &d.unit))
            .map(|px| px.max(0.0) as u32)
            .unwrap_or(0)
    });
    let render_opts = RenderOptions::new(flags)
        .with_locked(locked)
        .with_data(data)
        .with_construction_overlay(construction_overlay);
    let prep = DocumentPrep::new(&doc, data, Some(&scene_imports)).with_image_sizes(image_sizes(
        &doc,
        Some(&imports),
        &assets,
    ));
    let compiler = PageCompiler::new(&prep, &fonts);
    let compile_a = compile_local_for_render(&doc, &compiler, index_a, render_opts);
    let compile_b = compile_local_for_render(&doc, &compiler, index_b, render_opts);
    let spread_err = |e: zenith_render::RenderError| {
        PipelineError::new(
            "render.spread_failed",
            format!("spread render error: {e}"),
            2,
        )
    };
    let left = render_image_scaled(&compile_a.scene, scale, &fonts, &assets).map_err(spread_err)?;
    let right =
        render_image_scaled(&compile_b.scene, scale, &fonts, &assets).map_err(spread_err)?;
    let image =
        composite_spread(&left, &right, scaled_gutter(gutter_px, scale)).map_err(spread_err)?;
    let png = encode_png(&image).map_err(spread_err)?;
    let mut compile_diagnostics = compiler.document_diagnostics();
    compile_diagnostics.extend(compile_a.diagnostics);
    compile_diagnostics.extend(compile_b.diagnostics);
    let mut diagnostics = validation;
    diagnostics.extend(text_src_diagnostics);
    diagnostics.extend(import_diagnostics);
    diagnostics.extend(disk_diagnostics_with_imports(
        host.fs,
        &doc,
        project_dir,
        &imports,
    ));
    diagnostics.extend(govern_compile_diagnostics(compile_diagnostics, &policy));
    Ok(PngArtifact {
        png,
        width: image.width,
        height: image.height,
        page_count: doc.body.pages.len(),
        diagnostics: Diagnostic::dedup(diagnostics),
        import_files,
    })
}

/// Spread gutter in output pixels: `round(gutter_px × scale)` (half away from
/// zero). Exact at scale 1.
fn scaled_gutter(gutter_px: u32, scale: f64) -> u32 {
    if scale == 1.0 {
        return gutter_px;
    }
    let scaled = (f64::from(gutter_px) * scale).round();
    if scaled.is_finite() && scaled > 0.0 {
        scaled.min(f64::from(u32::MAX)) as u32
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gutter_scales_and_rounds() {
        assert_eq!(scaled_gutter(10, 1.0), 10);
        assert_eq!(scaled_gutter(10, 0.25), 3);
        assert_eq!(scaled_gutter(0, 2.0), 0);
    }
}
