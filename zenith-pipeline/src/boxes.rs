//! Final node geometry of every page, with the render path's setup.

use std::collections::BTreeMap;
use std::path::Path;

use zenith_core::{Diagnostic, Document, default_provider};
use zenith_scene::{CompiledBox, DocumentPrep, PageCompiler};

use crate::assets::{build_font_provider_with_imports, read_image_sizes, resolve_text_sources};
use crate::host::Host;
use crate::imports::load_import_graph;

/// The final geometry of every compiled node of each page of `doc`, by id,
/// from [`PageCompiler::compiled_boxes`] (no lint).
///
/// Each page compiles with the render path's setup: text sources, imports,
/// project fonts, and image intrinsic sizes from `project_dir`. No config is
/// read, and diagnostics are dropped. A font load error falls back to the
/// bundled fonts.
#[must_use]
pub fn resolved_boxes(
    host: Host<'_>,
    doc: &Document,
    project_dir: Option<&Path>,
) -> Vec<BTreeMap<String, CompiledBox>> {
    let mut doc = doc.clone();
    let mut ignored: Vec<Diagnostic> = Vec::new();
    resolve_text_sources(host.fs, &mut doc, project_dir, &mut ignored);
    let imports = load_import_graph(host.fs, &doc, project_dir);
    let fonts = build_font_provider_with_imports(host, &doc, project_dir, &imports, false)
        .unwrap_or_else(|_| default_provider());
    let scene_imports = imports.to_scene_graph();
    let prep = DocumentPrep::new(&doc, None, Some(&scene_imports))
        .with_image_sizes(read_image_sizes(host.fs, &doc, project_dir, &imports));
    let compiler = PageCompiler::new(&prep, &fonts);
    (0..compiler.page_count())
        .map(|index| compiler.compiled_boxes(index))
        .collect()
}
