//! Dispatch for the read-mostly document commands: `validate`, `fmt`,
//! `tokens`, `inspect`, `imports`, and `perceive`.
//!
//! Every failure goes through [`CliError::emit`]: a `zenith-error-v1`
//! envelope on stdout under `--json`, else text on stderr.

use std::process::ExitCode;

use crate::cli::{self, FmtArgs, ImportsArgs, InspectArgs, PerceiveArgs, TokensArgs, ValidateArgs};
use crate::cli_helpers::{parse_at_spec, read_file};
use crate::commands;
use crate::report::CliError;

use super::output::apply_edit;

pub(super) fn dispatch_validate(args: ValidateArgs) -> ExitCode {
    let src = match read_file(&args.path) {
        Ok(s) => s,
        Err(e) => return e.emit(args.json),
    };
    let flags = zenith_pipeline::PolicyFlags {
        allow: args.allow,
        warn: args.warn,
        deny: args.deny,
    };
    let out = commands::validate::run(&src, args.path.parent(), args.json, &flags);
    println!("{}", out.stdout);
    ExitCode::from(out.exit_code)
}

pub(super) fn dispatch_fmt(args: FmtArgs) -> ExitCode {
    let src = match read_file(&args.path) {
        Ok(s) => s,
        Err(e) => return e.emit(args.json),
    };
    match commands::fmt::run(&src) {
        Ok(result) => {
            if let Err(e) = apply_edit(&args.path, &result.formatted, "fmt.apply") {
                return e.emit(args.json);
            }
            println!("{}", commands::fmt::render_stdout(&result, args.json));
            ExitCode::SUCCESS
        }
        Err(e) => CliError::new("fmt.failed", e.message, e.exit_code).emit(args.json),
    }
}

pub(super) fn dispatch_tokens(args: TokensArgs) -> ExitCode {
    let src = match read_file(&args.path) {
        Ok(s) => s,
        Err(e) => return e.emit(args.json),
    };
    match commands::tokens::list(&src, args.json) {
        Ok(out) => {
            println!("{}", out);
            ExitCode::SUCCESS
        }
        Err((msg, code)) => CliError::new("tokens.failed", msg, code).emit(args.json),
    }
}

pub(super) fn dispatch_inspect(args: InspectArgs) -> ExitCode {
    match args.command {
        Some(cli::InspectSub::Path(path_args)) => {
            let json = path_args.json;
            let src = match read_file(&path_args.path) {
                Ok(s) => s,
                Err(e) => return e.emit(json),
            };
            match commands::inspect::path::run(
                &src,
                &path_args.node_id,
                path_args.json,
                path_args.craft,
            ) {
                Ok(out) => {
                    println!("{}", out);
                    ExitCode::SUCCESS
                }
                Err(e) => CliError::new("inspect.failed", e.message, e.exit_code).emit(json),
            }
        }
        None => {
            let json = args.json;
            let Some(path) = args.path.as_ref() else {
                return CliError::usage(
                    "error: missing document path; run `zenith inspect <FILE.zen>`",
                )
                .emit(json);
            };
            let src = match read_file(path) {
                Ok(s) => s,
                Err(e) => return e.emit(json),
            };
            match commands::inspect::run(&src, args.node.as_deref(), json, path.parent()) {
                Ok(out) => {
                    println!("{}", out);
                    ExitCode::SUCCESS
                }
                Err(e) => CliError::new("inspect.failed", e.message, e.exit_code).emit(json),
            }
        }
    }
}

pub(super) fn dispatch_imports(args: ImportsArgs) -> ExitCode {
    match args.command {
        cli::ImportsSub::List(a) => {
            let src = match read_file(&a.path) {
                Ok(s) => s,
                Err(e) => return e.emit(a.json),
            };
            match commands::composition_imports::list_imports(&src, &a.path, a.json) {
                Ok(out) => {
                    println!("{out}");
                    ExitCode::SUCCESS
                }
                Err(e) => CliError::new("imports.failed", e.message, e.exit_code).emit(a.json),
            }
        }
        cli::ImportsSub::Materialize(a) => dispatch_materialize(a),
    }
}

fn dispatch_materialize(a: cli::ImportsMaterializeArgs) -> ExitCode {
    let json = a.json;
    let at = match parse_at_spec(a.at.as_deref()) {
        Ok(pair) => pair,
        Err(msg) => return CliError::usage(msg).emit(json),
    };
    let src = match read_file(&a.path) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };
    let result = match commands::composition_imports::materialize_import(
        &src,
        &a.path,
        &a.target,
        &a.page,
        at,
        a.id.as_deref(),
    ) {
        Ok(result) => result,
        Err(e) => return CliError::new("imports.failed", e.message, e.exit_code).emit(json),
    };
    if a.dry_run {
        if json {
            println!(
                "{}",
                commands::composition_imports::format_materialize_json(&a.path, &result, true)
            );
        } else {
            match String::from_utf8(result.formatted) {
                Ok(s) => print!("{s}"),
                Err(_) => {
                    return CliError::new(
                        "io.not_utf8",
                        "error[io.not_utf8]: formatted output is not valid UTF-8",
                        2,
                    )
                    .emit(json);
                }
            }
        }
        return ExitCode::SUCCESS;
    }
    if let Err(e) = apply_edit(&a.path, &result.formatted, "imports.materialize") {
        return e.emit(json);
    }
    if json {
        println!(
            "{}",
            commands::composition_imports::format_materialize_json(&a.path, &result, false)
        );
    } else {
        println!("{}", result.summary);
    }
    ExitCode::SUCCESS
}

pub(super) fn dispatch_perceive(args: PerceiveArgs) -> ExitCode {
    match args.command {
        cli::PerceiveSub::Vector { path, nodes } => {
            let src = match read_file(&path) {
                Ok(s) => s,
                Err(e) => return e.emit(args.json),
            };
            match commands::perceive::vector(&src, args.json, &nodes) {
                Ok(outcome) => {
                    println!("{}", outcome.stdout);
                    ExitCode::from(outcome.exit_code)
                }
                Err(err) => {
                    CliError::new("perceive.failed", err.message, err.exit_code).emit(args.json)
                }
            }
        }
    }
}
