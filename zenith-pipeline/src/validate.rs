//! The full validate pipeline ([`validate_source`], [`validate_parsed`]) and compile-stage
//! diagnostics without raster ([`compile_check_diagnostics`]).

use std::path::Path;

use zenith_core::{Diagnostic, DiagnosticPolicy, Document, validate_with_policy};
use zenith_scene::{DocumentPrep, PageCompiler};

use crate::assets::{
    build_font_provider_with_imports, collect_image_dimension_diagnostics,
    collect_missing_asset_diagnostics, read_image_sizes, resolve_text_sources,
};
use crate::host::Host;
use crate::imports::{
    ImportFiles, LoadedImportGraph, attributed_loader_diagnostics, load_import_graph,
};
use crate::io::map_pages;
use crate::policy::{ConfigLayers, PolicyFlags};
use crate::prepare::{govern_compile_diagnostics, parse_diagnostic};

/// Diagnostics of one validate run.
#[derive(Debug)]
pub struct Validation {
    /// Every diagnostic, repeats removed.
    pub diagnostics: Vec<Diagnostic>,
    /// Import files that diagnostic spans index into.
    pub files: ImportFiles,
    /// 0 = no errors, 1 = validation errors, 2 = parse or config error.
    pub exit_code: u8,
}

/// Run the full validate pipeline on `src`.
///
/// Stages, in report order:
/// 1. Config: global, then local walked up from `project_dir`. A load error
///    is one `config.error` diagnostic, exit code 2.
/// 2. Parse. A parse error is one `parse.error` diagnostic, exit code 2.
/// 3. Validate with `merge_policy(global, local, in_file, flags)` and the
///    merged brand contract.
/// 4. With `project_dir`: `asset.missing` and image size advisories for the
///    host document.
/// 5. The import graph's loader diagnostics.
/// 6. With no Error so far: every page compiles (no raster), so the
///    compile-stage diagnostics a render reports show here too.
///
/// Repeats are removed. Exit code 1 when any diagnostic is an Error.
#[must_use]
pub fn validate_source(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    flags: &PolicyFlags,
) -> Validation {
    validate_parsed(host, parse_diagnostic(src).as_ref(), project_dir, flags)
}

/// [`validate_source`] over the parse of the source the caller already
/// holds: `Ok` the document, or `Err` the `parse.error` diagnostic.
///
/// The result equals [`validate_source`] of the text `parsed` came from,
/// without parsing it again. The config still loads first, so a config
/// error wins over a parse error.
#[must_use]
pub fn validate_parsed(
    host: Host<'_>,
    parsed: Result<&Document, &Diagnostic>,
    project_dir: Option<&Path>,
    flags: &PolicyFlags,
) -> Validation {
    let failed = |d: Diagnostic| Validation {
        diagnostics: vec![d],
        files: ImportFiles::default(),
        exit_code: 2,
    };
    let layers = match ConfigLayers::load(host.config, project_dir) {
        Ok(layers) => layers,
        Err(msg) => return failed(Diagnostic::error("config.error", msg, None, None)),
    };
    let doc = match parsed {
        Ok(d) => d,
        Err(d) => return failed(d.clone()),
    };

    let merged = layers.policy(&doc.diagnostic_policy, flags);
    let effective_brand = layers.brand(&doc.brand_contract);
    let mut diagnostics = validate_with_policy(doc, &merged, &effective_brand).diagnostics;
    if let Some(dir) = project_dir {
        diagnostics.extend(collect_missing_asset_diagnostics(host.fs, doc, dir));
        diagnostics.extend(collect_image_dimension_diagnostics(host.fs, doc, dir));
    }
    let imports = load_import_graph(host.fs, doc, project_dir);
    diagnostics.extend(attributed_loader_diagnostics(&imports));
    if !Diagnostic::has_errors(&diagnostics) {
        diagnostics.extend(compile_check_diagnostics(
            host,
            doc,
            project_dir,
            &imports,
            &merged,
        ));
    }
    let diagnostics = Diagnostic::dedup(diagnostics);
    let exit_code = u8::from(Diagnostic::has_errors(&diagnostics));
    Validation {
        diagnostics,
        files: ImportFiles::from_graph(&imports),
        exit_code,
    }
}

/// Compile every page of `doc` (no raster) and return the diagnostics a
/// render reports past validation.
///
/// Text sources first, then the document diagnostics once, then each page's
/// own in page order: `text.overflow`, `text.fit_failed`, `font.unresolved`,
/// `font.glyph_missing`, and the rest. `policy` governs them as on render.
/// No data context is bound, so `(data)` refs report `data.no_context`.
/// Repeats are removed.
#[must_use]
pub fn compile_check_diagnostics(
    host: Host<'_>,
    doc: &Document,
    project_dir: Option<&Path>,
    imports: &LoadedImportGraph,
    policy: &DiagnosticPolicy,
) -> Vec<Diagnostic> {
    let mut doc = doc.clone();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    resolve_text_sources(host.fs, &mut doc, project_dir, &mut diagnostics);
    let fonts = match build_font_provider_with_imports(host, &doc, project_dir, imports, false) {
        Ok(fonts) => fonts,
        Err(e) => {
            diagnostics.extend(e.diagnostics);
            return Diagnostic::dedup(diagnostics);
        }
    };
    let scene_imports = imports.to_scene_graph();
    let prep = DocumentPrep::new(&doc, None, Some(&scene_imports))
        .with_image_sizes(read_image_sizes(host.fs, &doc, project_dir, imports));
    let compiler = PageCompiler::new(&prep, &fonts);
    let mut compiled = compiler.document_diagnostics();
    let pages = map_pages(host.runner, doc.body.pages.len(), |page_index| {
        compiler.compile_page_local(page_index).diagnostics
    });
    compiled.extend(pages.into_iter().flatten());
    diagnostics.extend(govern_compile_diagnostics(compiled, policy));
    Diagnostic::dedup(diagnostics)
}
