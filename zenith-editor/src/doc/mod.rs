//! Reading a parsed document for editing. Wiring only.
//!
//! - `tree` — node lookup, ancestors, and ids in use.
//! - `shape` — authored geometry of lines, polygons, and paths.
//! - `place` — the page a node draws on and its compiled box id.

pub(crate) mod place;
pub(crate) mod shape;
pub(crate) mod tree;
