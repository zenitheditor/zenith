//! [`perceive_pages`]: render pages as `zenith render` does, then measure
//! them.

use std::path::Path;

use zenith_core::Diagnostic;
use zenith_render::render_image_scaled;

use super::analyze::{PageInputs, ink_output, page_metrics};
use super::inputs::trim_rect;
use super::output::{
    PERCEIVE_DECIMALS, PERCEIVE_NOTE, PERCEIVE_SCHEMA, PagePerception, PerceiveReport,
};
use super::raster::{crop, ink, surface};
use crate::error::PipelineError;
use crate::host::Host;
use crate::imports::ImportFiles;
use crate::io::map_slice;
use crate::policy::PolicyFlags;
use crate::prepare::parse;
use crate::render::{CompiledPage, PageSelection, RenderOptions, compile_pages};

/// The signals of the selected pages plus the diagnostics of their render.
#[derive(Debug)]
pub struct PerceiveArtifact {
    /// The envelope.
    pub report: PerceiveReport,
    /// Validation, import, disk, and compile diagnostics of the render.
    /// None is an error.
    pub diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports, for locating diagnostic spans.
    pub import_files: ImportFiles,
}

/// Render the `selection` of pages of `src` to PNG pixels as
/// `zenith render --png` does at scale 1, then measure each page.
///
/// The measured area is the trim box: a page bleed is cut off. Metrics that
/// need a document input (margins, construction guides) report
/// `not_applicable` when the page declares none. Pages measure through
/// `host.runner`, and results stay in page order.
///
/// # Errors
///
/// Every [`compile_pages`] error, error diagnostics from the compile (exit
/// 1, as `zenith render` blocks on them), `render.raster_failed` (exit 2),
/// or `perceive.raster_failed` (exit 2).
pub fn perceive_pages(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    selection: PageSelection,
    flags: &PolicyFlags,
) -> Result<PerceiveArtifact, PipelineError> {
    let doc = parse(src)?;
    let opts = RenderOptions::new(flags).with_parsed(Some(&doc));
    let compiled = compile_pages(host, src, project_dir, selection, opts, true)?;
    if Diagnostic::has_errors(&compiled.diagnostics) {
        return Err(PipelineError::blocked(compiled.diagnostics, 1)
            .with_import_files(compiled.import_files));
    }
    let fonts = &compiled.fonts;
    let assets = &compiled.assets;
    let measured = map_slice(host.runner, &compiled.pages, |p: &CompiledPage| {
        let image = render_image_scaled(&p.scene, 1.0, fonts, assets).map_err(|e| {
            PipelineError::new(
                "render.raster_failed",
                format!("render error on page {}: {e}", p.page),
                2,
            )
        })?;
        let page_index = p.page.saturating_sub(1);
        let page = doc.body.pages.get(page_index).ok_or_else(|| {
            PipelineError::new(
                "render.page_out_of_range",
                format!("page {} is not in the document", p.page),
                2,
            )
        })?;
        let area = crop(
            &image.rgba,
            image.width,
            image.height,
            trim_rect(page, image.width, image.height),
        )?;
        let ink = ink(&area);
        let surface = surface(&area)?;
        let empty = Default::default();
        let boxes = p.boxes.as_ref().unwrap_or(&empty);
        Ok(PagePerception {
            page: p.page,
            id: page.id.clone(),
            width: area.width,
            height: area.height,
            ink: ink_output(ink),
            metrics: page_metrics(PageInputs {
                doc: &doc,
                page,
                boxes,
                surface: &surface,
                ink,
            }),
        })
    });
    let pages = measured.into_iter().collect::<Result<Vec<_>, _>>()?;
    Ok(PerceiveArtifact {
        report: PerceiveReport {
            schema: PERCEIVE_SCHEMA,
            note: PERCEIVE_NOTE,
            decimals: PERCEIVE_DECIMALS,
            page_count: compiled.page_count,
            pages,
        },
        diagnostics: compiled.diagnostics,
        import_files: compiled.import_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::{MemFs, NoConfig};
    use crate::perceive::output::Metric;

    const DOC: &str = r##"zenith version=1 {
  tokens format="zenith-token-v1" {
    token id="ink" type="color" value="#000000"
  }
  document id="d" {
    page id="one" w=(px)40 h=(px)20 bleed=(px)4 {
      construction {
        guide id="g" type="segment" x1=(px)10 y1=(px)0 x2=(px)10 y2=(px)20
      }
      rect id="r" x=(px)10 y=(px)5 w=(px)10 h=(px)10 fill=(token)"ink"
    }
    page id="two" w=(px)10 h=(px)10 {}
  }
}
"##;

    fn run(selection: PageSelection) -> Result<PerceiveArtifact, PipelineError> {
        let fs = MemFs::new();
        let host = Host::new(&fs, &NoConfig);
        perceive_pages(host, DOC, None, selection, &PolicyFlags::default())
    }

    #[test]
    fn one_page_measures_the_trim_box() {
        let art = run(PageSelection::One(1)).expect("perceive");
        assert_eq!(art.report.schema, "zenith-perceive-page-v1");
        assert_eq!(art.report.page_count, 2);
        assert_eq!(art.report.pages.len(), 1);
        let page = &art.report.pages[0];
        assert_eq!((page.page, page.id.as_str()), (1, "one"));
        assert_eq!((page.width, page.height), (40, 20));
        assert_eq!(page.ink.pixel_count, 100);
        let Metric::Measured(grid) = &page.metrics.grid_conformance else {
            panic!(
                "grid conformance must be measured: {:?}",
                page.metrics.grid_conformance
            );
        };
        assert_eq!(grid.guide_count, 1);
        assert_eq!(grid.node_count, 1);
    }

    #[test]
    fn all_pages_keep_page_order() {
        let art = run(PageSelection::All).expect("perceive");
        let ids: Vec<&str> = art.report.pages.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["one", "two"]);
    }

    #[test]
    fn out_of_range_page_is_an_error() {
        assert!(run(PageSelection::One(3)).is_err());
    }

    #[test]
    fn perceiving_twice_gives_the_same_report() {
        let a = run(PageSelection::All).expect("perceive").report;
        let b = run(PageSelection::All).expect("perceive").report;
        assert_eq!(a, b);
    }
}
