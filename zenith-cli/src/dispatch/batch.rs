//! Dispatch for the batch generators: `merge` and `variant`.

use std::path::Path;
use std::process::ExitCode;

use crate::cli::{MergeArgs, VariantArgs};
use crate::cli_helpers::read_file;
use crate::commands;
use crate::commands::serialize_pretty;
use crate::report::CliError;

use super::output::{create_dir_error, write_error};

pub(super) fn dispatch_merge(args: MergeArgs) -> ExitCode {
    let json = args.json;
    let doc_src = match read_file(&args.doc) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };
    let csv_src = match read_file(&args.data) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };
    let report = match commands::merge::run_with_format(
        &doc_src,
        &csv_src,
        args.doc.parent(),
        &args.out_dir,
        args.name_by.as_deref(),
        args.format,
    ) {
        Ok(report) => report,
        Err(e) => return CliError::new("merge.setup_failed", e.message, e.exit_code).emit(json),
    };
    // The manifest goes first so a write error is the only output.
    if let Some(manifest_path) = &args.manifest {
        let manifest =
            commands::merge::build_manifest(&doc_src, &csv_src, args.name_by.as_deref(), &report);
        if let Err(e) = write_manifest(manifest_path, &serialize_pretty(&manifest)) {
            return e.emit(json);
        }
    }
    if json {
        println!(
            "{}",
            serialize_pretty(&commands::merge::to_json_output(&report))
        );
    } else {
        let n_written = report.rows.iter().filter(|r| r.failure.is_none()).count();
        println!(
            "wrote {} file(s) to '{}'",
            n_written,
            args.out_dir.display()
        );
        for r in &report.rows {
            for diagnostic in &r.diagnostics {
                eprintln!(
                    "row {}: {}",
                    r.row + 1,
                    commands::format_diagnostic_line(diagnostic)
                );
            }
        }
        for r in report.failed() {
            eprintln!("row {}: {}", r.row + 1, r.failure.as_deref().unwrap_or(""));
        }
    }
    if report.rows.iter().any(|r| r.failure.is_some()) {
        ExitCode::from(1u8)
    } else {
        ExitCode::SUCCESS
    }
}

pub(super) fn dispatch_variant(args: VariantArgs) -> ExitCode {
    let json = args.json;
    let doc_src = match read_file(&args.doc) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };
    // The output stem is the input file name without its extension.
    let stem = args
        .doc
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("doc");
    let report = match commands::variant::run_variant_with_format(
        &doc_src,
        args.doc.parent(),
        &args.out_dir,
        stem,
        args.format,
    ) {
        Ok(report) => report,
        Err(e) => {
            return CliError::new("variant.setup_failed", e.message, e.exit_code).emit(json);
        }
    };
    // The manifest goes first so a write error is the only output.
    if let Some(manifest_path) = &args.manifest {
        let manifest = commands::variant::build_manifest(&doc_src, &report);
        if let Err(e) = write_manifest(manifest_path, &serialize_pretty(&manifest)) {
            return e.emit(json);
        }
    }
    let failed = report.failed();
    if json {
        println!(
            "{}",
            serialize_pretty(&commands::variant::to_json_output(&report))
        );
    } else {
        println!(
            "generated {} variant(s) to '{}'",
            report.generated(),
            args.out_dir.display()
        );
        for r in &report.variants {
            for diagnostic in &r.diagnostics {
                eprintln!(
                    "variant {}: {}",
                    r.id,
                    commands::format_diagnostic_line(diagnostic)
                );
            }
        }
        for r in &failed {
            eprintln!("variant {}: {}", r.id, r.failure.as_deref().unwrap_or(""));
        }
    }
    if failed.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1u8)
    }
}

/// Write `manifest_json` to `path`, creating its parent directory.
fn write_manifest(path: &Path, manifest_json: &str) -> Result<(), CliError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return Err(create_dir_error(parent, &e));
    }
    std::fs::write(path, manifest_json.as_bytes()).map_err(|e| write_error(path, &e))
}
