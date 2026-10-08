//! Dispatch logic for `zenith render`.
//!
//! Requested outputs stage before writing. Hard diagnostics block every staged output.
//! Diagnostics merge and print once as JSON or grouped stderr lines.
//! JSON uses the `zenith-render-v1` envelope with `ok` or `blocked` status.

use std::path::Path;
use std::process::ExitCode;

use zenith_core::{DataContext, Diagnostic};

use crate::cli::RenderArgs;
use crate::cli_helpers::{parse_spread_spec, read_file};
use crate::commands;
use crate::commands::render::{RenderCmdErr, RenderEntryOptions, SpreadRenderOpts};
use crate::config::CliPolicyFlags;
use crate::json_types::RenderImageJson;
use crate::report::{CliError, ImportFiles};

use super::output::gate;

pub(in crate::dispatch) fn dispatch_render(args: RenderArgs) -> ExitCode {
    let json = args.json;
    if args.scene.is_none()
        && args.png.is_none()
        && args.svg.is_none()
        && args.all_pages_svg.is_none()
        && args.pdf.is_none()
        && args.all_pages.is_none()
        && args.contact_sheet.is_none()
    {
        return CliError::usage(
            "error: at least one of --scene <OUT>, --png <OUT>, --svg <OUT>, --pdf <OUT>, --all-pages <DIR>, --all-pages-svg <DIR>, or --contact-sheet <OUT> is required",
        )
        .emit(json);
    }
    let scale = match args.scale.as_deref().map(parse_scale).transpose() {
        Ok(s) => s,
        Err(msg) => return CliError::usage(msg).emit(json),
    };
    if scale.is_some()
        && args.png.is_none()
        && args.all_pages.is_none()
        && args.contact_sheet.is_none()
    {
        return CliError::usage(
            "error: --scale applies only to PNG outputs; add --png <OUT>, --all-pages <DIR>, or --contact-sheet <OUT>",
        )
        .emit(json);
    }
    let raster_scale = match args
        .raster_scale
        .as_deref()
        .map(|raw| commands::render::parse_scale_flag(raw, "--raster-scale"))
        .transpose()
    {
        Ok(scale) => scale.unwrap_or(1.0),
        Err(message) => return CliError::usage(message).emit(json),
    };
    if args.raster_scale.is_some()
        && args.pdf.is_none()
        && args.svg.is_none()
        && args.all_pages_svg.is_none()
    {
        return CliError::usage(
            "error: --raster-scale requires --pdf <OUT>, --svg <OUT>, or --all-pages-svg <DIR>",
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
        scale,
        raster_scale,
        pending: Vec::new(),
        directories: Vec::new(),
        messages: Vec::new(),
        outputs: Vec::new(),
        images: Vec::new(),
        rasterized_regions: Vec::new(),
        diagnostics: Vec::new(),
        import_files: ImportFiles::default(),
    };
    match run.render_all(spread).and_then(|()| run.flush()) {
        Ok(()) => run.finish_ok(),
        Err(stop) => run.finish_blocked(stop),
    }
}

/// Parse a `--scale` value: a finite number with `0 < F <= 4`.
fn parse_scale(raw: &str) -> Result<f64, String> {
    commands::render::parse_scale_flag(raw, "--scale")
}

/// Why a render run stopped.
pub(super) struct Stop {
    /// Diagnostics of the failing output. At least one is an error.
    pub(super) diagnostics: Vec<Diagnostic>,
    pub(super) exit_code: u8,
    /// Files of the composition imports behind the diagnostic spans.
    pub(super) import_files: ImportFiles,
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
pub(super) struct RenderRun<'a> {
    pub(super) args: &'a RenderArgs,
    pub(super) src: &'a str,
    pub(super) flags: &'a CliPolicyFlags,
    pub(super) data: Option<&'a DataContext>,
    /// The checked `--scale`, when given.
    pub(super) scale: Option<f64>,
    pub(super) raster_scale: f64,
    pub(super) directories: Vec<std::path::PathBuf>,
    pub(super) messages: Vec<String>,
    pub(super) pending: Vec<(std::path::PathBuf, Vec<u8>)>,
    /// Written paths, in write order.
    pub(super) outputs: Vec<String>,
    /// Written PNGs with size and scale, in write order.
    pub(super) images: Vec<RenderImageJson>,
    pub(super) rasterized_regions: Vec<crate::json_types::RenderRasterizedRegionJson>,
    /// Diagnostics of every finished output, in output order.
    pub(super) diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports behind the diagnostic spans.
    pub(super) import_files: ImportFiles,
}

impl RenderRun<'_> {
    /// The raster scale of this run (`1.0` when `--scale` is absent).
    fn output_scale(&self) -> f64 {
        self.scale.unwrap_or(1.0)
    }

    pub(super) fn entry_options(&self) -> RenderEntryOptions<'_> {
        RenderEntryOptions {
            locked: self.args.locked,
            subset: !self.args.embed_full_fonts,
            flags: self.flags,
            data: self.data,
            construction_overlay: self.args.construction_overlay,
            scale: self.output_scale(),
            raster_scale: self.raster_scale,
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
                    scale: self.output_scale(),
                },
            )?;
            let image = self.image(png_out, "spread", (artifact.width, artifact.height));
            self.write(
                png_out,
                artifact.png,
                artifact.diagnostics,
                &artifact.import_files,
                "spread PNG",
            )?;
            self.record_image(RenderImageJson {
                pages: vec![page_a, page_b],
                ..image
            });
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
                artifact.json.into_bytes(),
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
            let image = self.image(png_out, "png", (artifact.width, artifact.height));
            self.write(
                png_out,
                artifact.png,
                artifact.diagnostics,
                &artifact.import_files,
                "PNG",
            )?;
            self.record_image(RenderImageJson {
                pages: vec![args.page.unwrap_or(1)],
                ..image
            });
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
            self.rasterized_regions
                .extend(artifact.rasterized_regions.into_iter().map(|region| {
                    crate::json_types::RenderRasterizedRegionJson {
                        path: pdf_out.display().to_string(),
                        format: "pdf",
                        page: region.page,
                        command_start: region.command_start,
                        command_end: region.command_end,
                        reason: format!("{:?}", region.reason),
                        raster_scale: (self.raster_scale != 1.0).then_some(self.raster_scale),
                    }
                }));
            self.write(
                pdf_out,
                artifact.pdf,
                artifact.diagnostics,
                &artifact.import_files,
                "PDF",
            )?;
        }
        self.render_svg_outputs()?;
        if let Some(out_dir) = &args.all_pages {
            self.render_pages(out_dir)?;
        }
        if let Some(sheet_out) = &args.contact_sheet {
            self.render_contact_sheet(sheet_out)?;
        }
        Ok(())
    }

    /// Render the contact sheet to `out` and record it.
    fn render_contact_sheet(&mut self, out: &Path) -> Result<(), Stop> {
        let sheet = commands::render::to_contact_sheet(
            self.src,
            self.args.path.parent(),
            self.args.page,
            self.scale,
            self.entry_options(),
        )?;
        gate(&sheet.diagnostics, &sheet.import_files)?;
        self.pending.push((out.to_path_buf(), sheet.png));
        if !self.args.json {
            self.messages.push(format!(
                "contact sheet written to '{}' ({}x{} px, {} page(s), {} column(s), scale {})",
                out.display(),
                sheet.width,
                sheet.height,
                sheet.pages.len(),
                sheet.columns,
                sheet.scale
            ));
        }
        self.images.push(RenderImageJson {
            path: out.display().to_string(),
            kind: "contact_sheet",
            width: sheet.width,
            height: sheet.height,
            scale: sheet.scale,
            pages: sheet.pages,
            columns: Some(sheet.columns),
            rows: Some(sheet.rows),
        });
        self.diagnostics.extend(sheet.diagnostics);
        self.import_files.extend(&sheet.import_files);
        Ok(())
    }

    /// An image record for `path` at this run's scale (pages filled by the
    /// caller).
    fn image(
        &self,
        path: &Path,
        kind: &'static str,
        (width, height): (u32, u32),
    ) -> RenderImageJson {
        RenderImageJson {
            path: path.display().to_string(),
            kind,
            width,
            height,
            scale: self.output_scale(),
            pages: Vec::new(),
            columns: None,
            rows: None,
        }
    }

    /// Record a written PNG and print its size in human mode.
    fn record_image(&mut self, image: RenderImageJson) {
        if !self.args.json {
            self.messages.push(format!(
                "  {}x{} px, scale {}",
                image.width, image.height, image.scale
            ));
        }
        self.images.push(image);
    }

    fn render_pages(&mut self, out_dir: &Path) -> Result<(), Stop> {
        self.directories.push(out_dir.to_path_buf());
        let artifact = commands::render::to_png_all_pages_options(
            self.src,
            self.args.path.parent(),
            self.entry_options(),
        )?;
        // Block on hard diagnostics before any page reaches disk.
        gate(&artifact.diagnostics, &artifact.import_files)?;
        let page_count = artifact.pages.len();
        for (i, (png, size)) in artifact.pages.into_iter().zip(artifact.sizes).enumerate() {
            let page_path = out_dir.join(format!("page-{}.png", i + 1));
            let image = self.image(&page_path, "page", size);
            self.pending.push((page_path, png));
            self.images.push(RenderImageJson {
                pages: vec![i + 1],
                ..image
            });
        }
        if !self.args.json {
            self.messages.push(format!(
                "{} page(s) written to '{}' (scale {})",
                page_count,
                out_dir.display(),
                self.output_scale()
            ));
        }
        self.diagnostics.extend(artifact.diagnostics);
        self.import_files.extend(&artifact.import_files);
        Ok(())
    }
}
