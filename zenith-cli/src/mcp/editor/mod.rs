//! The `zenith_editor_*` MCP tools. Wiring only.
//!
//! - `registry` — this process's editor sessions (local and attached).
//! - `run` — the tool functions.

mod registry;
mod run;

pub(crate) use run::{attach, close, command, open, render, sessions};
