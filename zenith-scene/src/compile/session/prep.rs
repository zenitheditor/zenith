//! Document-level compile preparation, run once per document.

use std::collections::BTreeMap;

use zenith_core::{DataContext, Diagnostic, Document, ResolvedToken, resolve_tokens};

use super::super::ImageSizes;
use super::super::data_resolve::{scan_for_data_refs, substitute_data_refs};
use super::super::imports::{ImportGraph, ImportScopes};
use super::super::markdown_resolve::{MdBlockMap, resolve_markdown, scan_for_markdown_text};

/// Document-level compile results shared by every page of one document.
///
/// Holds the data-substituted document clone (when one is needed), the parsed
/// markdown blocks, the import scopes, the resolved tokens, and the
/// diagnostics every page reports first. Build it once, then compile pages
/// through a [`PageCompiler`](crate::compile::PageCompiler).
pub struct DocumentPrep<'d> {
    /// The caller's document.
    source: &'d Document,
    /// Data-substituted and markdown-resolved clone. `None` keeps `source`.
    owned: Option<Document>,
    /// Parsed block-level markdown, keyed by `text` node id.
    pub(in crate::compile) md_blocks: MdBlockMap,
    /// Data context, also passed to imported page sources.
    pub(in crate::compile) data: Option<&'d DataContext>,
    /// Import graph, also passed to imported page sources.
    pub(in crate::compile) imports: Option<&'d ImportGraph<'d>>,
    /// Imported document scopes for `instance source="…"`.
    pub(in crate::compile) import_scopes: ImportScopes<'d>,
    /// Resolved host token table.
    pub(in crate::compile) resolved: BTreeMap<String, ResolvedToken>,
    /// Data, import, and token diagnostics. Every page reports them first.
    pub(in crate::compile) shared_diagnostics: Vec<Diagnostic>,
    /// Pixel size `(w, h)` of each image / SVG asset, by asset id. Empty
    /// unless set with [`DocumentPrep::with_image_sizes`].
    pub(in crate::compile) image_sizes: ImageSizes,
}

impl<'d> DocumentPrep<'d> {
    /// Run the document-level compile steps for `doc`.
    ///
    /// Pass `Some(data)` to resolve `(data)"field.path"` references. Pass
    /// `Some(imports)` to enable `instance source="…"` and imported page
    /// sources. Diagnostics match the leading diagnostics of
    /// [`compile_page`](crate::compile::compile_page) exactly.
    #[must_use]
    pub fn new(
        doc: &'d Document,
        data: Option<&'d DataContext>,
        imports: Option<&'d ImportGraph<'d>>,
    ) -> Self {
        let mut diagnostics: Vec<Diagnostic> = Vec::new();

        // Data binding and markdown resolution. The document clones only when
        // a data context is given or a markdown text node exists.
        let mut md_blocks = MdBlockMap::new();
        let owned: Option<Document> = match data {
            Some(ctx) => {
                let mut cloned = doc.clone();
                substitute_data_refs(&mut cloned, ctx, &mut diagnostics);
                md_blocks = resolve_markdown(&mut cloned);
                Some(cloned)
            }
            None => {
                if scan_for_data_refs(doc) {
                    diagnostics.push(Diagnostic::advisory(
                        "data.no_context",
                        "document contains `(data)` references but no data context was \
                         provided at compile time; the references are left unresolved",
                        None,
                        None,
                    ));
                }
                if scan_for_markdown_text(doc) {
                    let mut cloned = doc.clone();
                    md_blocks = resolve_markdown(&mut cloned);
                    Some(cloned)
                } else {
                    None
                }
            }
        };
        let compiled: &Document = owned.as_ref().unwrap_or(doc);

        let import_scopes = match imports {
            Some(graph) => ImportScopes::from_graph(graph, compiled, &mut diagnostics),
            None => ImportScopes::disabled(),
        };

        let token_resolution = resolve_tokens(&compiled.tokens);
        diagnostics.extend(token_resolution.diagnostics);

        Self {
            source: doc,
            owned,
            md_blocks,
            data,
            imports,
            import_scopes,
            resolved: token_resolution.resolved,
            shared_diagnostics: diagnostics,
            image_sizes: ImageSizes::new(),
        }
    }

    /// Set the intrinsic pixel size `(w, h)` of each image / SVG asset, by
    /// asset id (an imported asset uses `import-id/asset-id`).
    ///
    /// Auto-layout sizes a hugging `image` from it: both axes omitted take
    /// the pixel size, one fixed axis keeps the aspect ratio. An image whose
    /// asset has no entry has no intrinsic size (`layout.unsized_child` when
    /// it hugs). Scene compilation reads no files, so the caller decodes the
    /// asset headers.
    #[must_use]
    pub fn with_image_sizes(mut self, sizes: BTreeMap<String, (f64, f64)>) -> Self {
        self.image_sizes = sizes;
        self
    }

    /// The document pages compile against: the substituted clone, else the
    /// caller's document.
    #[must_use]
    pub fn document(&self) -> &Document {
        self.owned.as_ref().unwrap_or(self.source)
    }

    /// Number of pages in the compiled document.
    #[must_use]
    pub fn page_count(&self) -> usize {
        self.document().body.pages.len()
    }

    /// Diagnostics every page reports before its own diagnostics.
    #[must_use]
    pub fn shared_diagnostics(&self) -> &[Diagnostic] {
        &self.shared_diagnostics
    }
}
