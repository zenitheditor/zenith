//! Output staging, diagnostic gates, and render reports.
use super::run::{RenderRun, Stop};
use crate::cli_helpers::{print_diagnostics_stderr, write_bytes};
use crate::commands::serialize_pretty;
use crate::json_types::{DiagnosticJson, RenderOutput};
use crate::json_types::{RenderImageJson, RenderRasterizedRegionJson};
use crate::report::ImportFiles;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::ExitCode;
use zenith_core::Diagnostic;
const RENDER_SCHEMA: &str = "zenith-render-v1";
impl RenderRun<'_> {
    /// Gate diagnostics and stage output bytes.
    pub(super) fn write(
        &mut self,
        out: &Path,
        bytes: Vec<u8>,
        diagnostics: Vec<Diagnostic>,
        import_files: &ImportFiles,
        label: &str,
    ) -> Result<(), Stop> {
        gate(&diagnostics, import_files)?;
        self.pending.push((out.to_path_buf(), bytes));
        if !self.args.json {
            self.messages
                .push(format!("{label} written to '{}'", out.display()));
        }
        self.diagnostics.extend(diagnostics);
        self.import_files.extend(import_files);
        Ok(())
    }

    pub(super) fn finish_ok(self) -> ExitCode {
        let diagnostics = Diagnostic::dedup(self.diagnostics);
        if self.args.json {
            print_envelope(
                "ok",
                self.outputs,
                (self.images, self.rasterized_regions),
                &diagnostics,
                self.src,
                &self.import_files,
            );
        } else {
            print_diagnostics_stderr(&diagnostics, self.src, &self.import_files);
        }
        ExitCode::SUCCESS
    }

    pub(super) fn finish_blocked(mut self, stop: Stop) -> ExitCode {
        let written: BTreeSet<_> = self.outputs.iter().map(String::as_str).collect();
        self.images
            .retain(|image| written.contains(image.path.as_str()));
        self.rasterized_regions
            .retain(|region| written.contains(region.path.as_str()));
        self.diagnostics.extend(stop.diagnostics);
        self.import_files.extend(&stop.import_files);
        let diagnostics = Diagnostic::dedup(self.diagnostics);
        if self.args.json {
            print_envelope(
                "blocked",
                self.outputs,
                (self.images, self.rasterized_regions),
                &diagnostics,
                self.src,
                &self.import_files,
            );
        } else {
            for path in &self.outputs {
                println!("written: {path}");
            }
            print_diagnostics_stderr(&diagnostics, self.src, &self.import_files);
            eprintln!(
                "render blocked by {} hard diagnostic(s)",
                diagnostics.iter().filter(|d| d.is_error()).count()
            );
        }
        ExitCode::from(stop.exit_code)
    }
    pub(super) fn flush(&mut self) -> Result<(), Stop> {
        gate(&self.diagnostics, &self.import_files)?;
        for dir in &self.directories {
            if let Err(e) = std::fs::create_dir_all(dir) {
                return Err(write_stop(dir, &e));
            }
        }
        for (path, bytes) in self.pending.drain(..) {
            if let Err(e) = write_bytes(&path, &bytes) {
                return Err(write_stop(&path, &e));
            }
            self.outputs.push(path.display().to_string());
        }
        for message in &self.messages {
            println!("{message}");
        }
        Ok(())
    }
}
/// Stop with exit code 2 when any diagnostic is an error.
pub(super) fn gate(diagnostics: &[Diagnostic], import_files: &ImportFiles) -> Result<(), Stop> {
    if Diagnostic::has_errors(diagnostics) {
        return Err(Stop {
            diagnostics: diagnostics.to_vec(),
            exit_code: 2,
            import_files: import_files.clone(),
        });
    }
    Ok(())
}

fn write_stop(path: &Path, e: &std::io::Error) -> Stop {
    Stop {
        diagnostics: vec![Diagnostic::error(
            "io.write_failed",
            format!(
                "cannot write '{}': {e}; check the directory exists and is writable",
                path.display()
            ),
            None,
            None,
        )],
        exit_code: 2,
        import_files: ImportFiles::default(),
    }
}

fn print_envelope(
    status: &'static str,
    outputs: Vec<String>,
    metadata: (Vec<RenderImageJson>, Vec<RenderRasterizedRegionJson>),
    diagnostics: &[Diagnostic],
    src: &str,
    import_files: &ImportFiles,
) {
    let out = RenderOutput {
        schema: RENDER_SCHEMA,
        status,
        outputs,
        images: metadata.0,
        rasterized_regions: metadata.1,
        diagnostics: DiagnosticJson::located_all_in(diagnostics, src, import_files),
    };
    println!("{}", serialize_pretty(&out));
}
