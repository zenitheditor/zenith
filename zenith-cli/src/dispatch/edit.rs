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

/// Inline ops are either the `{"ops":[...]}` object or a bare array of ops.
fn inline_tx(text: &str) -> String {
    if text.trim_start().starts_with('[') {
        format!("{{\"ops\":{text}}}")
    } else {
        text.to_owned()
    }
}

pub(super) fn dispatch_tx(args: TxArgs) -> ExitCode {
    let json = args.json;
    let doc_src = match read_file(&args.path) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };
    let tx_json = match (&args.ops, &args.tx_file) {
        (Some(inline), _) => inline_tx(inline),
        (None, Some(file)) => match read_file(file) {
            Ok(s) => s,
            Err(e) => return e.emit(json),
        },
        // clap requires one of the two.
        (None, None) => String::new(),
    };
    let label = args.path.display().to_string();
    // Refuse a `--png` target that is the document or another output before
    // the edit is written.
    let mut png_guard = match png_target(&args) {
        Ok(guard) => guard,
        Err(e) => return e.emit(json),
    };
    let ctx = commands::tx::TxCtx {
        project_dir: args.path.parent(),
        label: &label,
        show_diff: !args.apply || args.diff,
        apply: args.apply,
    };
    let mut outcome = match commands::tx::run_with(&doc_src, &tx_json, &ctx) {
        Ok(o) => o,
        Err(e) => return CliError::new("tx.failed", e.message, e.exit_code).emit(json),
    };
    // Render before anything is written: a page that does not render leaves
    // both files as they were.
    let mut png = None;
    if let (Some(out), true) = (&args.png, outcome.exit_code != 1) {
        match render_png(
            &outcome.written.text,
            args.path.parent(),
            args.page.unwrap_or(1),
            out,
        ) {
            Ok(rendered) => png = Some(rendered),
            Err(message) => {
                outcome.attach_png_error(message);
                print_outcome(json, &outcome.json_str, &outcome.human);
                return ExitCode::from(2);
            }
        }
    }
    if args.apply
        && outcome.exit_code != 1
        && let Err(e) = apply_edit(&args.path, outcome.written.text.as_bytes(), "tx.apply")
    {
        return e.emit(json);
    }
    let mut exit_code = outcome.exit_code;
    if let (Some(rendered), Some(guard)) = (png, png_guard.as_mut()) {
        match write_png(guard, rendered) {
            Ok(note) => outcome.attach_png(note),
            Err(message) => {
                outcome.attach_png_error(message);
                exit_code = 2;
            }
        }
    }
    print_outcome(json, &outcome.json_str, &outcome.human);
    ExitCode::from(exit_code)
}

/// The output guard for `--png`: the document and the tx file are protected,
/// and the target must be replaceable. `None` without `--png`.
fn png_target(args: &TxArgs) -> Result<Option<crate::output_file::OutputGuard>, CliError> {
    let Some(out) = &args.png else {
        return Ok(None);
    };
    let mut protected = vec![args.path.as_path()];
    protected.extend(args.tx_file.as_deref());
    let mut guard = crate::output_file::OutputGuard::new(&protected)
        .map_err(|e| write_error(&args.path, &e))?;
    guard.check(out).map_err(|e| write_error(out, &e))?;
    crate::output_file::check_replaceable(out).map_err(|e| write_error(out, &e))?;
    Ok(Some(guard))
}

/// A rendered page and where it goes.
struct RenderedPng {
    bytes: Vec<u8>,
    out: std::path::PathBuf,
    width: u32,
    height: u32,
    page: usize,
    pages: usize,
}

/// Render page `page` of the edited source `src`. The error is the message
/// for the result: nothing was written.
fn render_png(
    src: &str,
    project_dir: Option<&std::path::Path>,
    page: usize,
    out: &std::path::Path,
) -> Result<RenderedPng, String> {
    let artifact = commands::render::to_png_with_dir(
        src,
        project_dir,
        page,
        false,
        &zenith_pipeline::PolicyFlags::default(),
        None,
    )
    .map_err(|e| {
        format!(
            "error[tx.png_failed]: page {page} did not render, so neither the edit nor the PNG was written: {}",
            e.message
        )
    })?;
    Ok(RenderedPng {
        bytes: artifact.png,
        out: out.to_path_buf(),
        width: artifact.width,
        height: artifact.height,
        page,
        pages: artifact.page_count,
    })
}

/// Write the rendered page. The edit is already on disk when this runs.
fn write_png(
    guard: &mut crate::output_file::OutputGuard,
    rendered: RenderedPng,
) -> Result<crate::json_types::TxPngJson, String> {
    let out = &rendered.out;
    let failed = |e: &std::io::Error| {
        format!(
            "error[tx.png_failed]: the edit is written but the PNG was not: {}",
            crate::output_file::write_failure(out, e)
        )
    };
    guard.write(out, &rendered.bytes).map_err(|e| failed(&e))?;
    Ok(crate::json_types::TxPngJson {
        path: out.display().to_string(),
        width: rendered.width,
        height: rendered.height,
        page: rendered.page,
        pages: rendered.pages,
    })
}

pub(super) fn dispatch_outline_text(args: OutlineTextArgs) -> ExitCode {
    let json = args.json;
    let doc_src = match read_file(&args.path) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };
    let label = args.path.display().to_string();
    let ctx = commands::tx::TxCtx {
        project_dir: args.path.parent(),
        label: &label,
        show_diff: !args.apply || args.diff,
        apply: args.apply,
    };
    let outcome = match commands::tx::run_outline_text(
        &doc_src,
        &ctx,
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
            outcome.written.text.as_bytes(),
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
        && let Err(e) = apply_edit(&a.doc, outcome.written.text.as_bytes(), "theme.apply")
    {
        return e.emit(json);
    }
    print_outcome(json, &outcome.json_str, &outcome.human);
    ExitCode::from(outcome.exit_code)
}
