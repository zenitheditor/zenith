//! [`render_png_region`]: one window of a page, at any scale, as PNG.
//!
//! The editor canvas renders only the visible part of a page at the exact
//! device scale (`zoom × devicePixelRatio`). The window and its limits are
//! the `zenith-render` region rules: see [`zenith_render::snap_view`].

use std::path::Path;

use zenith_core::Diagnostic;
use zenith_render::{
    DeviceRect, PageRect, RegionError, device_page_size, render_region_png, snap_view,
};

use super::options::RenderOptions;
use super::png::compile_png_page;
use crate::error::PipelineError;
use crate::host::Host;
use crate::imports::ImportFiles;

/// The window of a region render.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RegionView {
    /// A rectangle in page pixels. The render covers it, snapped out to whole
    /// device pixels and clamped to the page (see [`snap_view`]).
    Page(PageRect),
    /// A rectangle in device pixels of the page at the render scale.
    Device(DeviceRect),
}

/// A region PNG plus the diagnostics that produced it.
#[derive(Debug)]
pub struct RegionArtifact {
    /// The encoded PNG bytes, `rect.width × rect.height` pixels.
    pub png: Vec<u8>,
    /// The device rect the PNG covers.
    pub rect: DeviceRect,
    /// The output scale.
    pub scale: f64,
    /// The page size `(width, height)` in device pixels at `scale`.
    pub device_size: (u32, u32),
    /// The page size `(width, height)` in page pixels (the scene's media
    /// box, bleed included).
    pub page_size: (f64, f64),
    /// The number of pages in the document.
    pub page_count: usize,
    /// Validation diagnostics, then compile-stage diagnostics, as
    /// [`render_png`](super::render_png) reports them.
    pub diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports, for locating diagnostic spans.
    pub import_files: ImportFiles,
}

/// The pipeline error of a refused or failed region render (exit 2).
#[must_use]
pub fn region_error(e: &RegionError) -> PipelineError {
    PipelineError::new(e.code(), e.to_string(), 2)
}

/// Resolve `view` to a device rect of a `width × height` page at `scale`.
///
/// # Errors
///
/// The [`snap_view`] errors for [`RegionView::Page`], and the
/// [`device_page_size`] errors for [`RegionView::Device`], as
/// [`region_error`].
pub fn region_rect(
    width: f64,
    height: f64,
    scale: f64,
    view: RegionView,
) -> Result<DeviceRect, PipelineError> {
    match view {
        RegionView::Page(page_rect) => snap_view(width, height, scale, page_rect),
        RegionView::Device(rect) => device_page_size(width, height, scale).map(|_| rect),
    }
    .map_err(|e| region_error(&e))
}

/// Parse and validate `src` with the merged policy, compile 1-based `page`,
/// and rasterize the `view` window of it to PNG at `opts.scale`.
///
/// Inputs, diagnostics, and errors match [`render_png`](super::render_png),
/// except the scale: it has no fixed cap, only the region limits. The pixels
/// equal the same window of [`render_png`](super::render_png) at the same
/// scale whenever the whole page at that scale fits the region caps (see the
/// `zenith-render` region docs).
///
/// # Errors
///
/// The [`render_png`](super::render_png) errors, plus `render.invalid_scale`,
/// `render.scale_too_large`, `render.invalid_viewport`, and
/// `render.region_too_large` (exit 2) from the region rules.
pub fn render_png_region(
    host: Host<'_>,
    src: &str,
    project_dir: Option<&Path>,
    page: usize,
    opts: RenderOptions<'_>,
    view: RegionView,
) -> Result<RegionArtifact, PipelineError> {
    let compiled = compile_png_page(host, src, project_dir, page, opts)?;
    let scene = &compiled.scene;
    let device_size =
        device_page_size(scene.width, scene.height, opts.scale).map_err(|e| region_error(&e))?;
    let rect = region_rect(scene.width, scene.height, opts.scale, view)?;
    let png = render_region_png(scene, opts.scale, rect, &compiled.fonts, &compiled.assets)
        .map_err(|e| region_error(&e))?;
    Ok(RegionArtifact {
        png,
        rect,
        scale: opts.scale,
        device_size,
        page_size: (scene.width, scene.height),
        page_count: compiled.page_count,
        diagnostics: compiled.diagnostics,
        import_files: compiled.import_files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::{MemFs, NoConfig};
    use crate::policy::PolicyFlags;
    use crate::render::render_png;

    const SRC: &str = r##"zenith version=1 {
  project id="p" name="P"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#204080"
  }
  styles {}
  document id="d" title="D" {
    page id="pg" w=(px)120 h=(px)80 {
      ellipse id="e" x=(px)10 y=(px)10 w=(px)90 h=(px)50 fill=(token)"color.ink" rotate=(deg)20
    }
  }
}
"##;

    #[test]
    fn whole_page_region_equals_render_png() {
        let fs = MemFs::new();
        let host = Host::new(&fs, &NoConfig);
        let flags = PolicyFlags::default();
        let opts = RenderOptions::new(&flags).with_scale(1.5);
        let full = render_png(host, SRC, None, 1, opts).expect("render");
        let view = RegionView::Page(PageRect {
            x: 0.0,
            y: 0.0,
            width: 120.0,
            height: 80.0,
        });
        let region = render_png_region(host, SRC, None, 1, opts, view).expect("region");
        assert_eq!(region.png, full.png);
        assert_eq!(region.device_size, (180, 120));
        assert_eq!(region.page_size, (120.0, 80.0));
        assert_eq!(
            region.rect,
            DeviceRect {
                x: 0,
                y: 0,
                width: 180,
                height: 120
            }
        );
        assert_eq!(region.diagnostics, full.diagnostics);
    }

    #[test]
    fn region_limits_are_pipeline_errors() {
        let fs = MemFs::new();
        let host = Host::new(&fs, &NoConfig);
        let flags = PolicyFlags::default();
        let view = RegionView::Device(DeviceRect {
            x: 0,
            y: 0,
            width: 10,
            height: 10,
        });
        let code = |scale: f64, view: RegionView| {
            let opts = RenderOptions::new(&flags).with_scale(scale);
            render_png_region(host, SRC, None, 1, opts, view)
                .expect_err("refused")
                .diagnostics[0]
                .code
                .clone()
        };
        assert_eq!(code(0.0, view), "render.invalid_scale");
        assert_eq!(code(f64::NAN, view), "render.invalid_scale");
        assert_eq!(code(1e7, view), "render.scale_too_large");
        let off = RegionView::Device(DeviceRect {
            x: 170,
            y: 0,
            width: 20,
            height: 10,
        });
        assert_eq!(code(1.5, off), "render.invalid_viewport");
        let huge = RegionView::Page(PageRect {
            x: 0.0,
            y: 0.0,
            width: 120.0,
            height: 80.0,
        });
        assert_eq!(code(100.0, huge), "render.region_too_large");
    }
}
