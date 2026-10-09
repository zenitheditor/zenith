//! Parse → validate with the merged config policy and brand, plus the small
//! helpers every entry point shares.

use std::path::Path;

use zenith_core::{
    Diagnostic, DiagnosticPolicy, Document, KdlAdapter, KdlSource, apply_policy,
    validate_with_policy,
};

use crate::assets::disk_diagnostics_with_imports;
use crate::error::PipelineError;
use crate::host::Host;
use crate::imports::{
    ImportFiles, LoadedImportGraph, attributed_loader_diagnostics, load_import_graph,
};
use crate::policy::{ConfigLayers, PolicyFlags};

/// A parsed, validated document with the state the entry points share.
#[derive(Debug)]
pub struct Validated {
    /// The parsed document.
    pub doc: Document,
    /// The merged diagnostic policy, applied to compile-stage diagnostics too.
    pub policy: DiagnosticPolicy,
    /// The loaded composition import graph.
    pub imports: LoadedImportGraph,
    /// Policy-governed validation diagnostics. None is an error. Entry points
    /// put them first in their own diagnostic list.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parse `src`, then validate it with the merged policy and brand contract.
///
/// The effective policy is `merge_policy(global, local, in_file, flags)`:
/// the global config always, the local config walked up from `start_dir`
/// when `Some`, the document's own block, then `flags`. The brand contract
/// merges global → local → in-file per category. The import graph loads from
/// `start_dir` through `host.fs`.
///
/// # Errors
///
/// - A config read or parse error: `config.error`, exit code 2.
/// - A parse error: `parse.error` with its span, exit code 2.
/// - Any Error diagnostic after policy: exit code 1, with the import-loader
///   and disk diagnostics added so one round shows every known problem.
pub fn parse_validate(
    host: Host<'_>,
    src: &str,
    start_dir: Option<&Path>,
    flags: &PolicyFlags,
) -> Result<Validated, PipelineError> {
    parse_validate_with(host, src, None, start_dir, flags)
}

/// [`parse_validate`], reusing `parsed` when the caller already holds the
/// parse of `src`.
///
/// `parsed` must be the parse of `src`: the result then equals
/// [`parse_validate`] of `src`, without parsing it again.
///
/// # Errors
///
/// The [`parse_validate`] errors.
pub fn parse_validate_with(
    host: Host<'_>,
    src: &str,
    parsed: Option<&Document>,
    start_dir: Option<&Path>,
    flags: &PolicyFlags,
) -> Result<Validated, PipelineError> {
    let layers = ConfigLayers::load(host.config, start_dir)
        .map_err(|msg| PipelineError::new("config.error", msg, 2))?;

    let doc = match parsed {
        Some(doc) => doc.clone(),
        None => parse(src)?,
    };

    let merged = layers.policy(&doc.diagnostic_policy, flags);
    let effective_brand = layers.brand(&doc.brand_contract);
    let report = validate_with_policy(&doc, &merged, &effective_brand);
    let imports = load_import_graph(host.fs, &doc, start_dir);
    if Diagnostic::has_errors(&report.diagnostics) {
        let mut diagnostics = report.diagnostics;
        diagnostics.extend(attributed_loader_diagnostics(&imports));
        diagnostics.extend(disk_diagnostics_with_imports(
            host.fs, &doc, start_dir, &imports,
        ));
        return Err(PipelineError::blocked(diagnostics, 1)
            .with_import_files(ImportFiles::from_graph(&imports)));
    }

    Ok(Validated {
        doc,
        policy: merged,
        imports,
        diagnostics: report.diagnostics,
    })
}

/// Parse `src` as a `.zen` document.
///
/// # Errors
///
/// A `parse.error` diagnostic with its span, exit code 2.
pub fn parse(src: &str) -> Result<Document, PipelineError> {
    parse_diagnostic(src).map_err(|d| PipelineError::blocked(vec![d], 2))
}

/// Parse `src` as a `.zen` document.
///
/// # Errors
///
/// The `parse.error` diagnostic, with its span.
pub fn parse_diagnostic(src: &str) -> Result<Document, Diagnostic> {
    KdlAdapter
        .parse(src.as_bytes())
        .map_err(|e| Diagnostic::error("parse.error", e.message, e.span, None))
}

/// The pieces of a [`Validated`] document, plus its import diagnostics and
/// file map.
pub(crate) struct ValidatedParts {
    pub(crate) doc: Document,
    pub(crate) policy: DiagnosticPolicy,
    pub(crate) imports: LoadedImportGraph,
    /// Policy-governed validation diagnostics.
    pub(crate) diagnostics: Vec<Diagnostic>,
    /// Attributed import-loader diagnostics.
    pub(crate) import_diagnostics: Vec<Diagnostic>,
    pub(crate) import_files: ImportFiles,
}

impl Validated {
    /// Split into the pieces every entry point needs, adding the attributed
    /// import-loader diagnostics and the import file map.
    pub(crate) fn into_parts(self) -> ValidatedParts {
        let import_diagnostics = attributed_loader_diagnostics(&self.imports);
        let import_files = ImportFiles::from_graph(&self.imports);
        ValidatedParts {
            doc: self.doc,
            policy: self.policy,
            imports: self.imports,
            diagnostics: self.diagnostics,
            import_diagnostics,
            import_files,
        }
    }
}

/// Apply the merged `policy` to compile-stage diagnostics.
///
/// Compile diagnostics (`font.unresolved`, `font.glyph_missing`, …) come
/// from `zenith-scene` after validation, so they never pass the validation
/// choke point. This is where `deny` / `allow` / `warn` reach them. An empty
/// policy is an identity pass. The policy only relabels the list; it never
/// touches rendered bytes.
#[must_use]
pub fn govern_compile_diagnostics(
    diagnostics: Vec<Diagnostic>,
    policy: &DiagnosticPolicy,
) -> Vec<Diagnostic> {
    apply_policy(diagnostics, policy)
}

/// The 0-based index of 1-based `page` in `doc`.
///
/// # Errors
///
/// `render.page_out_of_range`, exit code 2, when the document has no pages
/// or `page` is outside `1..=pages`.
pub fn resolve_page_index(doc: &Document, page: usize) -> Result<usize, PipelineError> {
    let n = doc.body.pages.len();
    if doc.body.pages.is_empty() || page < 1 || page > n {
        return Err(PipelineError::new(
            "render.page_out_of_range",
            format!("page {page} out of range; document has {n} page(s); pass --page 1 to {n}"),
            2,
        ));
    }
    Ok(page - 1)
}
