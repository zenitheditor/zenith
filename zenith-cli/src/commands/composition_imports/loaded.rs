//! The [`LoadedImportGraph`] result type: parsed import documents plus the
//! diagnostics and edge records collected while traversing the graph.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use zenith_core::{Diagnostic, Document};
use zenith_scene::ImportGraph as SceneImportGraph;

/// Outcome of loading one import declaration during graph traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImportEdgeStatus {
    /// File loaded and (when declared) hash matched.
    Ok,
    /// Resolved path could not be read.
    Missing,
    /// Declared `sha256` does not match file bytes.
    HashMismatch,
    /// File bytes were not valid `.zen`.
    ParseError,
    /// Import graph contains a cycle through this edge.
    Cycle,
    /// No project directory; path cannot be resolved.
    Unresolvable,
    /// Non-`zen` kind; loader does not open the file.
    SkippedKind,
}

impl ImportEdgeStatus {
    /// Stable wire / CLI label for this status.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Missing => "missing",
            Self::HashMismatch => "hash_mismatch",
            Self::ParseError => "parse_error",
            Self::Cycle => "cycle",
            Self::Unresolvable => "unresolvable",
            Self::SkippedKind => "skipped_kind",
        }
    }

    /// `Ok` when the declared hash matched (or was absent); `HashMismatch` otherwise.
    pub(crate) fn from_hash_ok(hash_ok: bool) -> Self {
        if hash_ok {
            Self::Ok
        } else {
            Self::HashMismatch
        }
    }
}

/// One import-declaration edge recorded during [`super::load_import_graph`].
///
/// Edges are additive metadata for inspection (`zenith imports list`). Render
/// and validate consume documents/diagnostics only and ignore this list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportEdge {
    /// Absolute (normalized) path of the importing document; `None` for host.
    pub(crate) importer: Option<PathBuf>,
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) src: String,
    /// Normalized filesystem path when a base directory was available.
    pub(crate) resolved_path: Option<PathBuf>,
    pub(crate) sha256_declared: Option<String>,
    /// Hex digest of file bytes when the file was read (or served from cache).
    pub(crate) sha256_actual: Option<String>,
    pub(crate) status: ImportEdgeStatus,
    /// Host declarations are depth `0`; each nested import level increments by one.
    pub(crate) depth: u32,
}

/// Parsed import graph plus diagnostics and edge records from traversal.
#[derive(Debug)]
pub(crate) struct LoadedImportGraph {
    pub(super) diagnostics: Vec<Diagnostic>,
    pub(super) documents: BTreeMap<String, Document>,
    pub(super) document_dirs: BTreeMap<String, PathBuf>,
    /// Import-declaration edges in deterministic traversal order.
    pub(super) edges: Vec<ImportEdge>,
}

impl LoadedImportGraph {
    /// Consume the graph and return diagnostics in deterministic traversal order.
    #[cfg(test)]
    pub(crate) fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    /// Diagnostics collected while loading imports.
    pub(crate) fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Import-declaration edges recorded during load (ignored by render/validate).
    pub(crate) fn edges(&self) -> &[ImportEdge] {
        &self.edges
    }

    /// Build a borrowed scene import graph for compile-time expansion.
    pub(crate) fn to_scene_graph(&self) -> SceneImportGraph<'_> {
        let mut graph = SceneImportGraph::new();
        for (id, doc) in &self.documents {
            graph.insert(id.as_str(), doc);
        }
        graph
    }

    pub(crate) fn documents_with_dirs(&self) -> impl Iterator<Item = (&str, &Document, &Path)> {
        self.documents.iter().filter_map(|(id, doc)| {
            self.document_dirs
                .get(id)
                .map(|dir| (id.as_str(), doc, dir.as_path()))
        })
    }
}
