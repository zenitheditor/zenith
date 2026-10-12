//! Dispatch for the tool commands: `new`, `update`, `plugin`, `mcp`,
//! `fonts`, and `schema`.

use std::process::ExitCode;

use crate::cli::{self, FontsArgs, McpArgs, NewArgs, PluginArgs, SchemaArgs, UpdateArgs};
use crate::cli_helpers::{scope_from_arg, shell_word, targets_from_flags};
use crate::report::CliError;
use crate::{commands, mcp, selfupdate};

use super::output::warn;

pub(super) fn dispatch_new(args: NewArgs) -> ExitCode {
    if let Some(starter) = args.starter {
        let size = commands::new::StarterSize {
            format: args.format,
            width: args.width,
            height: args.height,
            landscape: args.landscape,
        };
        return report_new(
            commands::new::run_starter(
                &args.path,
                args.name.as_deref(),
                starter,
                args.theme.as_deref(),
                size,
            ),
            NewTip::Starter,
        );
    }
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
    let result = commands::new::run(
        &args.path,
        args.name.as_deref(),
        page,
        args.theme.as_deref(),
    );
    let tip = if args.theme.is_none() {
        NewTip::Blank
    } else {
        NewTip::None
    };
    report_new(result, tip)
}

/// The hint `zenith new` prints after it created a file.
#[derive(Clone, Copy)]
enum NewTip {
    /// A blank page with no theme: point at starters and themes.
    Blank,
    /// A starter was written: say how to adapt it.
    Starter,
    /// A blank page on a theme: no hint.
    None,
}

/// After a starter: the text slots and how to fill them in one transaction.
fn print_starter_tip(path: &std::path::Path) {
    let shown = shell_word(&path.display().to_string());
    let slots = std::fs::read(path).ok().and_then(|bytes| {
        use zenith_core::KdlSource as _;
        let doc = zenith_core::KdlAdapter.parse(&bytes).ok()?;
        let resolved = zenith_core::resolve_tokens(&doc.tokens).resolved;
        Some(commands::new::text_slots(
            &commands::inspect::build_doc_tree(&doc.body.pages, &resolved),
        ))
    });
    match slots {
        Some(slots) if !slots.is_empty() => {
            println!("{}", commands::new::slot_listing(&slots, &shown, 40));
        }
        _ => println!("note: could not list the text slots; run `zenith inspect {shown}`."),
    }
    println!(
        "other edits, each an op for --ops (`zenith schema op <name>` has the fields of any other):\n{}\n",
        crate::cli::COMMON_EDITS
    );
    println!(
        "This is a finished layout: replace the text above, restyle with `zenith tokens {shown}` and {{\"op\":\"update_token_value\",\"id\":\"color.primary\",\"value\":\"#rrggbb\"}}, and add or remove nodes (`duplicate_node` repeats a row). `zenith inspect {shown}` prints its layout outline (id, box, layout, text per node). Add `--png out.png` to `zenith tx --apply` to see the result in the same call. Every text node is marked role=\"placeholder\": `zenith validate {shown}` warns once (content.placeholder), naming the first ones you leave, and a `tx` text op on a node clears its mark. To write nodes by hand, `zenith schema write` has the syntax and common attributes."
    );
}

/// The starter names, `|`-separated.
fn starter_names() -> String {
    <commands::new::Starter as clap::ValueEnum>::value_variants()
        .iter()
        .filter_map(|s| clap::ValueEnum::to_possible_value(s).map(|v| v.get_name().to_owned()))
        .collect::<Vec<_>>()
        .join("|")
}

/// Print the outcome of `zenith new` and the hint `tip` selects.
fn report_new(
    result: Result<commands::new::NewResult, commands::new::NewErr>,
    tip: NewTip,
) -> ExitCode {
    match result {
        Ok(result) => {
            if let Some(w) = &result.warning {
                warn(w);
            }
            println!(
                "created '{}' (doc-id: {})",
                result.path.display(),
                result.doc_id
            );
            match tip {
                NewTip::Blank => println!(
                    "tip: a finished layout is faster than a blank page: --starter {} (add --theme <name> and a size flag to restyle and resize it)",
                    starter_names()
                ),
                NewTip::Starter => print_starter_tip(&result.path),
                NewTip::None => {}
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
    // Over HTTP, tool paths stay under the working directory unless a root
    // is given: a leaked token then cannot reach the rest of the disk.
    let root = match (&args.root, &args.http) {
        (Some(root), _) => Some(root.clone()),
        (None, Some(_)) => match std::env::current_dir() {
            Ok(dir) => Some(dir),
            Err(e) => {
                eprintln!("zenith mcp: cannot read the working directory ({e}); pass --root <DIR>");
                return ExitCode::from(2);
            }
        },
        (None, None) => None,
    };
    if let Some(root) = root {
        match mcp::set_root(&root) {
            Ok(root) => eprintln!("zenith mcp: tool paths stay under '{}'", root.display()),
            Err(e) => {
                eprintln!("zenith mcp: {e}");
                return ExitCode::from(2);
            }
        }
    }
    match args.http {
        Some(addr) => ExitCode::from(mcp::run_http(&mcp::HttpOptions {
            addr,
            allow_remote: args.allow_remote,
            allow_hosts: args.allow_host,
        })),
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
        Some(cli::SchemaSub::Write) => commands::schema::write(json),
        Some(cli::SchemaSub::Nodes) => commands::schema::nodes(json),
        Some(cli::SchemaSub::Node { kind, notes }) => {
            commands::schema::node_detail_with(&kind, json, notes)
        }
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
