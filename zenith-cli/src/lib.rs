//! Command-line interface library for Zenith.
//!
//! Owns all command dispatch, argument parsing (via clap), JSON I/O shaping,
//! and human-readable stdout/stderr formatting.
//!
//! `src/main.rs` is kept thin — it only calls [`run`].
//! `zenith-layout` is reached transitively through `zenith-scene`; the CLI
//! never constructs layout types directly.
//!
//! # Module layout
//!
//! - `cli` — clap `#[derive(Parser)]` types.
//! - `commands/` — one module per subcommand; all business logic is here,
//!   operating on in-memory bytes, never touching the FS.
//! - `json_types` — serialisable DTOs for JSON output.
//! - `lib.rs` — this file: wiring + `run()` dispatcher + file I/O edge.

pub mod cli;
pub mod commands;
pub mod history;
pub mod json_types;
pub mod library;
pub mod mcp;
pub mod selfupdate;

mod cli_helpers;
mod dispatch;

pub use dispatch::run;
