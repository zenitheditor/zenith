//! Dispatch for the editing commands: `tx`, `outline-text`, and `theme`.
//!
//! With `--apply`, the edit reaches disk before the result prints, so a write
//! error is the only output.

use std::process::ExitCode;

use crate::cli::{self, OutlineTextArgs, ThemeArgs, TxArgs};
use crate::cli_helpers::read_file;
use crate::commands;
use crate::report::CliError;

use super::output::{apply_edit, print_outcome, write_error};

pub(super) fn dispatch_tx(args: TxArgs) -> ExitCode {
    let json = args.json;
    let doc_src = match read_file(&args.path) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };
    let tx_json = match read_file(&args.tx_file) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };
    let outcome = match commands::tx::run(&doc_src, &tx_json) {
        Ok(o) => o,
        Err(e) => return CliError::new("tx.failed", e.message, e.exit_code).emit(json),
    };
    if args.apply
        && outcome.exit_code != 1
        && let Err(e) = apply_edit(
            &args.path,
            outcome.result.source_after.as_bytes(),
            "tx.apply",
        )
    {
        return e.emit(json);
    }
    print_outcome(json, &outcome.json_str, &outcome.human);
    ExitCode::from(outcome.exit_code)
}

pub(super) fn dispatch_outline_text(args: OutlineTextArgs) -> ExitCode {
    let json = args.json;
    let doc_src = match read_file(&args.path) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };
    let outcome = match commands::tx::run_outline_text(
        &doc_src,
        args.path.parent(),
        &args.node,
        &args.id_prefix,
        args.locked,
    ) {
        Ok(o) => o,
        Err(e) => return CliError::new("text_outline.failed", e.message, e.exit_code).emit(json),
    };
    if args.apply
        && outcome.exit_code != 1
        && let Err(e) = apply_edit(
            &args.path,
            outcome.result.source_after.as_bytes(),
            "text_outline.apply",
        )
    {
        return e.emit(json);
    }
    print_outcome(json, &outcome.json_str, &outcome.human);
    ExitCode::from(outcome.exit_code)
}

pub(super) fn dispatch_theme(args: ThemeArgs) -> ExitCode {
    match args.command {
        cli::ThemeSub::New(a) => dispatch_theme_new(*a),
        cli::ThemeSub::Apply(a) => dispatch_theme_apply(a),
    }
}

fn dispatch_theme_new(a: cli::ThemeNewArgs) -> ExitCode {
    let scheme = match a.scheme.as_str() {
        "light" => zenith_core::theme::Scheme::Light,
        "dark" => zenith_core::theme::Scheme::Dark,
        other => {
            return CliError::usage(format!(
                "error: --scheme must be 'light' or 'dark', got '{other}'"
            ))
            .emit(false);
        }
    };
    let input = commands::theme::ThemeInput {
        name: &a.name,
        scheme,
        primary: &a.primary,
        secondary: a.secondary.as_deref(),
        accent: a.accent.as_deref(),
        neutral: a.neutral.as_deref(),
        info: a.info.as_deref(),
        success: a.success.as_deref(),
        warning: a.warning.as_deref(),
        error: a.error.as_deref(),
        shape: commands::theme::Shape {
            radius_box: a.radius_box,
            radius_field: a.radius_field,
            radius_selector: a.radius_selector,
            border: a.border,
            depth: a.depth,
            noise: a.noise,
        },
    };
    match commands::theme::new(&input) {
        Ok(source) => {
            if let Some(path) = &a.out {
                if let Err(e) = std::fs::write(path, &source) {
                    return write_error(path, &e).emit(false);
                }
                println!("wrote {}", path.display());
            } else {
                print!("{source}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => CliError::new(
            "theme.new_failed",
            format!("error: {}", e.message),
            e.exit_code,
        )
        .emit(false),
    }
}

fn dispatch_theme_apply(a: cli::ThemeApplyArgs) -> ExitCode {
    let json = a.json;
    let doc_src = match read_file(&a.doc) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };
    let outcome = match commands::theme::apply_run(a.doc.parent(), &a.pack, &doc_src) {
        Ok(o) => o,
        Err(e) => return CliError::new("theme.apply_failed", e.message, e.exit_code).emit(json),
    };
    if a.apply
        && outcome.exit_code != 1
        && let Err(e) = apply_edit(
            &a.doc,
            outcome.result.source_after.as_bytes(),
            "theme.apply",
        )
    {
        return e.emit(json);
    }
    print_outcome(json, &outcome.json_str, &outcome.human);
    ExitCode::from(outcome.exit_code)
}
