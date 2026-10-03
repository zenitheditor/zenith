//! Dispatch logic for `zenith render`.
//!
//! Every requested output (spread, scene, PNG, PDF, all pages) renders in
//! turn. Diagnostics from all of them merge, repeats removed, and print once:
//! one `zenith-render-v1` envelope on stdout under `--json`, else grouped
//! lines on stderr. The first output that fails stops the run with status
//! `blocked` and every diagnostic known so far.

use std::path::Path;
use std::process::ExitCode;

use zenith_core::{DataContext, Diagnostic};

use crate::cli::RenderArgs;
use crate::cli_helpers::{parse_spread_spec, print_diagnostics_stderr, read_file, write_bytes};
use crate::commands;
use crate::commands::render::{RenderCmdErr, RenderEntryOptions, SpreadRenderOpts};
use crate::commands::serialize_pretty;
use crate::config::CliPolicyFlags;
use crate::json_types::{DiagnosticJson, RenderOutput};
use crate::report::{CliError, ImportFiles};

const RENDER_SCHEMA: &str = "zenith-render-v1";

pub(super) fn dispatch_render(args: RenderArgs) -> ExitCode {
    let json = args.json;
    if args.scene.is_none() && args.png.is_none() && args.pdf.is_none() && args.all_pages.is_none()
    {
        return CliError::usage(
            "error: at least one of --scene <OUT>, --png <OUT>, --pdf <OUT>, or --all-pages <DIR> is required",
        )
        .emit(json);
    }
    if args.spread.is_some() && args.png.is_none() {
        return CliError::usage("error: --spread requires --png <OUT>").emit(json);
    }
    if args.spread.is_some() && args.pdf.is_some() {
        return CliError::usage("error: --spread cannot be combined with --pdf (not supported)")
            .emit(json);
    }
    let spread = match args.spread.as_deref().map(parse_spread_spec).transpose() {
        Ok(pair) => pair,
        Err(msg) => return CliError::usage(msg).emit(json),
    };
    let src = match read_file(&args.path) {
        Ok(s) => s,
        Err(e) => return e.emit(json),
    };
    // Load the data context once so `(data)"field"` refs resolve on every
    // output.
    let data_ctx: Option<DataContext> = match &args.data {
        Some(data_path) => match commands::render::load_data_context(data_path) {
            Ok(ctx) => Some(ctx),
            Err(e) => {
                return CliError::new(
                    "data.load_failed",
                    format!(
                        "error[data.load_failed]: {e}; check '{}' is valid JSON or CSV",
                        data_path.display()
                    ),
                    2,
                )
                .emit(json);
            }
        },
        None => None,
    };
    let flags = CliPolicyFlags {
        allow: args.allow.clone(),
        warn: args.warn.clone(),
        deny: args.deny.clone(),
    };
    let mut run = RenderRun {
        args: &args,
        src: &src,
        flags: &flags,
        data: data_ctx.as_ref(),
        outputs: Vec::new(),
        diagnostics: Vec::new(),
        import_files: ImportFiles::default(),
    };
    match run.render_all(spread) {
        Ok(()) => run.finish_ok(),
        Err(stop) => run.finish_blocked(stop),
    }
}

/// Why a render run stopped.
struct Stop {
    /// Diagnostics of the failing output. At least one is an error.
    diagnostics: Vec<Diagnostic>,
    exit_code: u8,
    /// Files of the composition imports behind the diagnostic spans.
    import_files: ImportFiles,
}

impl From<RenderCmdErr> for Stop {
    fn from(e: RenderCmdErr) -> Self {
        Self {
            diagnostics: e.diagnostics,
            exit_code: e.exit_code,
            import_files: e.import_files,
        }
    }
}

/// State of one `zenith render` invocation.
struct RenderRun<'a> {
    args: &'a RenderArgs,
    src: &'a str,
    flags: &'a CliPolicyFlags,
    data: Option<&'a DataContext>,
    /// Written paths, in write order.
    outputs: Vec<String>,
    /// Diagnostics of every finished output, in output order.
    diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports behind the diagnostic spans.
    import_files: ImportFiles,
}

impl RenderRun<'_> {
    fn entry_options(&self) -> RenderEntryOptions<'_> {
        RenderEntryOptions {
            locked: self.args.locked,
            subset: !self.args.embed_full_fonts,
            flags: self.flags,
            data: self.data,
            construction_overlay: self.args.construction_overlay,
        }
    }

    fn render_all(&mut self, spread: Option<(usize, usize)>) -> Result<(), Stop> {
        let args = self.args;
        let dir = args.path.parent();
        if let (Some((page_a, page_b)), Some(png_out)) = (spread, &args.png) {
            let artifact = commands::render::to_png_spread(
                self.src,
                dir,
                page_a,
                page_b,
                args.gutter,
                SpreadRenderOpts {
                    locked: args.locked,
                    flags: self.flags,
                    data: self.data,
                    construction_overlay: args.construction_overlay,
                },
            )?;
            self.write(
                png_out,
                &artifact.png,
                artifact.diagnostics,
                &artifact.import_files,
                "spread PNG",
            )?;
        }
        if let Some(scene_out) = &args.scene {
            let artifact = commands::render::to_scene_json_with_options(
                self.src,
                dir,
                args.page.unwrap_or(1),
                self.entry_options(),
            )?;
            self.write(
                scene_out,
                artifact.json.as_bytes(),
                artifact.diagnostics,
                &artifact.import_files,
                "scene",
            )?;
        }
        // --spread already wrote the composited image to the --png target.
        if let (Some(png_out), None) = (&args.png, spread) {
            let artifact = commands::render::to_png_with_dir_options(
                self.src,
                dir,
                args.page.unwrap_or(1),
                self.entry_options(),
            )?;
            self.write(
                png_out,
                &artifact.png,
                artifact.diagnostics,
                &artifact.import_files,
                "PNG",
            )?;
        }
        if let Some(pdf_out) = &args.pdf {
            // `--page N` selects one page. Without it every page goes into one
            // multi-page PDF.
            let artifact = match args.page {
                Some(n) => commands::render::to_pdf_with_dir_options(
                    self.src,
                    dir,
                    n,
                    self.entry_options(),
                ),
                None => commands::render::to_pdf_all_pages_with_dir_options(
                    self.src,
                    dir,
                    self.entry_options(),
                ),
            }?;
            self.write(
                pdf_out,
                &artifact.pdf,
                artifact.diagnostics,
                &artifact.import_files,
                "PDF",
            )?;
        }
        if let Some(out_dir) = &args.all_pages {
            self.render_pages(out_dir)?;
        }
        Ok(())
    }

    fn render_pages(&mut self, out_dir: &Path) -> Result<(), Stop> {
        if let Err(e) = std::fs::create_dir_all(out_dir) {
            return Err(write_stop(out_dir, &e));
        }
        let artifact = commands::render::to_png_all_pages_options(
            self.src,
            self.args.path.parent(),
            self.entry_options(),
        )?;
        // Block on hard diagnostics before any page reaches disk.
        gate(&artifact.diagnostics, &artifact.import_files)?;
        for (i, png) in artifact.pages.iter().enumerate() {
            let page_path = out_dir.join(format!("page-{}.png", i + 1));
            if let Err(e) = write_bytes(&page_path, png) {
                return Err(write_stop(&page_path, &e));
            }
            self.outputs.push(page_path.display().to_string());
        }
        if !self.args.json {
            println!(
                "{} page(s) written to '{}'",
                artifact.pages.len(),
                out_dir.display()
            );
        }
        self.diagnostics.extend(artifact.diagnostics);
        self.import_files.extend(&artifact.import_files);
        Ok(())
    }

    /// Gate on `diagnostics`, write `bytes` to `out`, and record the output.
    fn write(
        &mut self,
        out: &Path,
        bytes: &[u8],
        diagnostics: Vec<Diagnostic>,
        import_files: &ImportFiles,
        label: &str,
    ) -> Result<(), Stop> {
        gate(&diagnostics, import_files)?;
        if let Err(e) = write_bytes(out, bytes) {
            return Err(write_stop(out, &e));
        }
        self.outputs.push(out.display().to_string());
        if !self.args.json {
            println!("{label} written to '{}'", out.display());
        }
        self.diagnostics.extend(diagnostics);
        self.import_files.extend(import_files);
        Ok(())
    }

    fn finish_ok(self) -> ExitCode {
        let diagnostics = Diagnostic::dedup(self.diagnostics);
        if self.args.json {
            print_envelope(
                "ok",
                self.outputs,
                &diagnostics,
                self.src,
                &self.import_files,
            );
        } else {
            print_diagnostics_stderr(&diagnostics);
        }
        ExitCode::SUCCESS
    }

    fn finish_blocked(mut self, stop: Stop) -> ExitCode {
        self.diagnostics.extend(stop.diagnostics);
        self.import_files.extend(&stop.import_files);
        let diagnostics = Diagnostic::dedup(self.diagnostics);
        if self.args.json {
            print_envelope(
                "blocked",
                self.outputs,
                &diagnostics,
                self.src,
                &self.import_files,
            );
        } else {
            print_diagnostics_stderr(&diagnostics);
            eprintln!(
                "render blocked by {} hard diagnostic(s)",
                diagnostics.iter().filter(|d| d.is_error()).count()
            );
        }
        ExitCode::from(stop.exit_code)
    }
}

/// Stop with exit code 2 when any diagnostic is an error.
fn gate(diagnostics: &[Diagnostic], import_files: &ImportFiles) -> Result<(), Stop> {
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
    diagnostics: &[Diagnostic],
    src: &str,
    import_files: &ImportFiles,
) {
    let out = RenderOutput {
        schema: RENDER_SCHEMA,
        status,
        outputs,
        diagnostics: DiagnosticJson::located_all_in(diagnostics, src, import_files),
    };
    println!("{}", serialize_pretty(&out));
}
