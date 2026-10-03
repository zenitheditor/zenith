//! Dispatch for the tool commands: `new`, `update`, `plugin`, `mcp`,
//! `fonts`, and `schema`.

use std::process::ExitCode;

use crate::cli::{self, FontsArgs, McpArgs, NewArgs, PluginArgs, SchemaArgs, UpdateArgs};
use crate::cli_helpers::{scope_from_arg, targets_from_flags};
use crate::report::CliError;
use crate::{commands, mcp, selfupdate};

use super::output::warn;

pub(super) fn dispatch_new(args: NewArgs) -> ExitCode {
    let page = match commands::new::resolve_page(
        args.format,
        args.width,
        args.height,
        args.landscape,
        args.pages,
    ) {
        Ok(p) => p,
        Err(msg) => return CliError::usage(msg).emit(false),
    };
    match commands::new::run(
        &args.path,
        args.name.as_deref(),
        page,
        args.theme.as_deref(),
    ) {
        Ok(result) => {
            if let Some(w) = &result.warning {
                warn(w);
            }
            println!(
                "created '{}' (doc-id: {})",
                result.path.display(),
                result.doc_id
            );
            if args.theme.is_none() {
                println!(
                    "tip: start on a theme with --theme <name> (run `zenith library list` to see themes)"
                );
            }
            ExitCode::SUCCESS
        }
        Err(e) => CliError::new("new.failed", e.message, e.exit_code).emit(false),
    }
}

pub(super) fn dispatch_update(args: UpdateArgs) -> ExitCode {
    match selfupdate::run(args.pre, args.version.as_deref()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => CliError::new("update.failed", format!("error: {msg}"), 2).emit(false),
    }
}

pub(super) fn dispatch_plugin(args: PluginArgs) -> ExitCode {
    let project_root = std::path::Path::new(".");
    match args.command {
        cli::PluginSub::Install(a) => ExitCode::from(commands::plugin::run_install(
            project_root,
            targets_from_flags(&a.agents),
            scope_from_arg(a.scope),
            a.force,
            a.dry_run,
        )),
        cli::PluginSub::Uninstall(a) => ExitCode::from(commands::plugin::run_uninstall(
            project_root,
            targets_from_flags(&a.agents),
            scope_from_arg(a.scope),
            a.dry_run,
        )),
        cli::PluginSub::List => ExitCode::from(commands::plugin::run_list(project_root)),
    }
}

pub(super) fn dispatch_mcp(args: McpArgs) -> ExitCode {
    match &args.http {
        Some(addr) => ExitCode::from(mcp::run_http(addr)),
        None => ExitCode::from(mcp::run()),
    }
}

pub(super) fn dispatch_fonts(args: FontsArgs) -> ExitCode {
    match args.command {
        None => {
            let (output, code) = commands::fonts::list(args.json);
            println!("{output}");
            ExitCode::from(code)
        }
        Some(cli::FontsSub::Features(a)) => print_coded(
            commands::fonts::features(&a.target, a.weight, &a.style, a.doc.as_deref(), a.json),
            a.json,
        ),
        Some(cli::FontsSub::Alternates(a)) => print_coded(
            commands::fonts::alternates(
                &a.target,
                &a.ch,
                a.weight,
                &a.style,
                a.doc.as_deref(),
                a.json,
            ),
            a.json,
        ),
    }
}

/// Print `output` to stdout on code 0. Otherwise it is a failure.
fn print_coded((output, code): (String, u8), json: bool) -> ExitCode {
    if code == 0 {
        println!("{output}");
        return ExitCode::SUCCESS;
    }
    CliError::new("fonts.failed", output, code).emit(json)
}

pub(super) fn dispatch_schema(args: SchemaArgs) -> ExitCode {
    let json = args.json;
    let (output, code) = match args.command {
        None => commands::schema::overview(json),
        Some(cli::SchemaSub::Nodes) => commands::schema::nodes(json),
        Some(cli::SchemaSub::Node { kind }) => commands::schema::node_detail(&kind, json),
        Some(cli::SchemaSub::Ops) => commands::schema::ops(json),
        Some(cli::SchemaSub::Op { name }) => commands::schema::op_detail(&name, json),
        Some(cli::SchemaSub::Tokens) => commands::schema::tokens(json),
        Some(cli::SchemaSub::Token { ty }) => commands::schema::token_detail(&ty, json),
        Some(cli::SchemaSub::Page) => commands::schema::page(json),
        Some(cli::SchemaSub::Asset) => commands::schema::asset(json),
        Some(cli::SchemaSub::Document) => commands::schema::document(json),
        Some(cli::SchemaSub::Ports) => commands::schema::ports(json),
        Some(cli::SchemaSub::Variant) => commands::schema::variant(json),
        Some(cli::SchemaSub::Diagnostics) => commands::schema::diagnostics(json),
        Some(cli::SchemaSub::Brand) => commands::schema::brand(json),
        Some(cli::SchemaSub::Block) => commands::schema::block(json),
        Some(cli::SchemaSub::Style) => commands::schema::style(json),
        Some(cli::SchemaSub::Defaults) => commands::schema::defaults(json),
    };
    println!("{}", output);
    ExitCode::from(code)
}
