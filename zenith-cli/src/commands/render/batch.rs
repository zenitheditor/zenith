//! Shared encoding for compiled batch scenes.

use crate::native::NativeConfig;
use std::path::Path;
use zenith_core::{AssetProvider, Diagnostic, DiagnosticPolicy, Document, FontProvider};
use zenith_pipeline::PolicyFlags;
use zenith_pipeline::policy::{load_policy_layers, merge_policy};
use zenith_pipeline::render::{
    check_render_scale, pdf_rasterization_diagnostics, rasterization_diagnostics,
};
use zenith_render::{
    PdfExportOptions, SvgOptions, render_pdf_multi_report_with_options, render_png,
    render_svg_with_options,
};
use zenith_scene::Scene;

/// Output format for merge and variant generation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum BatchFormat {
    #[default]
    Png,
    Svg,
    Pdf,
}

/// How a batch format maps the pages of one row to files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchLayout {
    /// One file per page.
    PerPage,
    /// One file holds every page of the row.
    PerRow,
}

impl BatchFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Svg => "svg",
            Self::Pdf => "pdf",
        }
    }

    /// The file layout of one row in this format.
    pub fn layout(self) -> BatchLayout {
        match self {
            Self::Png | Self::Svg => BatchLayout::PerPage,
            Self::Pdf => BatchLayout::PerRow,
        }
    }

    /// Whether this format has vector output with raster fallback captures.
    ///
    /// The config policy governs the fallback diagnostics of such a format.
    /// Reports carry them, and `raster_scale` sets their resolution.
    pub fn is_vector(self) -> bool {
        match self {
            Self::Png => false,
            Self::Svg | Self::Pdf => true,
        }
    }
}

/// Output format and vector fallback resolution for batch generation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BatchExportOptions {
    pub format: BatchFormat,
    pub raster_scale: f64,
    /// Explicit PDF DeviceCMYK override (`None` follows the document's `colorspace`).
    pub cmyk: Option<bool>,
}

impl Default for BatchExportOptions {
    fn default() -> Self {
        Self::from(BatchFormat::Png)
    }
}

impl From<BatchFormat> for BatchExportOptions {
    fn from(format: BatchFormat) -> Self {
        Self {
            format,
            raster_scale: 1.0,
            cmyk: None,
        }
    }
}

impl BatchExportOptions {
    pub(crate) fn check(self) -> Result<(), String> {
        check_render_scale(self.raster_scale, &self.raster_scale.to_string())?;
        if !self.format.is_vector() && self.raster_scale != 1.0 {
            return Err(
                "raster_scale requires SVG or PDF batch output. Set format to svg or pdf"
                    .to_owned(),
            );
        }
        Ok(())
    }

    /// Resolve `cmyk` from `doc.colorspace` when not explicitly overridden.
    pub(crate) fn resolved_for_doc(mut self, doc: &Document) -> Self {
        if self.cmyk.is_none() && doc.colorspace.as_deref() == Some("cmyk") {
            self.cmyk = Some(true);
        }
        self
    }
}

pub(crate) fn load_batch_policy(
    format: BatchFormat,
    doc: &Document,
    dir: Option<&Path>,
) -> Result<Option<DiagnosticPolicy>, String> {
    if !format.is_vector() {
        return Ok(None);
    }
    let (global, local) = load_policy_layers(&NativeConfig, dir)?;
    Ok(Some(merge_policy(
        &global,
        &local,
        &doc.diagnostic_policy,
        &PolicyFlags::default(),
    )))
}

/// Encode one scene as one file.
///
/// `page` is the one-based document page of `scene`.
pub(crate) fn encode_batch_scene(
    options: BatchExportOptions,
    scene: &Scene,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    page: usize,
    policy: Option<&DiagnosticPolicy>,
) -> Result<(Vec<u8>, Vec<Diagnostic>), Vec<Diagnostic>> {
    match options.format {
        BatchFormat::Png => render_png(scene, fonts, assets)
            .map(|bytes| (bytes, Vec::new()))
            .map_err(|e| {
                vec![Diagnostic::error(
                    "render.raster_failed",
                    format!("render error: {e}"),
                    None,
                    None,
                )]
            }),
        BatchFormat::Svg => {
            let output = render_svg_with_options(scene, fonts, assets, SvgOptions { raster_scale: options.raster_scale }).map_err(|e| vec![Diagnostic::error("render.svg_failed", format!("SVG export failed on page {page}: {e}. Check page resources and scene commands"), None, None)])?;
            governed(
                rasterization_diagnostics(&output.rasterized_regions, page),
                policy,
            )
            .map(|diagnostics| (output.bytes, diagnostics))
        }
        BatchFormat::Pdf => encode_batch_pdf(
            options,
            std::slice::from_ref(scene),
            &[page],
            fonts,
            assets,
            policy,
        ),
    }
}

/// Encode `scenes` as one PDF with fonts subset to the used glyphs.
///
/// `pages` holds the one-based document page of each scene.
/// Raster fallback advisories name those pages.
pub(crate) fn encode_batch_pdf(
    options: BatchExportOptions,
    scenes: &[Scene],
    pages: &[usize],
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    policy: Option<&DiagnosticPolicy>,
) -> Result<(Vec<u8>, Vec<Diagnostic>), Vec<Diagnostic>> {
    let mut output = render_pdf_multi_report_with_options(
        scenes,
        fonts,
        assets,
        PdfExportOptions {
            subset: true,
            raster_scale: options.raster_scale,
            cmyk: options.cmyk.unwrap_or(false),
        },
    )
    .map_err(|e| {
        let pages = pages
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        vec![Diagnostic::error(
            "render.pdf_failed",
            format!(
                "PDF export failed for document page(s) {pages}: {e}. Check page resources and scene commands"
            ),
            None,
            None,
        )]
    })?;
    for region in &mut output.rasterized_regions {
        if let Some(&page) = region.page.checked_sub(1).and_then(|n| pages.get(n)) {
            region.page = page;
        }
    }
    governed(
        pdf_rasterization_diagnostics(&output.rasterized_regions),
        policy,
    )
    .map(|diagnostics| (output.bytes, diagnostics))
}

/// Apply `policy` to export diagnostics. An Error blocks the output.
fn governed(
    diagnostics: Vec<Diagnostic>,
    policy: Option<&DiagnosticPolicy>,
) -> Result<Vec<Diagnostic>, Vec<Diagnostic>> {
    let diagnostics = match policy {
        Some(policy) => zenith_core::apply_policy(diagnostics, policy),
        None => diagnostics,
    };
    if Diagnostic::has_errors(&diagnostics) {
        return Err(diagnostics);
    }
    Ok(diagnostics)
}
