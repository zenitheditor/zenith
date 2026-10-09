//! Shared helpers for zenith-scene integration tests.
//!
//! This module holds only what every test binary that declares `mod common;` uses.
//! Helpers used by a subset of binaries live in sibling files in this directory,
//! loaded with `#[path = "common/<name>.rs"]`. Helpers used by one binary live in
//! that binary.

use zenith_core::{Document, KdlAdapter, KdlSource};

/// Parse a `.zen` source string.
pub fn parse(src: &str) -> Document {
    KdlAdapter
        .parse(src.as_bytes())
        .expect("test document must parse")
}
