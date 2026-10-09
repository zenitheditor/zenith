//! Source files behind diagnostic spans.
//!
//! A span indexes into one file. Most spans index into the host document. A
//! diagnostic with [`Diagnostic::import`] set indexes into the document of that
//! composition import. [`ImportFiles`] maps import ids to those files, so a
//! reporter computes `line`/`col` against the right text or omits them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use zenith_core::Diagnostic;

use super::loaded::LoadedImportGraph;

/// Import-loader codes whose span indexes into the file that declares the
/// import, which is an imported file for nested imports.
const LOADER_CODES: &[&str] = &[
    "import.hash_mismatch",
    "import.missing",
    "import.cycle",
    "import.parse_error",
];

/// Import id to resolved `.zen` file path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportFiles {
    files: BTreeMap<String, PathBuf>,
}

impl ImportFiles {
    /// Collect the resolved path of every composition import in `graph`.
    #[must_use]
    pub fn from_graph(graph: &LoadedImportGraph) -> Self {
        let mut files = BTreeMap::new();
        for edge in graph.edges() {
            if edge.kind != "zen" {
                continue;
            }
            if let Some(path) = &edge.resolved_path {
                files.entry(edge.id.clone()).or_insert_with(|| path.clone());
            }
        }
        Self { files }
    }

    /// Add every entry of `other`. Existing ids keep their path.
    pub fn extend(&mut self, other: &ImportFiles) {
        for (id, path) in &other.files {
            self.files.entry(id.clone()).or_insert_with(|| path.clone());
        }
    }

    /// The file of import `id`, when known.
    #[must_use]
    pub fn path(&self, id: &str) -> Option<&Path> {
        self.files.get(id).map(PathBuf::as_path)
    }
}

/// The import-loader diagnostics of `graph`, each tagged with the import whose
/// file holds its span.
///
/// A loader diagnostic names the failing import in `subject_id`. The span sits
/// in the file that declares that import. The host declares it for a top-level
/// import, so the diagnostic stays untagged. An imported file declares it for a
/// nested import, so the diagnostic is tagged with the id of that file's own
/// import. When the declaring file is ambiguous, the span is dropped so no
/// reporter maps it onto the wrong text.
#[must_use]
pub fn attributed_loader_diagnostics(graph: &LoadedImportGraph) -> Vec<Diagnostic> {
    graph
        .diagnostics()
        .iter()
        .map(|d| attribute(d.clone(), graph))
        .collect()
}

fn attribute(mut d: Diagnostic, graph: &LoadedImportGraph) -> Diagnostic {
    if !LOADER_CODES.contains(&d.code.as_str()) || d.span.is_none() {
        return d;
    }
    let Some(subject) = d.subject_id.as_deref() else {
        return d;
    };
    let mut importers: Vec<Option<&Path>> = graph
        .edges()
        .iter()
        .filter(|e| e.id == subject)
        .map(|e| e.importer.as_deref())
        .collect();
    importers.sort();
    importers.dedup();
    match importers.as_slice() {
        [] | [None] => d,
        [Some(importer)] => {
            let owner = graph
                .edges()
                .iter()
                .find(|e| e.kind == "zen" && e.resolved_path.as_deref() == Some(*importer))
                .map(|e| e.id.clone());
            match owner {
                Some(id) => d.with_import(id),
                None => {
                    d.span = None;
                    d
                }
            }
        }
        _ => {
            d.span = None;
            d
        }
    }
}
