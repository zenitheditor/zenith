//! Shared helpers for zenith-render integration tests.
//!
//! This module holds only what every test binary that declares `mod common;` uses.
//! Helpers used by a subset of binaries live in sibling files in this directory,
//! loaded with `#[path = "common/<name>.rs"]`. Helpers used by one binary live in
//! that binary.

/// Read straight-alpha RGBA8 pixel (px, py) from a buffer of the given `width`.
pub fn pixel(rgba: &[u8], width: u32, px: u32, py: u32) -> (u8, u8, u8, u8) {
    let base = ((py * width + px) * 4) as usize;
    (rgba[base], rgba[base + 1], rgba[base + 2], rgba[base + 3])
}
