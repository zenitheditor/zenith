//! Shared encoding for compiled batch scenes.

use crate::config::{CliPolicyFlags, find_local_policy, load_global_policy, merge_policy};
use std::path::Path;
use zenith_core::{AssetProvider, Diagnostic, DiagnosticPolicy, Document, FontProvider};
use zenith_render::{render_png, render_svg_with};
use zenith_scene::Scene;

/// Output format for merge and variant generation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum BatchFormat {
    #[default]
    Png,
    Svg,
}

impl BatchFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Svg => "svg",
        }
    }
}

pub(crate) fn load_batch_policy(
    format: BatchFormat,
    doc: &Document,
    dir: Option<&Path>,
) -> Result<Option<DiagnosticPolicy>, String> {
    match format {
        BatchFormat::Png => Ok(None),
        BatchFormat::Svg => {
            let global = load_global_policy()?;
            let local = match dir {
                Some(dir) => find_local_policy(dir)?,
                None => DiagnosticPolicy::default(),
            };
            Ok(Some(merge_policy(
                &global,
                &local,
                &doc.diagnostic_policy,
                &CliPolicyFlags::default(),
            )))
        }
    }
}

pub(crate) fn encode_batch_scene(
    format: BatchFormat,
    scene: &Scene,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    page: usize,
    policy: Option<&DiagnosticPolicy>,
) -> Result<(Vec<u8>, Vec<Diagnostic>), Vec<Diagnostic>> {
    match format {
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
            let output = render_svg_with(scene, fonts, assets).map_err(|e| vec![Diagnostic::error("render.svg_failed", format!("SVG export failed on page {page}: {e}. Check page resources and scene commands"), None, None)])?;
            let diagnostics =
                super::svg::rasterization_diagnostics(&output.rasterized_regions, page);
            let diagnostics = match policy {
                Some(policy) => zenith_core::apply_policy(diagnostics, policy),
                None => diagnostics,
            };
            if Diagnostic::has_errors(&diagnostics) {
                return Err(diagnostics);
            }
            Ok((output.bytes, diagnostics))
        }
    }
}
