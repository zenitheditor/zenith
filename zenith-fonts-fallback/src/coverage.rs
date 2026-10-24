//! The code points each fallback face maps, as sorted inclusive ranges.
//! `build.rs` writes the tables from the font files.

include!(concat!(env!("OUT_DIR"), "/coverage.rs"));
