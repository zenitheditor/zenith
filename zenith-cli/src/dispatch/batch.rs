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
    let options = match batch_options(args.format, args.raster_scale.as_deref()) {
        Ok(options) => options,
        Err(error) => return error.emit(json),
    };
    let doc_src = match read_file(&args.doc) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };
    let csv_src = match read_file(&args.data) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };
    let mut reserved = vec![args.doc.as_path(), args.data.as_path()];
    reserved.extend(args.manifest.as_deref());
    let report = match commands::merge::run_with_output_constraints(
        &doc_src,
        &csv_src,
        args.doc.parent(),
        &args.out_dir,
        args.name_by.as_deref(),
        options,
        &reserved,
    ) {
        Ok(report) => report,
        Err(e) => return CliError::new("merge.setup_failed", e.message, e.exit_code).emit(json),
    };
    let mut manifest_error = None;
    if let Some(manifest_path) = &args.manifest {
        let manifest =
            commands::merge::build_manifest(&doc_src, &csv_src, args.name_by.as_deref(), &report);
        let mut protected = reserved
            .iter()
            .copied()
            .filter(|path| *path != manifest_path.as_path())
            .map(Path::to_path_buf)
            .collect::<Vec<_>>();
        protected.extend(
            report
                .rows
                .iter()
                .flat_map(|row| row.outputs.iter())
                .map(|name| args.out_dir.join(name)),
        );
        if let Err(e) = write_manifest(manifest_path, &serialize_pretty(&manifest), &protected) {
            manifest_error = Some(e);
        }
    }
    if json {
        println!(
            "{}",
            serialize_pretty(&{
                let mut output = commands::merge::to_json_output(&report);
                if let Some(error) = &manifest_error {
                    output.diagnostics.extend(error.diagnostics());
                }
                output
            })
        );
    } else {
        let n_written = report
            .rows
            .iter()
            .map(|row| row.outputs.len())
            .sum::<usize>();
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
    if let Some(error) = manifest_error {
        if !json {
            eprintln!("{}", error.human);
        }
        return ExitCode::from(error.exit_code);
    }
    if report.rows.iter().any(|r| r.failure.is_some()) {
        ExitCode::from(1u8)
    } else {
        ExitCode::SUCCESS
    }
}

pub(super) fn dispatch_variant(args: VariantArgs) -> ExitCode {
    let json = args.json;
    let options = match batch_options(args.format, args.raster_scale.as_deref()) {
        Ok(options) => options,
        Err(error) => return error.emit(json),
    };
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
    let mut reserved = vec![args.doc.as_path()];
    reserved.extend(args.manifest.as_deref());
    let report = match commands::variant::run_variant_with_output_constraints(
        &doc_src,
        args.doc.parent(),
        &args.out_dir,
        stem,
        options,
        &reserved,
    ) {
        Ok(report) => report,
        Err(e) => {
            return CliError::new("variant.setup_failed", e.message, e.exit_code).emit(json);
        }
    };
    let mut manifest_error = None;
    if let Some(manifest_path) = &args.manifest {
        let manifest = commands::variant::build_manifest(&doc_src, &report);
        let mut protected = vec![args.doc.clone()];
        for outputs in report
            .variants
            .iter()
            .filter_map(|record| record.outputs.as_ref())
        {
            protected.push(args.out_dir.join(&outputs.zen));
            if !outputs.png.is_empty() {
                protected.push(args.out_dir.join(&outputs.png));
            }
            if let Some(svg) = &outputs.svg {
                protected.push(args.out_dir.join(svg));
            }
        }
        if let Err(e) = write_manifest(manifest_path, &serialize_pretty(&manifest), &protected) {
            manifest_error = Some(e);
        }
    }
    let failed = report.failed();
    if json {
        println!(
            "{}",
            serialize_pretty(&{
                let mut output = commands::variant::to_json_output(&report);
                if let Some(error) = &manifest_error {
                    output.diagnostics.extend(error.diagnostics());
                }
                output
            })
        );
    } else {
        let files = report
            .variants
            .iter()
            .filter_map(|record| record.outputs.as_ref())
            .map(|outputs| {
                1 + usize::from(!outputs.png.is_empty()) + usize::from(outputs.svg.is_some())
            })
            .sum::<usize>();
        println!(
            "generated {} variant(s) to '{}'",
            report.generated(),
            args.out_dir.display()
        );
        println!("wrote {files} file(s)");
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
    if let Some(error) = manifest_error {
        if !json {
            eprintln!("{}", error.human);
        }
        return ExitCode::from(error.exit_code);
    }
    if failed.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1u8)
    }
}

/// Write `manifest_json` to `path`, creating its parent directory.
fn write_manifest(
    path: &Path,
    manifest_json: &str,
    protected: &[std::path::PathBuf],
) -> Result<(), CliError> {
    let references: Vec<_> = protected.iter().map(|path| path.as_path()).collect();
    // Recheck against inputs and committed artifacts before creating directories.
    let mut guard = crate::output_file::OutputGuard::new(&references)
        .map_err(|error| write_error(path, &error))?;
    guard
        .check(path)
        .map_err(|error| write_error(path, &error))?;
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return Err(create_dir_error(parent, &e));
    }
    guard
        .write(path, manifest_json.as_bytes())
        .map_err(|e| write_error(path, &e))
}

fn batch_options(
    format: commands::render::BatchFormat,
    raw: Option<&str>,
) -> Result<commands::render::BatchExportOptions, CliError> {
    if raw.is_some() && format != commands::render::BatchFormat::Svg {
        return Err(CliError::usage(
            "error: --raster-scale requires --format svg",
        ));
    }
    let raster_scale = raw
        .map(|raw| commands::render::parse_scale_flag(raw, "--raster-scale"))
        .transpose()
        .map_err(CliError::usage)?
        .unwrap_or(1.0);
    Ok(commands::render::BatchExportOptions {
        format,
        raster_scale,
    })
}
