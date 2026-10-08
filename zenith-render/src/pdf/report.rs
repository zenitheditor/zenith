//! Strict PDF emission and deterministic raster capture reports.

use super::document::{PdfOptions, assemble};
use crate::RenderError;
use std::{collections::BTreeMap, ops::Range};
use zenith_core::{AssetKind, AssetProvider, FontProvider};
use zenith_scene::{Scene, SceneCommand};

/// Strict PDF export options. Raster scale affects captures only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfExportOptions {
    /// Subset embedded selectable fonts.
    pub subset: bool,
    /// Raster fallback resolution multiplier in the interval (0, 4].
    pub raster_scale: f64,
}
impl Default for PdfExportOptions {
    fn default() -> Self {
        Self {
            subset: true,
            raster_scale: 1.0,
        }
    }
}

/// Deterministic PDF bytes and the command ranges rendered as raster images.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfOutput {
    /// Complete PDF document bytes.
    pub bytes: Vec<u8>,
    /// Raster captures ordered by page and command start.
    pub rasterized_regions: Vec<PdfRasterizedRegion>,
}

/// One complete structural range captured under the page transform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfRasterizedRegion {
    /// One-based document page number.
    pub page: usize,
    /// Zero-based first scene command.
    pub command_start: usize,
    /// Exclusive zero-based command end.
    pub command_end: usize,
    /// Highest-precedence reason requiring this capture.
    pub reason: PdfRasterizationReason,
}

/// Capture reasons in precedence order, except non-normal blending overrides
/// every region with one whole-page capture including the backdrop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PdfRasterizationReason {
    /// Blur, shadow, filter, or mask scopes.
    Effects,
    /// Non-identity group opacity.
    GroupOpacity,
    /// Non-normal blending captures the whole page.
    NonNormalBlend,
    /// Scope kinds close across another open kind.
    CrossedScopes,
    /// Gradient stops or alpha require raster rendering.
    Gradient,
    /// Registered font supplies preferred PNG glyphs.
    BitmapGlyph,
    /// Imported SVG contains unsupported native PDF features.
    SvgAsset,
}

pub(super) struct PlannedRegion {
    pub(super) range: Range<usize>,
    pub(super) reason: PdfRasterizationReason,
}

/// Render one PDF page, reporting captures and rejecting lost resources.
/// Legacy Vec-returning APIs retain compatibility emission after capture errors.
pub fn render_pdf_report(
    scene: &Scene,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    options: PdfOptions,
) -> Result<PdfOutput, RenderError> {
    render_pdf_multi_report(std::slice::from_ref(scene), fonts, assets, options)
}

/// Render ordered PDF pages with strict resource and capture error handling.
pub fn render_pdf_multi_report(
    scenes: &[Scene],
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    options: PdfOptions,
) -> Result<PdfOutput, RenderError> {
    render_pdf_multi_report_with_options(
        scenes,
        fonts,
        assets,
        PdfExportOptions {
            subset: options.subset,
            raster_scale: 1.0,
        },
    )
}

/// Render one PDF page with explicit raster fallback resolution.
pub fn render_pdf_report_with_options(
    scene: &Scene,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    options: PdfExportOptions,
) -> Result<PdfOutput, RenderError> {
    render_pdf_multi_report_with_options(std::slice::from_ref(scene), fonts, assets, options)
}

/// Render ordered PDF pages with explicit raster fallback resolution.
pub fn render_pdf_multi_report_with_options(
    scenes: &[Scene],
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    options: PdfExportOptions,
) -> Result<PdfOutput, RenderError> {
    crate::raster_capture::check_scale(options.raster_scale)?;
    let mut checked_assets = BTreeMap::new();
    let mut plans = Vec::with_capacity(scenes.len());
    let mut rasterized_regions = Vec::new();
    for (index, scene) in scenes.iter().enumerate() {
        let page = index + 1;
        preflight(scene, fonts, assets, &mut checked_assets)
            .map_err(|error| RenderError::new(format!("PDF page {page}: {error}")))?;
        let plan = super::scopes::plan_report(scene, fonts, assets).map_err(|error| {
            RenderError::new(format!(
                "PDF page {page} malformed scopes: {error:?}; balance scene scopes"
            ))
        })?;
        check_capture_assets(scene, &plan, &checked_assets, options.raster_scale)
            .map_err(|error| RenderError::new(format!("PDF page {page}: {error}")))?;
        rasterized_regions.extend(plan.iter().map(|region| PdfRasterizedRegion {
            page,
            command_start: region.range.start,
            command_end: region.range.end,
            reason: region.reason,
        }));
        plans.push(plan);
    }
    Ok(PdfOutput {
        bytes: assemble(
            scenes,
            fonts,
            assets,
            PdfOptions {
                subset: options.subset,
            },
            Some(&plans),
            options.raster_scale,
        )?,
        rasterized_regions,
    })
}

fn preflight(
    scene: &Scene,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    checked_assets: &mut CheckedAssets,
) -> Result<(), RenderError> {
    if !scene.width.is_finite()
        || !scene.height.is_finite()
        || scene.width <= 0.0
        || scene.height <= 0.0
        || scene.width > f64::from(f32::MAX)
        || scene.height > f64::from(f32::MAX)
    {
        return Err(RenderError::new(format!(
            "invalid dimensions {}x{}; supply finite positive PDF dimensions",
            scene.width, scene.height
        )));
    }
    for (index, command) in scene.commands.iter().enumerate() {
        let result = match command {
            SceneCommand::DrawSvgAsset { asset, .. } => Err(RenderError::new(format!(
                "unsupported DrawSvgAsset {asset}; supply DrawImage"
            ))),
            SceneCommand::DrawImage {
                asset_id,
                svg_style,
                ..
            } => check_asset(asset_id, *svg_style, fonts, assets, checked_assets),
            SceneCommand::DrawGlyphRun {
                font_id,
                glyphs,
                font_size,
                ..
            } => check_glyphs(font_id, glyphs, *font_size, fonts),
            SceneCommand::FillRect { .. }
            | SceneCommand::FillRoundedRect { .. }
            | SceneCommand::FillEllipse { .. }
            | SceneCommand::FillPolygon { .. }
            | SceneCommand::FillPath { .. }
            | SceneCommand::StrokeRect { .. }
            | SceneCommand::StrokeRoundedRect { .. }
            | SceneCommand::StrokeEllipse { .. }
            | SceneCommand::StrokeLine { .. }
            | SceneCommand::StrokePolyline { .. }
            | SceneCommand::StrokePath { .. }
            | SceneCommand::PushClip { .. }
            | SceneCommand::PushClipRoundedRect { .. }
            | SceneCommand::PopClip
            | SceneCommand::PushLayer { .. }
            | SceneCommand::PopLayer
            | SceneCommand::PushTransform { .. }
            | SceneCommand::PushScaleTranslate { .. }
            | SceneCommand::PushTransformMatrix { .. }
            | SceneCommand::PopTransform
            | SceneCommand::BeginShadow { .. }
            | SceneCommand::EndShadow
            | SceneCommand::BeginBlur { .. }
            | SceneCommand::EndBlur
            | SceneCommand::BeginFilter { .. }
            | SceneCommand::EndFilter
            | SceneCommand::BeginMask { .. }
            | SceneCommand::EndMask => Ok(()),
        };
        result.map_err(|error| RenderError::new(format!("command {index}: {error}")))?;
    }
    Ok(())
}

fn check_asset(
    id: &str,
    style: Option<zenith_scene::SvgStyle>,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    checked_assets: &mut CheckedAssets,
) -> Result<(), RenderError> {
    let key = asset_key(id, style);
    if checked_assets.contains_key(&key) {
        return Ok(());
    }
    let asset = assets
        .by_id(id)
        .ok_or_else(|| RenderError::new(format!("unresolved asset {id}; register its bytes")))?;
    let intrinsic = match asset.kind {
        AssetKind::Image => {
            if crate::tiny_skia::decode_raster_to_pixmap(&asset.bytes).is_none() {
                return Err(RenderError::new(format!(
                    "invalid raster asset {id}; supply supported image bytes"
                )));
            }
            None
        }
        AssetKind::Svg => {
            let tree = crate::svg::checked_svg(&asset.bytes, style, fonts).map_err(|error| {
                RenderError::new(format!("SVG asset {id}: {error}; correct asset resources"))
            })?;
            Some((f64::from(tree.size.width()), f64::from(tree.size.height())))
        }
        AssetKind::Font | AssetKind::Unknown(_) => {
            return Err(RenderError::new(format!(
                "unsupported asset kind for {id}; supply image or SVG bytes"
            )));
        }
    };
    checked_assets.insert(key, intrinsic);
    Ok(())
}

fn check_glyphs(
    id: &str,
    glyphs: &[zenith_scene::SceneGlyph],
    size: f32,
    fonts: &dyn FontProvider,
) -> Result<(), RenderError> {
    use crate::glyph_bitmap::{GlyphRepresentation, representation};
    let font = fonts
        .by_id(id)
        .ok_or_else(|| RenderError::new(format!("unresolved font {id}; register font bytes")))?;
    let face = ttf_parser::Face::parse(&font.bytes, font.index).map_err(|error| {
        RenderError::new(format!(
            "invalid font {id}: {error:?}; register valid font bytes"
        ))
    })?;
    for glyph in glyphs {
        if glyph.glyph_id >= face.number_of_glyphs() {
            return Err(RenderError::new(format!(
                "font {id} lacks glyph {}; supply a supported glyph",
                glyph.glyph_id
            )));
        }
        match representation(&face, ttf_parser::GlyphId(glyph.glyph_id), size) {
            GlyphRepresentation::Png
            | GlyphRepresentation::Outline
            | GlyphRepresentation::Empty => {}
            GlyphRepresentation::UnsupportedBitmap | GlyphRepresentation::UnsupportedGlyph => {
                return Err(RenderError::new(format!(
                    "font {id} glyph {} lacks supported ink; supply outline or PNG glyphs",
                    glyph.glyph_id
                )));
            }
        }
    }
    Ok(())
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ColorKey([u8; 4], Option<[u32; 4]>);
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct StyleKey {
    stroke: Option<ColorKey>,
    fill: Option<ColorKey>,
    width: Option<u64>,
}
type AssetKey = (String, Option<StyleKey>);
type CheckedAssets = BTreeMap<AssetKey, Option<(f64, f64)>>;

fn asset_key(id: &str, style: Option<zenith_scene::SvgStyle>) -> AssetKey {
    let color = |value: zenith_scene::Color| {
        ColorKey(
            [value.r, value.g, value.b, value.a],
            value.cmyk.map(|channels| channels.map(f32::to_bits)),
        )
    };
    (
        id.to_owned(),
        style.map(|style| StyleKey {
            stroke: style.stroke.map(color),
            fill: style.fill.map(color),
            width: style.stroke_width.map(f64::to_bits),
        }),
    )
}

fn check_capture_assets(
    scene: &Scene,
    plan: &[PlannedRegion],
    checked: &CheckedAssets,
    device_scale: f64,
) -> Result<(), RenderError> {
    for region in plan {
        for (index, command) in scene
            .commands
            .iter()
            .enumerate()
            .skip(region.range.start)
            .take(region.range.len())
        {
            if let SceneCommand::DrawImage {
                asset_id,
                svg_style,
                w,
                h,
                ..
            } = command
                && let Some(Some((svw, svh))) = checked.get(&asset_key(asset_id, *svg_style))
            {
                crate::raster_capture::check_svg_size((*svw, *svh), (*w, *h), device_scale)
                    .map_err(|error| {
                        RenderError::new(format!(
                            "commands {}..{} capture command {index} SVG asset {asset_id}: {error}",
                            region.range.start, region.range.end
                        ))
                    })?;
            }
        }
    }
    Ok(())
}
