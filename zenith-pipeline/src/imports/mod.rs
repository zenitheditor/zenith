//! `.zen` composition import graph loading through a [`SourceFs`](crate::SourceFs).
//!
//! Core owns syntax and local validation. This module owns load-time work:
//! resolving import paths relative to the importing document, parsing
//! imported documents, checking declared source hashes, and detecting cycles.
//!
//! Wiring only; the concerns live in submodules:
//! - `loaded` — the [`LoadedImportGraph`] result type and import edge records.
//! - `loader` — recursive traversal, parsing, hash verification, cycle detection.
//! - `validate` — root-target validation and expanded-id collision detection.
//! - `diagnostics` — the `import.*` diagnostic constructors.
//! - `source` — import-source string parsing.
//! - `walk` — node-tree walks and page-size comparison.
//! - `files` — the import file map behind diagnostic spans.

mod diagnostics;
mod files;
mod loaded;
mod loader;
mod source;
mod validate;
mod walk;

#[cfg(test)]
mod tests;

pub use files::{ImportFiles, attributed_loader_diagnostics};
pub use loaded::{ImportEdge, ImportEdgeStatus, LoadedImportGraph};
pub use loader::load_import_graph;
pub use source::{ImportSource, parse_import_source};
