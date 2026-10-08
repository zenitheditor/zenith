//! MCP merge arguments, committed-output reports, and manifest writes.

use std::path::Path;

use serde_json::{Value, json};

use super::{doc_ref, opt_str, read, req_str};
use crate::commands::{merge, render};
use crate::json_types::{DiagnosticJson, MergeManifest};
use crate::mcp::protocol::ToolResult;
use crate::mcp::serialize::compact;

pub(super) fn call(args: &Value) -> ToolResult {
    match run(args) {
        Ok((value, is_error)) => ToolResult {
            text: compact(&value),
            structured: Some(value),
            is_error,
        },
        Err(message) => ToolResult::err(message),
    }
}

fn run(args: &Value) -> Result<(Value, bool), String> {
    let options = options(args)?;
    let doc = req_str(args, "doc")?;
    let data = req_str(args, "data")?;
    let out_dir = Path::new(req_str(args, "out_dir")?);
    let name_by = opt_str(args, "name_by");
    let manifest_path = opt_str(args, "manifest").map(Path::new);
    let location = doc_ref::locate(doc)?;
    let source = read(&location.path)?;
    let csv = read(Path::new(data))?;
    let mut reserved = vec![location.path.as_path(), Path::new(data)];
    reserved.extend(manifest_path);
    let report = merge::run_with_output_constraints(
        &source,
        &csv,
        location.path.parent(),
        out_dir,
        name_by,
        options,
        &reserved,
    )
    .map_err(|error| error.message)?;
    let output = merge::to_json_output(&report);
    let failures: Vec<_> = report
        .rows
        .iter()
        .filter_map(|row| {
            row.failure
                .as_ref()
                .map(|error| json!({"row": row.row + 1, "error": error}))
        })
        .collect();
    let outputs: Vec<_> = report
        .rows
        .iter()
        .flat_map(|row| row.outputs.iter())
        .map(|filename| out_dir.join(filename).display().to_string())
        .collect();
    let mut diagnostics = output.diagnostics;
    if let Some(path) = manifest_path {
        let manifest = merge::build_manifest(&source, &csv, name_by, &report);
        let mut protected = vec![location.path.clone(), Path::new(data).to_path_buf()];
        protected.extend(
            report
                .rows
                .iter()
                .flat_map(|row| row.outputs.iter())
                .map(|name| out_dir.join(name)),
        );
        if let Err(error) = write_manifest(path, &manifest, &protected) {
            diagnostics.push(DiagnosticJson::error("io.write_failed", format!(
                "cannot write manifest '{}': {error}. Check the directory exists and is writable", path.display()
            )));
        }
    }
    let is_error = output.failed != 0 || !diagnostics.is_empty();
    let mut value = json!({
        "total_rows": output.total_rows,
        "written": output.written,
        "failed": output.failed,
        "failures": failures,
        "format": options.format.extension(),
        "files_written": outputs.len(),
        "outputs": outputs,
        "rows": output.rows,
    });
    if let Some(object) = value.as_object_mut() {
        if !diagnostics.is_empty() {
            object.insert("diagnostics".to_owned(), json!(diagnostics));
        }
        if options.format == render::BatchFormat::Svg && options.raster_scale != 1.0 {
            object.insert("raster_scale".to_owned(), json!(options.raster_scale));
        }
    }
    Ok((value, is_error))
}

fn options(args: &Value) -> Result<render::BatchExportOptions, String> {
    let format = match args.get("format") {
        None => render::BatchFormat::Png,
        Some(Value::String(value)) if value == "png" => render::BatchFormat::Png,
        Some(Value::String(value)) if value == "svg" => render::BatchFormat::Svg,
        Some(value) => {
            return Err(format!(
                "error[cli.invalid_argument]: invalid format {value}. Pass png or svg"
            ));
        }
    };
    let raster_scale = match args.get("raster_scale") {
        None => 1.0,
        Some(value) => {
            if format != render::BatchFormat::Svg {
                return Err("error[cli.invalid_argument]: raster_scale requires format 'svg'. Set format to svg".to_owned());
            }
            render::check_render_scale(value.as_f64().unwrap_or(f64::NAN), &value.to_string())
                .map_err(|message| {
                    format!("error[cli.invalid_argument]: raster_scale: {message}")
                })?
        }
    };
    Ok(render::BatchExportOptions {
        format,
        raster_scale,
    })
}

fn write_manifest(
    path: &Path,
    manifest: &MergeManifest,
    protected: &[std::path::PathBuf],
) -> std::io::Result<()> {
    let references: Vec<_> = protected.iter().map(|path| path.as_path()).collect();
    let mut guard = crate::output_file::OutputGuard::new(&references)?;
    guard.check(path)?;
    let bytes = serde_json::to_vec_pretty(manifest).map_err(std::io::Error::other)?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    guard.write(path, &bytes)
}
