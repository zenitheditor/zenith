//! Dispatch for `zenith import` (`svg`, …).

use std::process::ExitCode;

use serde::Serialize;
use zenith_core::{KdlAdapter, KdlSource};
use zenith_producers::{SvgImportOptions, svg_to_document};

use crate::cli::{ImportArgs, ImportSub, ImportSvgArgs};
use crate::commands::serialize_pretty;
use crate::report::CliError;

use super::output::{create_dir_error, write_error};

#[derive(Serialize)]
struct ImportSvgJson {
    schema: &'static str,
    input: String,
    out: String,
    width: f64,
    height: f64,
    paths_imported: usize,
    paths_skipped: usize,
    unsupported_nodes: usize,
    color_tokens: usize,
    stroke_tokens: usize,
}

pub(super) fn dispatch_import(args: ImportArgs) -> ExitCode {
    match args.command {
        ImportSub::Svg(svg_args) => dispatch_import_svg(svg_args),
    }
}

fn dispatch_import_svg(args: ImportSvgArgs) -> ExitCode {
    let json = args.json;
    if args.out.exists() && !args.force {
        return CliError::new(
            "import.output_exists",
            format!(
                "error[import.output_exists]: '{}' already exists; pass --force to overwrite",
                args.out.display()
            ),
            2,
        )
        .emit(json);
    }

    let bytes = match std::fs::read(&args.input) {
        Ok(b) => b,
        Err(e) => {
            return CliError::new(
                "import.read_failed",
                format!(
                    "error[import.read_failed]: could not read '{}': {e}",
                    args.input.display()
                ),
                2,
            )
            .emit(json);
        }
    };

    let stem = args
        .input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("imported");
    let name = args.name.clone().unwrap_or_else(|| stem.to_owned());
    let slug = stem.to_owned();

    let output = match svg_to_document(&bytes, &SvgImportOptions { slug, name }) {
        Ok(out) => out,
        Err(e) => {
            return CliError::new(
                "import.svg_failed",
                format!("error[import.svg_failed]: {e}"),
                2,
            )
            .emit(json);
        }
    };

    let formatted = match KdlAdapter.format(&output.document) {
        Ok(buf) => buf,
        Err(e) => {
            return CliError::new(
                "import.format_failed",
                format!("error[import.format_failed]: failed to format imported document: {e}"),
                2,
            )
            .emit(json);
        }
    };

    let protected = [args.input.as_path()];
    let mut guard = match crate::output_file::OutputGuard::new(&protected) {
        Ok(g) => g,
        Err(e) => return write_error(&args.out, &e).emit(json),
    };
    if let Err(e) = guard.check(&args.out) {
        return write_error(&args.out, &e).emit(json);
    }
    if let Some(parent) = args.out.parent()
        && !parent.as_os_str().is_empty()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return create_dir_error(parent, &e).emit(json);
    }
    if let Err(e) = guard.write(&args.out, &formatted) {
        return write_error(&args.out, &e).emit(json);
    }

    if json {
        let payload = ImportSvgJson {
            schema: "zenith-import-svg-v1",
            input: args.input.display().to_string(),
            out: args.out.display().to_string(),
            width: output.width,
            height: output.height,
            paths_imported: output.paths_imported,
            paths_skipped: output.paths_skipped,
            unsupported_nodes: output.unsupported_nodes,
            color_tokens: output.color_tokens,
            stroke_tokens: output.stroke_tokens,
        };
        println!("{}", serialize_pretty(&payload));
    } else {
        println!(
            "imported '{}' -> '{}' ({}x{} px, {} path(s), {} color token(s), {} stroke token(s))",
            args.input.display(),
            args.out.display(),
            output.width,
            output.height,
            output.paths_imported,
            output.color_tokens,
            output.stroke_tokens,
        );
        if output.unsupported_nodes > 0 {
            eprintln!(
                "note: skipped {} unsupported non-vector SVG node(s) (text or raster image)",
                output.unsupported_nodes
            );
        }
    }

    ExitCode::SUCCESS
}
