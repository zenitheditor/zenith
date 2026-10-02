//! Compile-stage diagnostics without rasterizing, for `validate`.

use std::path::Path;

use zenith_core::{Diagnostic, DiagnosticPolicy, Document};
use zenith_scene::{DocumentPrep, PageCompiler};

use crate::commands::composition_imports::LoadedImportGraph;

use super::assets::build_font_provider_with_imports;
use super::pages::map_pages;
use super::pipeline::govern_compile_diagnostics;
use super::text_source::resolve_text_sources;

/// Compile every page of `doc` (no raster) and return the diagnostics the
/// render path reports past validation.
///
/// Covers text sources, document diagnostics once, then each page's own in
/// page order: `text.overflow`, `text.fit_failed`, `font.unresolved`,
/// `font.glyph_missing`, and the rest. `policy` governs them exactly as on
/// render. No data context is bound, so `(data)` refs report
/// `data.no_context`. Repeats are removed.
pub(crate) fn compile_check_diagnostics(
    doc: &Document,
    project_dir: Option<&Path>,
    imports: &LoadedImportGraph,
    policy: &DiagnosticPolicy,
) -> Vec<Diagnostic> {
    let mut doc = doc.clone();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    resolve_text_sources(&mut doc, project_dir, &mut diagnostics);
    let fonts = match build_font_provider_with_imports(&doc, project_dir, imports, false) {
        Ok(fonts) => fonts,
        Err(e) => {
            diagnostics.extend(e.diagnostics);
            return Diagnostic::dedup(diagnostics);
        }
    };
    let scene_imports = imports.to_scene_graph();
    let prep = DocumentPrep::new(&doc, None, Some(&scene_imports));
    let compiler = PageCompiler::new(&prep, &fonts);
    let mut compiled = compiler.document_diagnostics();
    let pages = map_pages(doc.body.pages.len(), |page_index| {
        compiler.compile_page_local(page_index).diagnostics
    });
    compiled.extend(pages.into_iter().flatten());
    diagnostics.extend(govern_compile_diagnostics(compiled, policy));
    Diagnostic::dedup(diagnostics)
}
