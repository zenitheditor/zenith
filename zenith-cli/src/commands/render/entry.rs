//! The render command's error type, output artifacts, and public entry points.

use std::path::Path;

use zenith_core::{BytesAssetProvider, DataContext, Diagnostic, dim_to_px};
use zenith_render::{
    PdfOptions, render_pdf_multi_with, render_pdf_with, render_png, render_spread_png,
};
use zenith_scene::{DocumentPrep, PageCompiler, Scene};

use crate::config::CliPolicyFlags;
use crate::report::{ImportFiles, attributed_loader_diagnostics};

use super::assets::{
    build_asset_provider_with_imports, build_font_provider_with_imports,
    disk_diagnostics_with_imports,
};
use super::pages::{compile_for_render, compile_local_for_render, map_pages};
use super::pipeline::{Validated, govern_compile_diagnostics, parse_validate, resolve_page_index};
use super::text_source::resolve_text_sources;

// ── Error type ────────────────────────────────────────────────────────────────

/// Error produced by the render command.
///
/// `diagnostics` holds every diagnostic known when the render stopped. It
/// always has at least one [`Severity::Error`](zenith_core::Severity::Error)
/// entry. `message` is the human text of the error lines.
#[derive(Debug)]
pub struct RenderCmdErr {
    /// Human-readable message.
    pub message: String,
    /// Recommended exit code.
    pub exit_code: u8,
    /// Every diagnostic known at the failure point, in report order.
    pub diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports, for locating diagnostic spans.
    pub import_files: ImportFiles,
}

impl RenderCmdErr {
    /// One error diagnostic with `code` and `msg`.
    pub(super) fn new(code: &str, msg: impl Into<String>, exit_code: u8) -> Self {
        Self::blocked(vec![Diagnostic::error(code, msg, None, None)], exit_code)
    }

    /// A render stopped by `diagnostics`. The message joins the error lines.
    pub(super) fn blocked(diagnostics: Vec<Diagnostic>, exit_code: u8) -> Self {
        let diagnostics = Diagnostic::dedup(diagnostics);
        let message = diagnostics
            .iter()
            .filter(|d| d.is_error())
            .map(crate::commands::format_error_diag)
            .collect::<Vec<_>>()
            .join("\n");
        Self {
            message,
            exit_code,
            diagnostics,
            import_files: ImportFiles::default(),
        }
    }

    /// Return this error with the composition import files that its
    /// diagnostic spans can index into.
    pub(super) fn with_import_files(mut self, import_files: ImportFiles) -> Self {
        self.import_files = import_files;
        self
    }
}

// ── Artifacts ─────────────────────────────────────────────────────────────────

/// Scene JSON plus the compile-stage diagnostics that produced it.
#[derive(Debug)]
pub struct SceneArtifact {
    /// The serialised scene JSON.
    pub json: String,
    /// Validation diagnostics, then compile-stage diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports, for locating diagnostic spans.
    pub import_files: ImportFiles,
}

/// Rendered PNG bytes plus the compile-stage diagnostics that produced them.
#[derive(Debug)]
pub struct PngArtifact {
    /// The encoded PNG bytes.
    pub png: Vec<u8>,
    /// Validation diagnostics, then compile-stage diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports, for locating diagnostic spans.
    pub import_files: ImportFiles,
}

/// Rendered vector PDF bytes plus the compile-stage diagnostics that produced
/// them.
#[derive(Debug)]
pub struct PdfArtifact {
    /// The encoded PDF bytes.
    pub pdf: Vec<u8>,
    /// Validation diagnostics, then compile-stage diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports, for locating diagnostic spans.
    pub import_files: ImportFiles,
}

/// Shared options for render entry points.
#[derive(Clone, Copy)]
pub struct RenderEntryOptions<'a> {
    /// Verify asset/font sha256 and fail on mismatch.
    pub locked: bool,
    /// Subset PDF fonts to used glyphs.
    pub subset: bool,
    /// Diagnostic-policy CLI flags.
    pub flags: &'a CliPolicyFlags,
    /// Optional data context for `(data)` references.
    pub data: Option<&'a DataContext>,
    /// Append page construction guides to the scene after canonical compile.
    pub construction_overlay: bool,
}

impl<'a> RenderEntryOptions<'a> {
    fn png(flags: &'a CliPolicyFlags, locked: bool, data: Option<&'a DataContext>) -> Self {
        Self {
            locked,
            subset: true,
            flags,
            data,
            construction_overlay: false,
        }
    }

    fn scene(flags: &'a CliPolicyFlags, data: Option<&'a DataContext>) -> Self {
        Self {
            locked: false,
            subset: true,
            flags,
            data,
            construction_overlay: false,
        }
    }

    fn pdf(
        flags: &'a CliPolicyFlags,
        locked: bool,
        subset: bool,
        data: Option<&'a DataContext>,
    ) -> Self {
        Self {
            locked,
            subset,
            flags,
            data,
            construction_overlay: false,
        }
    }

    /// Return a copy with construction overlay enabled or disabled.
    pub fn with_construction_overlay(mut self, construction_overlay: bool) -> Self {
        self.construction_overlay = construction_overlay;
        self
    }
}

// ── Entry points ──────────────────────────────────────────────────────────────

/// Parse `src`, validate it with the merged diagnostic policy, compile the
/// requested `page` (1-based), and return the scene JSON plus the
/// compile-stage diagnostics.
///
/// `project_dir` is the `.zen` file's parent directory. When `Some`, font
/// assets declared in the document are loaded and registered in the font
/// provider so that `font.family` tokens referencing them resolve to the
/// actual face rather than falling back to the bundled Noto fonts. When
/// `None`, only the bundled fonts are available.
///
/// `data` is an optional data context for resolving `(data)"field"` property
/// references at compile time. When `None`, data refs produce
/// `data.missing_field` / `data.no_context` advisories (non-fatal).
///
/// `flags` carries the `--allow`/`--warn`/`--deny` CLI overrides; pass
/// `&CliPolicyFlags::default()` when no flags are available (e.g. MCP).
///
/// Returns `Err` when:
/// - A config file cannot be read (exit code 2).
/// - The source fails to parse (exit code 2).
/// - The document has validation errors (exit code 1).
/// - The `page` is out of range (exit code 2).
/// - Scene JSON serialisation fails (exit code 2).
pub fn to_scene_json(
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    flags: &CliPolicyFlags,
    data: Option<&DataContext>,
) -> Result<SceneArtifact, RenderCmdErr> {
    to_scene_json_with_options(
        src,
        project_dir,
        page,
        RenderEntryOptions::scene(flags, data),
    )
}

pub fn to_scene_json_with_options(
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    opts: RenderEntryOptions<'_>,
) -> Result<SceneArtifact, RenderCmdErr> {
    let Validated {
        mut doc,
        policy,
        imports,
        diagnostics: validation,
    } = parse_validate(src, project_dir, opts.flags)?;
    let import_diagnostics = attributed_loader_diagnostics(&imports);
    let import_files = ImportFiles::from_graph(&imports);
    let scene_imports = imports.to_scene_graph();
    let mut text_src_diagnostics: Vec<Diagnostic> = Vec::new();
    resolve_text_sources(&mut doc, project_dir, &mut text_src_diagnostics);
    let fonts = build_font_provider_with_imports(&doc, project_dir, &imports, false)?;
    let page_index = resolve_page_index(&doc, page)?;
    let prep = DocumentPrep::new(&doc, opts.data, Some(&scene_imports));
    let compiler = PageCompiler::new(&prep, &fonts);
    let compile_result = compile_for_render(&doc, &compiler, page_index, opts);
    let json = compile_result.scene.to_json().map_err(|e| {
        RenderCmdErr::new(
            "render.scene_serialize_failed",
            format!("scene serialisation error: {e}"),
            2,
        )
    })?;
    let mut diagnostics = validation;
    diagnostics.extend(text_src_diagnostics);
    diagnostics.extend(import_diagnostics);
    diagnostics.extend(disk_diagnostics_with_imports(&doc, project_dir, &imports));
    diagnostics.extend(govern_compile_diagnostics(
        compile_result.diagnostics,
        &policy,
    ));
    let diagnostics = Diagnostic::dedup(diagnostics);
    Ok(SceneArtifact {
        json,
        diagnostics,
        import_files,
    })
}

/// Parse `src`, validate it, compile the scene, and return PNG bytes.
///
/// No image or SVG assets are loaded (an empty asset provider is used); any
/// `image`/`svg` nodes are rendered without their content. Use
/// [`to_png_with_dir`] to source asset bytes relative to the document's
/// directory.
///
/// `page` is the 1-based page number to render. No CLI policy flags are
/// applied; config files are still resolved (global only, no `start_dir`).
/// No data context is supplied; data refs produce non-fatal advisories.
///
/// Returns `Err` when:
/// - A config file cannot be read (exit code 2).
/// - The source fails to parse (exit code 2).
/// - The document has validation errors (exit code 1).
/// - The `page` is out of range (exit code 2).
/// - Rendering fails (exit code 2).
pub fn to_png(src: &str, page: usize) -> Result<PngArtifact, RenderCmdErr> {
    to_png_with_dir(src, None, page, false, &CliPolicyFlags::default(), None)
}

/// Like [`to_png`], but sources image and SVG asset bytes from `project_dir`
/// (the `.zen` file's parent directory) when provided, and honours the merged
/// diagnostic policy.
///
/// For each `image`- or `svg`-kind `AssetDecl`, the `src` is resolved relative
/// to `project_dir` and read into a [`BytesAssetProvider`]. A read failure
/// silently skips that asset; the missing file is instead surfaced as a hard
/// `asset.missing` Error diagnostic on the returned artifact (which trips the
/// render gate). When `project_dir` is `None` no assets are loaded.
///
/// When `locked` is set, every image and SVG asset's bytes are verified against
/// their declared `sha256` and any mismatch, missing hash, or read failure is a
/// hard error (exit code 2). When `project_dir` is `None` there are no assets,
/// so `locked` is a no-op.
///
/// `page` is the 1-based page number to render.
///
/// `data` is an optional data context for resolving `(data)"field"` property
/// references at compile time. When `None`, data refs produce non-fatal
/// advisories.
///
/// `flags` carries the `--allow`/`--warn`/`--deny` CLI overrides; pass
/// `&CliPolicyFlags::default()` when no flags are available (e.g. MCP).
pub fn to_png_with_dir(
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    locked: bool,
    flags: &CliPolicyFlags,
    data: Option<&DataContext>,
) -> Result<PngArtifact, RenderCmdErr> {
    to_png_with_dir_options(
        src,
        project_dir,
        page,
        RenderEntryOptions::png(flags, locked, data),
    )
}

pub fn to_png_with_dir_options(
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    opts: RenderEntryOptions<'_>,
) -> Result<PngArtifact, RenderCmdErr> {
    let Validated {
        mut doc,
        policy,
        imports,
        diagnostics: validation,
    } = parse_validate(src, project_dir, opts.flags)?;
    let import_diagnostics = attributed_loader_diagnostics(&imports);
    let import_files = ImportFiles::from_graph(&imports);
    let scene_imports = imports.to_scene_graph();
    let mut text_src_diagnostics: Vec<Diagnostic> = Vec::new();
    resolve_text_sources(&mut doc, project_dir, &mut text_src_diagnostics);
    let fonts = build_font_provider_with_imports(&doc, project_dir, &imports, opts.locked)?;
    let page_index = resolve_page_index(&doc, page)?;
    let assets = match project_dir {
        Some(dir) => build_asset_provider_with_imports(&doc, dir, &imports, opts.locked)?,
        None => BytesAssetProvider::new(),
    };
    let prep = DocumentPrep::new(&doc, opts.data, Some(&scene_imports));
    let compiler = PageCompiler::new(&prep, &fonts);
    let compile_result = compile_for_render(&doc, &compiler, page_index, opts);
    let png = render_png(&compile_result.scene, &fonts, &assets)
        .map_err(|e| RenderCmdErr::new("render.raster_failed", format!("render error: {e}"), 2))?;
    let mut diagnostics = validation;
    diagnostics.extend(text_src_diagnostics);
    diagnostics.extend(import_diagnostics);
    diagnostics.extend(disk_diagnostics_with_imports(&doc, project_dir, &imports));
    diagnostics.extend(govern_compile_diagnostics(
        compile_result.diagnostics,
        &policy,
    ));
    let diagnostics = Diagnostic::dedup(diagnostics);
    Ok(PngArtifact {
        png,
        diagnostics,
        import_files,
    })
}

/// Parse `src`, validate it with the merged diagnostic policy, compile the
/// requested `page`, and render a vector PDF, sourcing image/SVG and font asset
/// bytes from `project_dir` when provided (exactly like [`to_png_with_dir`]).
///
/// The PDF carries print box metadata (MediaBox / TrimBox / BleedBox /
/// CropBox) and native DeviceCMYK for CMYK-origin colors. Output is
/// deterministic. `page` is the 1-based page number.
///
/// `data` is an optional data context for resolving `(data)"field"` property
/// references at compile time. When `None`, data refs produce non-fatal
/// advisories.
///
/// `flags` carries the `--allow`/`--warn`/`--deny` CLI overrides; pass
/// `&CliPolicyFlags::default()` when no flags are available (e.g. MCP).
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
    let Validated {
        mut doc,
        policy,
        imports,
        diagnostics: validation,
    } = parse_validate(src, project_dir, opts.flags)?;
    let import_diagnostics = attributed_loader_diagnostics(&imports);
    let import_files = ImportFiles::from_graph(&imports);
    let scene_imports = imports.to_scene_graph();
    let mut text_src_diagnostics: Vec<Diagnostic> = Vec::new();
    resolve_text_sources(&mut doc, project_dir, &mut text_src_diagnostics);
    let fonts = build_font_provider_with_imports(&doc, project_dir, &imports, opts.locked)?;
    let page_index = resolve_page_index(&doc, page)?;
    let assets = match project_dir {
        Some(dir) => build_asset_provider_with_imports(&doc, dir, &imports, opts.locked)?,
        None => BytesAssetProvider::new(),
    };
    let prep = DocumentPrep::new(&doc, opts.data, Some(&scene_imports));
    let compiler = PageCompiler::new(&prep, &fonts);
    let compile_result = compile_for_render(&doc, &compiler, page_index, opts);
    let pdf = render_pdf_with(
        &compile_result.scene,
        &fonts,
        &assets,
        PdfOptions {
            subset: opts.subset,
        },
    );
    let mut diagnostics = validation;
    diagnostics.extend(text_src_diagnostics);
    diagnostics.extend(import_diagnostics);
    diagnostics.extend(disk_diagnostics_with_imports(&doc, project_dir, &imports));
    diagnostics.extend(govern_compile_diagnostics(
        compile_result.diagnostics,
        &policy,
    ));
    let diagnostics = Diagnostic::dedup(diagnostics);
    Ok(PdfArtifact {
        pdf,
        diagnostics,
        import_files,
    })
}

/// Parse `src`, validate it with the merged diagnostic policy, compile EVERY
/// page (in document order, page 1 first), and render them into a single
/// multi-page vector PDF, sourcing image/SVG and font asset bytes from
/// `project_dir` when provided (exactly like [`to_pdf_with_dir`]).
///
/// This is the default `--pdf` behavior: a multi-page document produces a
/// multi-page PDF. Use [`to_pdf_with_dir`] to select one explicit page.
///
/// Diagnostics from disk plus every page's governed compile diagnostics are
/// merged in document order (page 1's first), document diagnostics once and
/// repeats removed. The
/// PDF carries print box metadata and native DeviceCMYK exactly as the
/// single-page path; a one-page document yields byte-identical output to
/// [`to_pdf_with_dir`] for page 1.
///
/// `data` is applied to every page. `flags` carries the
/// `--allow`/`--warn`/`--deny` CLI overrides; pass `&CliPolicyFlags::default()`
/// when no flags are available (e.g. MCP).
///
/// Returns `Err` on parse failure (exit 2), validation errors (exit 1), an
/// empty document (exit 2), or an asset/font failure (exit 2).
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
    let Validated {
        mut doc,
        policy,
        imports,
        diagnostics: validation,
    } = parse_validate(src, project_dir, opts.flags)?;
    let import_diagnostics = attributed_loader_diagnostics(&imports);
    let import_files = ImportFiles::from_graph(&imports);
    let scene_imports = imports.to_scene_graph();
    let mut diagnostics: Vec<Diagnostic> = validation;
    resolve_text_sources(&mut doc, project_dir, &mut diagnostics);
    diagnostics.extend(import_diagnostics);
    let fonts = build_font_provider_with_imports(&doc, project_dir, &imports, opts.locked)?;
    let page_count = doc.body.pages.len();
    if page_count == 0 {
        return Err(RenderCmdErr::new(
            "render.no_pages",
            "document has no pages to render; add a page node",
            2,
        ));
    }
    let assets = match project_dir {
        Some(dir) => build_asset_provider_with_imports(&doc, dir, &imports, opts.locked)?,
        None => BytesAssetProvider::new(),
    };
    let mut scenes: Vec<Scene> = Vec::with_capacity(page_count);
    diagnostics.extend(disk_diagnostics_with_imports(&doc, project_dir, &imports));
    let prep = DocumentPrep::new(&doc, opts.data, Some(&scene_imports));
    let compiler = PageCompiler::new(&prep, &fonts);
    diagnostics.extend(govern_compile_diagnostics(
        compiler.document_diagnostics(),
        &policy,
    ));
    // Pages compile in parallel. Results merge here in page order.
    let compiled = map_pages(page_count, |page_index| {
        compile_local_for_render(&doc, &compiler, page_index, opts)
    });
    for compile_result in compiled {
        scenes.push(compile_result.scene);
        diagnostics.extend(govern_compile_diagnostics(
            compile_result.diagnostics,
            &policy,
        ));
    }
    let diagnostics = Diagnostic::dedup(diagnostics);
    let pdf = render_pdf_multi_with(
        &scenes,
        &fonts,
        &assets,
        PdfOptions {
            subset: opts.subset,
        },
    );
    Ok(PdfArtifact {
        pdf,
        diagnostics,
        import_files,
    })
}

/// PNG bytes for every page plus the diagnostics of the whole render.
#[derive(Debug)]
pub struct PngPagesArtifact {
    /// Encoded PNG bytes, one entry per page in document order.
    pub pages: Vec<Vec<u8>>,
    /// Validation diagnostics, document diagnostics once, then each page's
    /// own, in page order. Repeats are removed.
    pub diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports, for locating diagnostic spans.
    pub import_files: ImportFiles,
}

/// Parse `src`, validate it with the merged diagnostic policy, and render
/// EVERY page to PNG, in document order (page 1 first).
///
/// Image and SVG asset bytes are sourced once from `project_dir` (shared
/// across all pages). Returns `Err` on parse failure (exit 2), validation
/// errors (exit 1), an empty document (exit 2), or a render failure (exit 2).
/// When `locked` is set, image and SVG asset bytes are verified against their
/// declared `sha256` (exit 2 on any mismatch/missing hash/read failure).
///
/// Document-level diagnostics (disk, import, text-source, token, data, chain,
/// and table-flow) are reported once, not once per page.
///
/// `data` is an optional data context for resolving `(data)"field"` property
/// references at compile time (applied to every page). When `None`, data refs
/// produce non-fatal advisories.
///
/// `flags` carries the `--allow`/`--warn`/`--deny` CLI overrides; pass
/// `&CliPolicyFlags::default()` when no flags are available (e.g. MCP).
pub fn to_png_all_pages(
    src: &str,
    project_dir: Option<&Path>,
    locked: bool,
    flags: &CliPolicyFlags,
    data: Option<&DataContext>,
) -> Result<PngPagesArtifact, RenderCmdErr> {
    to_png_all_pages_options(
        src,
        project_dir,
        RenderEntryOptions::png(flags, locked, data),
    )
}

pub fn to_png_all_pages_options(
    src: &str,
    project_dir: Option<&Path>,
    opts: RenderEntryOptions<'_>,
) -> Result<PngPagesArtifact, RenderCmdErr> {
    let Validated {
        mut doc,
        policy,
        imports,
        diagnostics: validation,
    } = parse_validate(src, project_dir, opts.flags)?;
    let import_diagnostics = attributed_loader_diagnostics(&imports);
    let import_files = ImportFiles::from_graph(&imports);
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
    let assets = match project_dir {
        Some(dir) => build_asset_provider_with_imports(&doc, dir, &imports, opts.locked)?,
        None => BytesAssetProvider::new(),
    };
    diagnostics.extend(import_diagnostics);
    diagnostics.extend(disk_diagnostics_with_imports(&doc, project_dir, &imports));
    let prep = DocumentPrep::new(&doc, opts.data, Some(&scene_imports));
    let compiler = PageCompiler::new(&prep, &fonts);
    diagnostics.extend(govern_compile_diagnostics(
        compiler.document_diagnostics(),
        &policy,
    ));
    // Pages compile and rasterize in parallel. Results merge here in page
    // order, so the first error reported is the lowest failing page.
    let rendered = map_pages(page_count, |page_index| {
        let compile_result = compile_local_for_render(&doc, &compiler, page_index, opts);
        let png = render_png(&compile_result.scene, &fonts, &assets).map_err(|e| {
            RenderCmdErr::new(
                "render.raster_failed",
                format!("render error on page {}: {e}", page_index + 1),
                2,
            )
        });
        (compile_result, png)
    });
    let mut pages = Vec::with_capacity(page_count);
    for (compile_result, png) in rendered {
        pages.push(png?);
        diagnostics.extend(govern_compile_diagnostics(
            compile_result.diagnostics,
            &policy,
        ));
    }
    Ok(PngPagesArtifact {
        pages,
        diagnostics: Diagnostic::dedup(diagnostics),
        import_files,
    })
}

/// Bundled render options for [`to_png_spread`], keeping its argument count
/// within the lint limit (the spread path also takes two page indices and a
/// gutter override). `Copy` so it cascades cheaply.
#[derive(Clone, Copy)]
pub struct SpreadRenderOpts<'a> {
    /// Verify asset sha256 and fail on mismatch.
    pub locked: bool,
    /// Diagnostic-policy CLI flags.
    pub flags: &'a CliPolicyFlags,
    /// Optional data context for `(data)` references.
    pub data: Option<&'a DataContext>,
    /// Append page construction guides to both compiled scenes.
    pub construction_overlay: bool,
}

/// Parse `src`, validate it with the merged diagnostic policy, compile pages
/// `page_a` and `page_b` (both 1-based), composite them side by side (A on
/// the left, B on the right), and return the spread PNG bytes plus the merged
/// compile-stage diagnostics.
///
/// The output canvas width is `page_a_width + gutter_override_px + page_b_width`
/// (or `page_a_width + doc.spread_gutter + page_b_width` when the override is
/// `None`, defaulting to 0 when neither is set). A `gutter_px > 0` inserts that
/// many fully-transparent columns between the two pages. Image/SVG/font asset
/// bytes are sourced from `project_dir` (shared across both pages) exactly like
/// [`to_png_with_dir`].
///
/// `data` is an optional data context for resolving `(data)"field"` property
/// references at compile time (applied to both pages). When `None`, data refs
/// produce non-fatal advisories.
///
/// `flags` carries the `--allow`/`--warn`/`--deny` CLI overrides; pass
/// `&CliPolicyFlags::default()` when no flags are available (e.g. MCP).
///
/// Returns `Err` when:
/// - A config file cannot be read (exit code 2).
/// - The source fails to parse (exit code 2).
/// - The document has validation errors (exit code 1).
/// - Either page is out of range (exit code 2).
/// - Rendering or compositing fails (exit code 2).
pub fn to_png_spread(
    src: &str,
    project_dir: Option<&Path>,
    page_a: usize,
    page_b: usize,
    gutter_override: Option<u32>,
    opts: SpreadRenderOpts<'_>,
) -> Result<PngArtifact, RenderCmdErr> {
    let SpreadRenderOpts {
        locked,
        flags,
        data,
        construction_overlay,
    } = opts;
    let Validated {
        mut doc,
        policy,
        imports,
        diagnostics: validation,
    } = parse_validate(src, project_dir, flags)?;
    let import_diagnostics = attributed_loader_diagnostics(&imports);
    let import_files = ImportFiles::from_graph(&imports);
    let scene_imports = imports.to_scene_graph();
    let mut text_src_diagnostics: Vec<Diagnostic> = Vec::new();
    resolve_text_sources(&mut doc, project_dir, &mut text_src_diagnostics);
    let fonts = build_font_provider_with_imports(&doc, project_dir, &imports, locked)?;
    let index_a = resolve_page_index(&doc, page_a)?;
    let index_b = resolve_page_index(&doc, page_b)?;
    let assets = match project_dir {
        Some(dir) => build_asset_provider_with_imports(&doc, dir, &imports, locked)?,
        None => BytesAssetProvider::new(),
    };
    // Resolve gutter: CLI override wins, then doc.spread_gutter, then 0.
    let gutter_px = gutter_override.unwrap_or_else(|| {
        doc.spread_gutter
            .as_ref()
            .and_then(|d| dim_to_px(d.value, &d.unit))
            .map(|px| px.max(0.0) as u32)
            .unwrap_or(0)
    });
    let render_opts = RenderEntryOptions::png(flags, locked, data)
        .with_construction_overlay(construction_overlay);
    let prep = DocumentPrep::new(&doc, data, Some(&scene_imports));
    let compiler = PageCompiler::new(&prep, &fonts);
    let compile_a = compile_local_for_render(&doc, &compiler, index_a, render_opts);
    let compile_b = compile_local_for_render(&doc, &compiler, index_b, render_opts);
    let png = render_spread_png(
        &compile_a.scene,
        &compile_b.scene,
        gutter_px,
        &fonts,
        &assets,
    )
    .map_err(|e| {
        RenderCmdErr::new(
            "render.spread_failed",
            format!("spread render error: {e}"),
            2,
        )
    })?;
    let mut compile_diagnostics = compiler.document_diagnostics();
    compile_diagnostics.extend(compile_a.diagnostics);
    compile_diagnostics.extend(compile_b.diagnostics);
    let mut diagnostics = validation;
    diagnostics.extend(text_src_diagnostics);
    diagnostics.extend(import_diagnostics);
    diagnostics.extend(disk_diagnostics_with_imports(&doc, project_dir, &imports));
    diagnostics.extend(govern_compile_diagnostics(compile_diagnostics, &policy));
    let diagnostics = Diagnostic::dedup(diagnostics);
    Ok(PngArtifact {
        png,
        diagnostics,
        import_files,
    })
}
