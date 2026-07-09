//! In-memory import graph support for scene compilation.
//!
//! The scene crate owns only borrowed, already-parsed documents here. It does
//! not perform filesystem or CLI lookups.

use std::collections::BTreeMap;

use zenith_core::{
    ComponentDef, Diagnostic, Document, ImportDecl, Page, ResolvedToken, Style, resolve_tokens,
};

use super::ComponentMap;

/// A parsed document made available to scene compilation as an import.
#[derive(Debug, Clone, Copy)]
pub struct ImportedDocument<'a> {
    /// The imported document AST.
    pub document: &'a Document,
}

impl<'a> ImportedDocument<'a> {
    /// Create an imported-document entry from an already parsed document.
    pub fn new(document: &'a Document) -> Self {
        Self { document }
    }
}

/// Deterministic, filesystem-free graph of imported documents.
#[derive(Debug, Clone, Default)]
pub struct ImportGraph<'a> {
    documents: BTreeMap<String, ImportedDocument<'a>>,
}

impl<'a> ImportGraph<'a> {
    /// Create an empty import graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or replace an import id with a parsed document.
    pub fn insert(&mut self, id: impl Into<String>, document: &'a Document) {
        self.documents
            .insert(id.into(), ImportedDocument::new(document));
    }

    /// Return a new graph containing `id -> document`.
    pub fn with_document(mut self, id: impl Into<String>, document: &'a Document) -> Self {
        self.insert(id, document);
        self
    }
}

pub(in crate::compile) struct ImportScopes<'a> {
    enabled: bool,
    scopes: BTreeMap<String, ImportedScope<'a>>,
}

impl<'a> ImportScopes<'a> {
    pub(in crate::compile) fn disabled() -> Self {
        Self {
            enabled: false,
            scopes: BTreeMap::new(),
        }
    }

    /// Build the import scopes from `graph`, threading the HOST document so that
    /// each import's declared `token-map` entries can bridge a host token into
    /// the imported subtree (see [`apply_token_maps`]).
    ///
    /// Host token resolution here does NOT push its diagnostics: they are already
    /// surfaced by the main compile token-resolution step. Imported-document token
    /// diagnostics ARE surfaced (they have no other emission site).
    pub(in crate::compile) fn from_graph(
        graph: &'a ImportGraph<'a>,
        host: &Document,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Self {
        let host_resolved = resolve_tokens(&host.tokens).resolved;
        let host_imports: BTreeMap<&str, &ImportDecl> = host
            .imports
            .iter()
            .map(|import| (import.id.as_str(), import))
            .collect();

        let mut scopes = BTreeMap::new();
        for (id, imported) in &graph.documents {
            let token_resolution = resolve_tokens(&imported.document.tokens);
            diagnostics.extend(token_resolution.diagnostics);

            let style_map: BTreeMap<&str, &Style> = imported
                .document
                .styles
                .styles
                .iter()
                .map(|style| (style.id.as_str(), style))
                .collect();

            let mut component_map: BTreeMap<&str, &ComponentDef> = BTreeMap::new();
            for component in &imported.document.components {
                component_map
                    .entry(component.id.as_str())
                    .or_insert(component);
            }

            let mut page_map: BTreeMap<&str, &Page> = BTreeMap::new();
            for page in &imported.document.body.pages {
                page_map.entry(page.id.as_str()).or_insert(page);
            }

            let mut resolved = token_resolution.resolved;
            if let Some(import) = host_imports.get(id.as_str()) {
                apply_token_maps(import, &host_resolved, &mut resolved, diagnostics);
            }

            scopes.insert(
                id.clone(),
                ImportedScope {
                    document: imported.document,
                    resolved,
                    style_map,
                    components: component_map,
                    pages: page_map,
                },
            );
        }

        Self {
            enabled: true,
            scopes,
        }
    }

    pub(in crate::compile) fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(in crate::compile) fn get(&self, id: &str) -> Option<&ImportedScope<'a>> {
        self.scopes.get(id)
    }
}

// NOTE: `import.token_unresolved` is registered in the catalog but intentionally
// NOT emitted here. Token maps only ever INSERT/override entries in an import
// scope's resolved table (never remove them), so a token that an imported node
// references is missing after mapping only if the imported document itself never
// defined it — a broken-imported-document condition already surfaced generically
// by `scene.unresolved_token` when the imported subtree is compiled. Emitting a
// distinct import-scoped code for the same condition would require making the
// shared color/geometry resolver stack import-aware (a large refactor) purely to
// duplicate an existing diagnostic, so the code is reserved rather than faked.

/// Apply an import's `token-map` entries, bridging host tokens into the
/// imported scope's resolved table.
///
/// `token-map from="X" to="Y"` overrides the imported token `X` with the host's
/// resolved token `Y`, so references to `X` inside the imported subtree paint
/// with the host value. When the host lacks `Y`, an `import.token_conflict`
/// diagnostic is emitted and the imported value is left untouched (isolation).
fn apply_token_maps(
    import: &ImportDecl,
    host_resolved: &BTreeMap<String, ResolvedToken>,
    scope_resolved: &mut BTreeMap<String, ResolvedToken>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for map in &import.token_maps {
        match host_resolved.get(&map.to) {
            Some(host_token) => {
                scope_resolved.insert(map.from.clone(), host_token.clone());
            }
            None => {
                diagnostics.push(Diagnostic::warning(
                    "import.token_conflict",
                    format!(
                        "import '{}' token-map target '{}' is not a resolved token in the host document; the mapping is ignored",
                        import.id, map.to
                    ),
                    map.source_span,
                    Some(import.id.clone()),
                ));
            }
        }
    }
}

pub(in crate::compile) struct ImportedScope<'a> {
    pub(in crate::compile) document: &'a Document,
    pub(in crate::compile) resolved: BTreeMap<String, ResolvedToken>,
    pub(in crate::compile) style_map: BTreeMap<&'a str, &'a Style>,
    pub(in crate::compile) components: ComponentMap<'a>,
    pub(in crate::compile) pages: BTreeMap<&'a str, &'a Page>,
}

pub(in crate::compile) enum ImportSource<'a> {
    Component {
        import_id: &'a str,
        component_id: &'a str,
    },
    Page {
        import_id: &'a str,
        page_id: &'a str,
    },
    UnsupportedTarget {
        import_id: &'a str,
        target: &'a str,
    },
    Invalid,
}

pub(in crate::compile) fn parse_import_source(source: &str) -> ImportSource<'_> {
    let Some((import_id, target)) = source.split_once('#') else {
        return ImportSource::Invalid;
    };
    if import_id.is_empty() || target.is_empty() {
        return ImportSource::Invalid;
    }
    if target.contains('#') {
        return ImportSource::Invalid;
    }

    if let Some(component_id) = target.strip_prefix("component.") {
        if component_id.is_empty() {
            return ImportSource::Invalid;
        }
        return ImportSource::Component {
            import_id,
            component_id,
        };
    }

    if let Some(page_id) = target.strip_prefix("page.") {
        if page_id.is_empty() {
            return ImportSource::Invalid;
        }
        return ImportSource::Page { import_id, page_id };
    }

    ImportSource::UnsupportedTarget { import_id, target }
}
