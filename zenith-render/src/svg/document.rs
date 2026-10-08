use zenith_core::{AssetProvider, FontProvider};
use zenith_scene::Scene;

use super::{assets, scopes, writer::Writer};
use crate::RenderError;

/// SVG export options. Raster scale affects fallback images only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SvgOptions {
    /// Raster fallback resolution multiplier in the interval (0, 4].
    pub raster_scale: f64,
}
impl Default for SvgOptions {
    fn default() -> Self {
        Self { raster_scale: 1.0 }
    }
}

/// Why an SVG region contains embedded raster pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SvgRasterizationReason {
    /// Raster effects require the original compositing pipeline.
    Effects,
    /// A blend mode requires the complete page backdrop.
    NonNormalBlend,
    /// Independent scene stacks cross instead of nesting.
    CrossedScopes,
    /// A font supplies a preferred bitmap glyph.
    BitmapGlyph,
}

/// A rasterized command range. The end index is exclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgRasterizedRegion {
    pub command_start: usize,
    pub command_end: usize,
    pub reason: SvgRasterizationReason,
}

/// Self-contained SVG bytes and ordered raster fallback reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgOutput {
    pub bytes: Vec<u8>,
    pub rasterized_regions: Vec<SvgRasterizedRegion>,
}

/// Export a scene as self-contained SVG bytes.
///
/// # Errors
/// Returns an error for malformed scenes, unresolved resources, or raster encoding errors.
pub fn render_svg(
    scene: &Scene,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
) -> Result<Vec<u8>, RenderError> {
    Ok(render_svg_with(scene, fonts, assets)?.bytes)
}

/// Export SVG with explicit reports for rasterized command ranges.
///
/// Ordinary geometry and outlined text remain vector. Raster effects preserve their complete structural scope.
/// Non-normal blends rasterize the complete page to preserve backdrop compositing.
///
/// # Errors
/// Returns an error for malformed scenes, unresolved resources, or raster encoding errors.
pub fn render_svg_with(
    scene: &Scene,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
) -> Result<SvgOutput, RenderError> {
    render_svg_with_options(scene, fonts, assets, SvgOptions::default())
}

/// Export SVG with raster reports and explicit fallback resolution.
pub fn render_svg_with_options(
    scene: &Scene,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    options: SvgOptions,
) -> Result<SvgOutput, RenderError> {
    crate::raster_capture::check_scale(options.raster_scale)?;
    if !scene.width.is_finite()
        || !scene.height.is_finite()
        || scene.width <= 0.0
        || scene.height <= 0.0
    {
        return Err(RenderError::new(format!(
            "invalid SVG dimensions {} × {}",
            scene.width, scene.height
        )));
    }
    let regions = scopes::plan(scene, fonts)?;
    if !regions.is_empty() {
        crate::raster_capture::check_capture((scene.width, scene.height), options.raster_scale)?;
    }
    let mut writer = Writer::new(scene.width, scene.height);
    let mut index = 0;
    for region in &regions {
        for command in scene
            .commands
            .get(index..region.command_start)
            .ok_or_else(|| RenderError::new("invalid SVG command range"))?
        {
            writer.command(command, fonts, assets)?;
        }
        let mut part = Scene::new(scene.width, scene.height);
        part.commands = scene
            .commands
            .get(region.command_start..region.command_end)
            .ok_or_else(|| RenderError::new("invalid SVG raster range"))?
            .to_vec();
        crate::raster_capture::check_transforms(
            &part.commands,
            region.command_start,
            options.raster_scale,
            "SVG",
        )?;
        // The PNG backend skips unresolved resources. Check each fallback before rasterization.
        for (offset, command) in part.commands.iter().enumerate() {
            if let Some(capture) = writer.check_command(command, fonts, assets)? {
                crate::raster_capture::check_svg_size(
                    capture.intrinsic,
                    capture.destination,
                    options.raster_scale,
                )
                .map_err(|error| {
                    RenderError::new(format!(
                        "SVG commands {}..{} capture command {} asset {}: {error}",
                        region.command_start,
                        region.command_end,
                        region.command_start + offset,
                        capture.asset_id
                    ))
                })?;
            }
        }
        for command in &mut part.commands {
            if let zenith_scene::SceneCommand::DrawSvgAsset { x, y, w, h, asset } = command {
                *command = zenith_scene::SceneCommand::DrawImage {
                    x: *x,
                    y: *y,
                    w: *w,
                    h: *h,
                    asset_id: std::mem::take(asset),
                    fit: zenith_scene::FitMode::Stretch,
                    pos_x: 0.0,
                    pos_y: 0.0,
                    opacity: 1.0,
                    clip_shape: None,
                    src_rect: None,
                    svg_style: None,
                };
            }
        }
        let image = crate::render_image_scaled(&part, options.raster_scale, fonts, assets)?;
        drop(part);
        let width = f64::from(image.width) / options.raster_scale;
        let height = f64::from(image.height) / options.raster_scale;
        if !width.is_finite() || !height.is_finite() {
            return Err(RenderError::new(format!(
                "SVG raster capture placement exceeds supported coordinates at scale {}; increase raster capture scale",
                options.raster_scale
            )));
        }
        let png = crate::encode_png(&image)?;
        writer.body.push_str(&assets::image_element(
            &png,
            "image/png",
            (0.0, 0.0, width, height),
            1.0,
        ));
        index = region.command_end;
    }
    for command in scene
        .commands
        .get(index..)
        .ok_or_else(|| RenderError::new("invalid SVG command range"))?
    {
        writer.command(command, fonts, assets)?;
    }
    Ok(SvgOutput {
        bytes: writer.finish().into_bytes(),
        rasterized_regions: regions,
    })
}
