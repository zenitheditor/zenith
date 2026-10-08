//! Command dispatch: parse arguments and route each subcommand to its
//! dispatch submodule. Wiring only.
//!
//! Each submodule owns the file I/O edge for its commands. Business logic
//! lives in `commands/`.

use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Command};

mod asset;
mod batch;
mod document;
mod edit;
mod fix;
mod history;
mod library;
mod output;
mod render;
mod tools;
mod workspace;

/// Main entry point: parse CLI arguments, dispatch to the appropriate command,
/// and return its exit code.
pub fn run() -> ExitCode {
    match Cli::parse().command {
        Command::New(args) => tools::dispatch_new(args),
        Command::Validate(args) => document::dispatch_validate(args),
        Command::Fmt(args) => document::dispatch_fmt(args),
        Command::Fix(args) => fix::dispatch_fix(args),
        Command::Tokens(args) => document::dispatch_tokens(args),
        Command::Render(args) => render::dispatch_render(*args),
        Command::Inspect(args) => document::dispatch_inspect(args),
        Command::Imports(args) => document::dispatch_imports(args),
        Command::Perceive(args) => document::dispatch_perceive(args),
        Command::Merge(args) => batch::dispatch_merge(args),
        Command::Asset(args) => asset::dispatch_asset(args),
        Command::Library(args) => library::dispatch_library(args),
        Command::History(args) => history::dispatch_history(args),
        Command::Undo(args) => history::dispatch_undo(args),
        Command::Redo(args) => history::dispatch_redo(args),
        Command::Version(args) => history::dispatch_version(args),
        Command::Restore(args) => history::dispatch_restore(args),
        Command::Sync(args) => history::dispatch_sync(args),
        Command::Tx(args) => edit::dispatch_tx(args),
        Command::OutlineText(args) => edit::dispatch_outline_text(args),
        Command::Variant(args) => batch::dispatch_variant(args),
        Command::Update(args) => tools::dispatch_update(args),
        Command::Theme(args) => edit::dispatch_theme(args),
        Command::Plugin(args) => tools::dispatch_plugin(args),
        Command::Mcp(args) => tools::dispatch_mcp(args),
        Command::Fonts(args) => tools::dispatch_fonts(args),
        Command::Schema(args) => tools::dispatch_schema(args),
        Command::Workspace(args) => workspace::dispatch_workspace(args),
    }
}
