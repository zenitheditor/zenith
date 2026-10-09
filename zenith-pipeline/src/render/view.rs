//! [`view_page`]: compile one page of a parsed document with its node boxes,
//! and optionally rasterize it. The editor's canvas path.

use std::collections::BTreeMap;
use std::path::Path;

use zenith_core::{BytesAssetProvider, DataContext, Diagnostic, Document};
use zenith_render::{
    DeviceRect, PageRect, device_page_size, encode_png, render_image_scaled, render_region_png,
};
use zenith_scene::{CompiledBox, DocumentPrep, PageCompiler};

use super::region::{RegionView, region_error, region_rect};
use super::scale::check_render_scale;
use crate::assets::{
    build_asset_provider_with_imports, build_font_provider_with_imports, image_sizes,
    resolve_text_sources,
};
use crate::error::PipelineError;
use crate::host::Host;
use crate::imports::load_import_graph;
use crate::prepare::resolve_page_index;

/// Options of [`view_page`].
#[derive(Clone, Copy, Debug)]
pub struct ViewOptions<'a> {
    /// The context for `(data)` references.
    pub data: Option<&'a DataContext>,
    /// Run the page lint pass. Off saves its cost; the scene and boxes are
    /// the same either way.
    pub lint: bool,
    /// Rasterize at this scale (`1.0` = page pixels). `None` skips the
    /// raster.
    pub raster: Option<f64>,
    /// Rasterize only this window of the page (page pixels; see
    /// [`render_png_region`](super::render_png_region)). `None` rasterizes
    /// the whole page, with the scale capped at
    /// [`MAX_RENDER_SCALE`](super::MAX_RENDER_SCALE).
    pub viewport: Option<PageRect>,
}

impl Default for ViewOptions<'_> {
    /// No data, no lint, no raster.
    fn default() -> Self {
        Self {
            data: None,
            lint: false,
            raster: None,
            viewport: None,
        }
    }
}

/// An encoded PNG with its pixel size and the device rect it covers.
#[derive(Debug, Clone, PartialEq)]
pub struct PngImage {
    /// The encoded PNG bytes.
    pub png: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The device rect of the page the PNG covers: the whole page without a
    /// viewport.
    pub rect: DeviceRect,
    /// The page size `(width, height)` in device pixels at the raster scale.
    pub device_size: (u32, u32),
    /// The page size `(width, height)` in page pixels (the scene's media
    /// box, bleed included).
    pub page_size: (f64, f64),
}

/// One compiled page of a parsed document.
#[derive(Debug)]
pub struct PageView {
    /// The number of pages in the document.
    pub page_count: usize,
    /// The final geometry of every compiled node of the page, by raw id
    /// (see [`PageCompiler::compile_page_with_boxes`]).
    pub boxes: BTreeMap<String, CompiledBox>,
    /// The page raster, when [`ViewOptions::raster`] asked for one.
    pub image: Option<PngImage>,
    /// Text-source diagnostics, then the page's compile diagnostics, with
    /// no config policy applied. Repeats are removed.
    pub diagnostics: Vec<Diagnostic>,
}

/// Compile the 0-based `page_index` of `doc` with its node boxes, and
/// rasterize it (or its `opts.viewport` window) when `opts.raster` is set.
///
/// `doc` is used as parsed: no config is read and no validation runs, so
/// the caller validates first. Text sources, composition imports, project
/// fonts, and image assets resolve from `project_dir` through `host.fs`, as
/// on the render path, so the raster equals [`render_png`](super::render_png)
/// for the same document and scale.
///
/// # Errors
///
/// `render.page_out_of_range` (exit 2), an asset or font load error
/// (exit 2), `render.invalid_scale` (exit 2), a region limit error (see
/// [`render_png_region`](super::render_png_region), exit 2), or a raster
/// error (exit 2).
pub fn view_page(
    host: Host<'_>,
    doc: &Document,
    project_dir: Option<&Path>,
    page_index: usize,
    opts: ViewOptions<'_>,
) -> Result<PageView, PipelineError> {
    let mut doc = doc.clone();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    resolve_text_sources(host.fs, &mut doc, project_dir, &mut diagnostics);
    let page_index = resolve_page_index(&doc, page_index.saturating_add(1))?;
    // A viewport render checks its scale against the region limits instead.
    let scale = match (opts.raster, opts.viewport) {
        (Some(scale), None) => Some(
            check_render_scale(scale, &scale.to_string())
                .map_err(|message| PipelineError::new("render.invalid_scale", message, 2))?,
        ),
        (raster, _) => raster,
    };
    let imports = load_import_graph(host.fs, &doc, project_dir);
    let fonts = build_font_provider_with_imports(host, &doc, project_dir, &imports, false)?;
    let assets = match project_dir {
        Some(dir) => build_asset_provider_with_imports(host.fs, &doc, dir, &imports, false)?,
        None => BytesAssetProvider::new(),
    };
    let scene_imports = imports.to_scene_graph();
    let prep = DocumentPrep::new(&doc, opts.data, Some(&scene_imports))
        .with_image_sizes(image_sizes(&doc, Some(&imports), &assets));
    let compiler = PageCompiler::new(&prep, &fonts);
    let (result, boxes) = compiler.compile_page_with_boxes(page_index, opts.lint);
    diagnostics.extend(result.diagnostics);
    let image = match (scale, opts.viewport) {
        (Some(scale), Some(viewport)) => {
            let scene = &result.scene;
            let device_size =
                device_page_size(scene.width, scene.height, scale).map_err(|e| region_error(&e))?;
            let rect = region_rect(scene.width, scene.height, scale, RegionView::Page(viewport))?;
            let png = render_region_png(scene, scale, rect, &fonts, &assets)
                .map_err(|e| region_error(&e))?;
            Some(PngImage {
                png,
                width: rect.width,
                height: rect.height,
                rect,
                device_size,
                page_size: (scene.width, scene.height),
            })
        }
        (Some(scale), None) => {
            let raster_err = |e: zenith_render::RenderError| {
                PipelineError::new("render.raster_failed", format!("render error: {e}"), 2)
            };
            let image =
                render_image_scaled(&result.scene, scale, &fonts, &assets).map_err(raster_err)?;
            let png = encode_png(&image).map_err(raster_err)?;
            Some(PngImage {
                png,
                width: image.width,
                height: image.height,
                rect: DeviceRect {
                    x: 0,
                    y: 0,
                    width: image.width,
                    height: image.height,
                },
                device_size: (image.width, image.height),
                page_size: (result.scene.width, result.scene.height),
            })
        }
        (None, _) => None,
    };
    Ok(PageView {
        page_count: doc.body.pages.len(),
        boxes,
        image,
        diagnostics: Diagnostic::dedup(diagnostics),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::{MemFs, NoConfig};
    use crate::policy::PolicyFlags;
    use crate::prepare::parse;
    use crate::render::{RenderOptions, render_png};

    const SRC: &str = r##"zenith version=1 {
  project id="p" name="P"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#204080"
  }
  styles {}
  document id="d" title="D" {
    page id="pg" w=(px)120 h=(px)80 {
      rect id="r" x=(px)10 y=(px)10 w=(px)50 h=(px)30 fill=(token)"color.ink" rotate=(deg)20
    }
    page id="pg2" w=(px)60 h=(px)60 {
      ellipse id="e" x=(px)5 y=(px)5 w=(px)20 h=(px)20 fill=(token)"color.ink"
    }
  }
}
"##;

    #[test]
    fn raster_equals_render_png_and_boxes_are_recorded() {
        let fs = MemFs::new();
        let host = Host::new(&fs, &NoConfig);
        let doc = parse(SRC).expect("parse");
        let view = view_page(
            host,
            &doc,
            None,
            0,
            ViewOptions {
                raster: Some(1.5),
                ..ViewOptions::default()
            },
        )
        .expect("view");
        assert_eq!(view.page_count, 2);
        let rect = view.boxes.get("r").expect("box");
        assert_eq!(rect.rotate, Some(20.0));
        let flags = PolicyFlags::default();
        let png = render_png(
            host,
            SRC,
            None,
            1,
            RenderOptions::new(&flags).with_scale(1.5),
        )
        .expect("render");
        let image = view.image.expect("image");
        assert_eq!(image.png, png.png);
        assert_eq!((image.width, image.height), (180, 120));
    }

    #[test]
    fn viewport_raster_is_the_region_render_and_lifts_the_scale_cap() {
        let fs = MemFs::new();
        let host = Host::new(&fs, &NoConfig);
        let doc = parse(SRC).expect("parse");
        let viewport = PageRect {
            x: 30.25,
            y: 12.5,
            width: 40.0,
            height: 30.0,
        };
        let view = view_page(
            host,
            &doc,
            None,
            0,
            ViewOptions {
                raster: Some(9.0),
                viewport: Some(viewport),
                ..ViewOptions::default()
            },
        )
        .expect("view");
        let image = view.image.expect("image");
        assert_eq!(
            image.rect,
            DeviceRect {
                x: 272,
                y: 112,
                width: 361,
                height: 271
            }
        );
        assert_eq!(image.device_size, (1080, 720));
        assert_eq!(image.page_size, (120.0, 80.0));
        let flags = PolicyFlags::default();
        let region = crate::render::render_png_region(
            host,
            SRC,
            None,
            1,
            RenderOptions::new(&flags).with_scale(9.0),
            RegionView::Page(viewport),
        )
        .expect("region");
        assert_eq!(image.png, region.png);
    }

    #[test]
    fn no_raster_and_page_range() {
        let fs = MemFs::new();
        let host = Host::new(&fs, &NoConfig);
        let doc = parse(SRC).expect("parse");
        let view = view_page(host, &doc, None, 1, ViewOptions::default()).expect("view");
        assert!(view.image.is_none());
        assert!(view.boxes.contains_key("e"));
        let err = view_page(host, &doc, None, 2, ViewOptions::default()).expect_err("range");
        assert_eq!(err.diagnostics[0].code, "render.page_out_of_range");
        let bad = view_page(
            host,
            &doc,
            None,
            0,
            ViewOptions {
                raster: Some(9.0),
                ..ViewOptions::default()
            },
        )
        .expect_err("scale");
        assert_eq!(bad.diagnostics[0].code, "render.invalid_scale");
    }
}
