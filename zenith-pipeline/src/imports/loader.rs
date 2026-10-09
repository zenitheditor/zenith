//! The import-graph loader: recursive traversal through a [`SourceFs`], per-import parsing,
//! hash verification, and cycle detection. Validation and diagnostic
//! constructors live in sibling submodules.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use zenith_core::{Diagnostic, Document, ImportDecl, KdlAdapter, KdlSource as _};

use super::loaded::{ImportEdge, ImportEdgeStatus, LoadedImportGraph};
use crate::io::SourceFs;
use crate::path::normalize_import_path;

/// Load every reachable `kind="zen"` composition import from `root`, reading
/// files through `fs`.
///
/// `root_dir` is the parent directory of the root `.zen` source. When absent,
/// `kind="zen"` declarations are unresolvable (`import.missing`); non-`zen`
/// kinds are recorded as `skipped_kind` only. Declared `sha256` values are
/// always verified when present.
///
/// Every import declaration (including non-`zen` kinds and failed loads) is
/// recorded as an [`ImportEdge`] on the returned graph for inspection.
pub fn load_import_graph(
    fs: &dyn SourceFs,
    root: &Document,
    root_dir: Option<&Path>,
) -> LoadedImportGraph {
    let mut loader = ImportGraphLoader {
        fs,
        diagnostics: Vec::new(),
        documents: BTreeMap::new(),
        document_dirs: BTreeMap::new(),
        documents_by_path: BTreeMap::new(),
        stack: Vec::new(),
        edges: Vec::new(),
    };
    match root_dir {
        Some(dir) => loader.load_document_imports(root, dir, None),
        None => loader.report_unresolvable_root(root),
    }
    loader.validate_root_targets(root);
    loader.detect_id_collisions(root);
    loader.finish()
}

pub(super) struct ImportGraphLoader<'f> {
    fs: &'f dyn SourceFs,
    pub(super) diagnostics: Vec<Diagnostic>,
    pub(super) documents: BTreeMap<String, Document>,
    document_dirs: BTreeMap<String, PathBuf>,
    documents_by_path: BTreeMap<PathBuf, CachedImportDocument>,
    pub(super) stack: Vec<PathBuf>,
    edges: Vec<ImportEdge>,
}

#[derive(Debug)]
struct CachedImportDocument {
    document: Document,
    sha256: String,
}

impl ImportGraphLoader<'_> {
    fn finish(self) -> LoadedImportGraph {
        LoadedImportGraph {
            diagnostics: self.diagnostics,
            documents: self.documents,
            document_dirs: self.document_dirs,
            edges: self.edges,
        }
    }

    /// Record one import-declaration edge (shared field mapping from `import`).
    fn record_edge(
        &mut self,
        import: &ImportDecl,
        importer: Option<&Path>,
        resolved_path: Option<PathBuf>,
        sha256_actual: Option<String>,
        status: ImportEdgeStatus,
        depth: u32,
    ) {
        self.edges.push(ImportEdge {
            importer: importer.map(Path::to_path_buf),
            id: import.id.clone(),
            kind: import.kind.clone(),
            src: import.src.clone(),
            resolved_path,
            sha256_declared: import.sha256.clone(),
            sha256_actual,
            status,
            depth,
        });
    }

    fn report_unresolvable_root(&mut self, doc: &Document) {
        for import in &doc.imports {
            if import.kind != "zen" {
                self.record_edge(import, None, None, None, ImportEdgeStatus::SkippedKind, 0);
                continue;
            }
            self.push_missing(
                import,
                format!(
                    "import '{}' cannot be resolved without a project directory",
                    import.id
                ),
            );
            self.record_edge(import, None, None, None, ImportEdgeStatus::Unresolvable, 0);
        }
    }

    fn load_document_imports(&mut self, doc: &Document, base_dir: &Path, importer: Option<&Path>) {
        let depth = self.stack.len() as u32;
        for import in &doc.imports {
            if import.kind != "zen" {
                let path = normalize_import_path(base_dir, &import.src);
                self.record_edge(
                    import,
                    importer,
                    Some(path),
                    None,
                    ImportEdgeStatus::SkippedKind,
                    depth,
                );
                continue;
            }
            self.load_one_import(import, base_dir, importer, depth);
        }
    }

    fn load_one_import(
        &mut self,
        import: &ImportDecl,
        base_dir: &Path,
        importer: Option<&Path>,
        depth: u32,
    ) {
        let path = normalize_import_path(base_dir, &import.src);

        if self.stack.contains(&path) {
            self.push_cycle(import, &path);
            self.record_edge(
                import,
                importer,
                Some(path),
                None,
                ImportEdgeStatus::Cycle,
                depth,
            );
            return;
        }
        if let Some(cached) = self.documents_by_path.get(&path) {
            let cached_sha256 = cached.sha256.clone();
            let cached_document = cached.document.clone();
            let hash_ok = self.verify_hash(import, &cached_sha256);
            self.documents.insert(import.id.clone(), cached_document);
            if let Some(parent) = path.parent() {
                self.document_dirs
                    .insert(import.id.clone(), parent.to_path_buf());
            }
            self.record_edge(
                import,
                importer,
                Some(path),
                Some(cached_sha256),
                ImportEdgeStatus::from_hash_ok(hash_ok),
                depth,
            );
            return;
        }

        let bytes = match self.fs.read(&path) {
            Ok(bytes) => bytes,
            Err(err) => {
                self.push_missing(
                    import,
                    format!(
                        "import '{}' file not found: '{}': {}",
                        import.id,
                        path.display(),
                        err
                    ),
                );
                self.record_edge(
                    import,
                    importer,
                    Some(path),
                    None,
                    ImportEdgeStatus::Missing,
                    depth,
                );
                return;
            }
        };

        let actual_sha256 = format!("{:x}", Sha256::digest(&bytes));
        let hash_ok = self.verify_hash(import, &actual_sha256);

        let doc = match KdlAdapter.parse(bytes.as_slice()) {
            Ok(doc) => doc,
            Err(err) => {
                self.push_parse_error(import, &path, &err.message);
                self.record_edge(
                    import,
                    importer,
                    Some(path),
                    Some(actual_sha256),
                    ImportEdgeStatus::ParseError,
                    depth,
                );
                return;
            }
        };

        self.record_edge(
            import,
            importer,
            Some(path.clone()),
            Some(actual_sha256.clone()),
            ImportEdgeStatus::from_hash_ok(hash_ok),
            depth,
        );

        self.stack.push(path.clone());
        if let Some(next_base) = path.parent() {
            self.load_document_imports(&doc, next_base, Some(&path));
        }
        self.stack.pop();
        let document_dir = path.parent().map(Path::to_path_buf);
        self.documents_by_path.insert(
            path,
            CachedImportDocument {
                document: doc.clone(),
                sha256: actual_sha256,
            },
        );
        if let Some(dir) = document_dir {
            self.document_dirs.insert(import.id.clone(), dir);
        }
        self.documents.insert(import.id.clone(), doc);
    }
}
