//! `doc.render {page?, scale?, viewport?}`: rasterize one page, or one
//! window of it, exactly as `zenith render --png` does.

use serde::Deserialize;
use serde_json::{Map, Value, json};
use zenith_core::{Diagnostic, FontMissLog};
use zenith_pipeline::render::{
    MAX_RENDER_SCALE, RegionView, check_render_scale, render_png, render_png_region,
};
use zenith_pipeline::{DeviceRect, PageRect, RenderOptions};

use super::common::{page_index, params};
use crate::ctx::{Ctx, ImageRegion, RenderedImage, pipeline_error};
use crate::error::EditorError;
use crate::fonts::missing_face_notices;
use crate::wire::DiagnosticOut;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenderParams {
    #[serde(default)]
    page: Option<usize>,
    #[serde(default)]
    scale: Option<f64>,
    #[serde(default)]
    viewport: Option<ViewportParams>,
}

/// A page-pixel window: `{x, y, w, h}`.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ViewportParams {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl ViewportParams {
    /// The window as a page rect.
    pub(crate) fn page_rect(self) -> PageRect {
        PageRect {
            x: self.x,
            y: self.y,
            width: self.w,
            height: self.h,
        }
    }
}

/// Render 1-based `page` (default: the session page) at `scale` (default:
/// the viewport zoom) and make it the session page.
///
/// With `viewport` (page px) only that window renders, snapped out to whole
/// device pixels, and the scale has no fixed cap: only the region limits
/// (`render.region_too_large`, `render.scale_too_large`,
/// `render.invalid_viewport`). Without it the reply and the PNG are the
/// whole-page render, scale at most 4.
///
/// While the text has errors the last valid text renders and the reply
/// says `stale: true`. The PNG goes out of band ([`Outcome::image`]); the
/// reply carries its size and SHA-256.
///
/// [`Outcome::image`]: crate::Outcome::image
pub(crate) fn run(ctx: &mut Ctx<'_, '_>, raw: Value) -> Result<Value, EditorError> {
    let p: RenderParams = params(ctx, raw)?;
    let display = ctx.display()?;
    let page_count = display.doc.body.pages.len();
    let index = page_index(ctx, p.page, page_count)?;
    let requested = p.scale.unwrap_or(ctx.session.viewport.zoom);
    let scale = match p.viewport {
        Some(_) => requested,
        None => scale(requested)?,
    };
    let log = FontMissLog::new();
    let env = ctx.env;
    let host = env.host.with_font_log(&log);
    let opts = RenderOptions::new(env.flags)
        .with_data(env.data)
        .with_scale(scale)
        .with_parsed(Some(&display.doc));
    ctx.work.compiles += 1;
    ctx.work.rasters += 1;
    let page = index + 1;
    let out = match p.viewport {
        Some(viewport) => {
            let view = RegionView::Page(viewport.page_rect());
            let a = render_png_region(host, &display.text, env.project_dir, page, opts, view)
                .map_err(|e| pipeline_error(&e, &display.text))?;
            let region = ImageRegion {
                x: a.rect.x,
                y: a.rect.y,
                scale,
                device_width: a.device_size.0,
                device_height: a.device_size.1,
                page_width: a.page_size.0,
                page_height: a.page_size.1,
            };
            Rendered {
                image: RenderedImage {
                    png: a.png,
                    width: a.rect.width,
                    height: a.rect.height,
                    page,
                    region: Some(region),
                },
                page_count: a.page_count,
                diagnostics: a.diagnostics,
            }
        }
        None => {
            let a = render_png(host, &display.text, env.project_dir, page, opts)
                .map_err(|e| pipeline_error(&e, &display.text))?;
            Rendered {
                image: RenderedImage {
                    png: a.png,
                    width: a.width,
                    height: a.height,
                    page,
                    region: None,
                },
                page_count: a.page_count,
                diagnostics: a.diagnostics,
            }
        }
    };
    let Rendered {
        image,
        page_count,
        diagnostics,
    } = out;
    if Diagnostic::has_errors(&diagnostics) {
        return Err(EditorError::new(
            "render.blocked",
            "the render has error diagnostics; fix them and render again",
        )
        .with_diagnostics(&diagnostics, &display.text));
    }
    let mut diagnostics_out = DiagnosticOut::all(&diagnostics, &display.text);
    diagnostics_out.extend(missing_face_notices(&log));
    let mut reply = json!({
        "page": page,
        "page_count": page_count,
        "width": image.width,
        "height": image.height,
        "scale": scale,
        "sha256": image.sha256(),
        "stale": display.stale,
        "diagnostics": diagnostics_out,
    });
    if let (Some(region), Value::Object(map)) = (image.region, &mut reply) {
        insert_region(map, region, image.width, image.height);
    }
    ctx.image = Some(image);
    ctx.session.page = page;
    Ok(reply)
}

/// One render before the error check.
struct Rendered {
    image: RenderedImage,
    page_count: usize,
    diagnostics: Vec<Diagnostic>,
}

/// Add the window fields of a viewport render to a reply: `rect` (device
/// px), `device_size` (the page in device px), `page_size` (the page in page
/// px), and `view` (the page-px rect the PNG covers, `rect / scale`).
pub(crate) fn insert_region(map: &mut Map<String, Value>, r: ImageRegion, width: u32, height: u32) {
    let rect = DeviceRect {
        x: r.x,
        y: r.y,
        width,
        height,
    };
    map.insert(
        "rect".to_owned(),
        json!({"x": rect.x, "y": rect.y, "w": rect.width, "h": rect.height}),
    );
    map.insert(
        "device_size".to_owned(),
        json!({"w": r.device_width, "h": r.device_height}),
    );
    map.insert(
        "page_size".to_owned(),
        json!({"w": r.page_width, "h": r.page_height}),
    );
    map.insert(
        "view".to_owned(),
        json!({
            "x": f64::from(rect.x) / r.scale,
            "y": f64::from(rect.y) / r.scale,
            "w": f64::from(rect.width) / r.scale,
            "h": f64::from(rect.height) / r.scale,
        }),
    );
}

/// Check a whole-page raster scale: finite, `> 0`, and at most the pipeline
/// maximum.
pub(crate) fn scale(value: f64) -> Result<f64, EditorError> {
    check_render_scale(value, &value.to_string()).map_err(|message| {
        EditorError::new(
            "render.invalid_scale",
            format!(
                "{message}; the largest scale is {MAX_RENDER_SCALE}; pass a viewport to go higher"
            ),
        )
    })
}
