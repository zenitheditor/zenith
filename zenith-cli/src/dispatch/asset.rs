//! Dispatch for `zenith asset import` and `zenith asset zpx-bake`.

use std::path::Path;
use std::process::ExitCode;

use crate::cli::{self, AssetArgs};
use crate::cli_helpers::read_file;
use crate::commands;
use crate::report::CliError;

use super::output::{apply_edit, create_dir_error, print_outcome, write_error};

pub(super) fn dispatch_asset(args: AssetArgs) -> ExitCode {
    match args.command {
        cli::AssetSub::Import(a) => dispatch_asset_import(a),
        cli::AssetSub::ZpxBake(a) => dispatch_asset_zpx_bake(a),
    }
}

fn dispatch_asset_import(a: cli::AssetImportArgs) -> ExitCode {
    let json = a.json;
    let doc_src = match read_file(&a.into) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };
    let input_bytes = match std::fs::read(&a.input) {
        Ok(bytes) => bytes,
        Err(e) => {
            return CliError::new(
                "io.read_failed",
                format!(
                    "error[io.read_failed]: cannot read '{}': {e}; check the path exists and is readable",
                    a.input.display()
                ),
                2,
            )
            .emit(json);
        }
    };
    let source_label = a.input.display().to_string();
    let outcome = match commands::asset::import_run(
        &doc_src,
        &input_bytes,
        commands::asset::AssetImportInput {
            id: &a.id,
            src: &a.src,
            kind: &a.kind,
            source_label: &source_label,
        },
    ) {
        Ok(o) => o,
        Err(e) => return CliError::new("asset.import_failed", e.message, e.exit_code).emit(json),
    };
    finish(&a.into, &a.src, "asset.import", a.apply, json, &outcome)
}

fn dispatch_asset_zpx_bake(a: cli::AssetZpxBakeArgs) -> ExitCode {
    let json = a.json;
    let doc_src = match read_file(&a.into) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };
    let manifest_src = match read_file(&a.manifest) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };
    let source_label = a.manifest.display().to_string();
    let outcome = match commands::asset::zpx_bake_run(
        &doc_src,
        &manifest_src,
        commands::asset::AssetImportInput {
            id: &a.id,
            src: &a.src,
            kind: "image",
            source_label: &source_label,
        },
    ) {
        Ok(o) => o,
        Err(e) => return CliError::new("asset.zpx_bake_failed", e.message, e.exit_code).emit(json),
    };
    finish(&a.into, &a.src, "asset.zpx_bake", a.apply, json, &outcome)
}

/// Apply the outcome when asked and not rejected, then print the result.
fn finish(
    doc_path: &Path,
    src: &str,
    history_label: &str,
    apply: bool,
    json: bool,
    outcome: &commands::asset::AssetImportOutcome,
) -> ExitCode {
    if apply
        && outcome.exit_code != 1
        && let Err(e) = apply_asset_outcome(doc_path, src, history_label, outcome)
    {
        return e.emit(json);
    }
    print_outcome(json, &outcome.json_str, &outcome.human);
    ExitCode::from(outcome.exit_code)
}

fn apply_asset_outcome(
    doc_path: &Path,
    src: &str,
    history_label: &str,
    outcome: &commands::asset::AssetImportOutcome,
) -> Result<(), CliError> {
    let doc_parent = doc_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let asset_path = doc_parent.join(src);
    match std::fs::read(&asset_path) {
        Ok(existing) => {
            if existing.as_slice() != outcome.produced.bytes.as_ref() {
                return Err(CliError::new(
                    "asset.exists",
                    format!(
                        "error[asset.exists]: destination '{}' already exists with different bytes; \
                         pick another --src or remove the file",
                        asset_path.display()
                    ),
                    2,
                ));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(CliError::new(
                "io.read_failed",
                format!(
                    "error[io.read_failed]: cannot read '{}': {e}; check the path is readable",
                    asset_path.display()
                ),
                2,
            ));
        }
    }
    if let Some(parent) = asset_path.parent()
        && !parent.as_os_str().is_empty()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        return Err(create_dir_error(parent, &e));
    }
    std::fs::write(&asset_path, outcome.produced.bytes.as_ref())
        .map_err(|e| write_error(&asset_path, &e))?;
    apply_edit(
        doc_path,
        outcome.result.source_after.as_bytes(),
        history_label,
    )
}
