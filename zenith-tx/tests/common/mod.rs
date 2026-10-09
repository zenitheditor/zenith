//! Helper shared by every integration-test binary in `zenith-tx/tests/` that
//! declares `mod common;`. Items used by only some binaries live in sibling
//! files here, loaded with `#[path]`; single-use items live in their test file.

use zenith_core::{Document, KdlAdapter, KdlSource};

/// Parse a KDL source string into a [`Document`], panicking on failure.
pub fn parse(src: &str) -> Document {
    KdlAdapter
        .parse(src.as_bytes())
        .expect("test doc must parse")
}
