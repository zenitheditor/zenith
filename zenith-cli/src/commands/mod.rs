//! Command implementations for the Zenith CLI.
//!
//! Each submodule exposes a pure function whose core logic operates on
//! in-memory source bytes/strings — never touching the filesystem.  File I/O
//! (reading the document, writing formatted source or rendered output) is the
//! responsibility of the dispatcher in `lib.rs`.
//! - `asset` — asset import transaction prep + result rendering.
//! - `format` — shared JSON serialisation and diagnostic-line formatting.

pub mod asset;
pub(crate) mod composition_imports;
pub mod fix;
pub mod fmt;
pub mod fonts;
pub(crate) mod format;
pub mod inspect;
pub mod library;
pub mod merge;
pub mod new;
pub mod perceive;
pub mod plugin;
pub mod render;
pub mod schema;
pub mod theme;
pub mod tokens;
pub mod tx;
pub mod validate;
pub mod variant;
pub mod workspace;

pub(crate) use format::{
    format_diagnostic_line, format_located_diagnostic_line, serialize_compact, serialize_pretty,
};
