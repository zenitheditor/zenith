//! Complete raster ranges under the page transform.

use super::report::{PdfRasterizationReason, PlannedRegion};
use crate::scopes::{ScopeError, ScopeTracker};
use zenith_core::{AssetProvider, FontProvider};
use zenith_scene::{BlendMode, Scene, SceneCommand};

pub(super) fn plan(
    scene: &Scene,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
) -> Result<Vec<std::ops::Range<usize>>, ScopeError> {
    Ok(plan_report(scene, fonts, assets)?
        .into_iter()
        .map(|region| region.range)
        .collect())
}

pub(super) fn plan_report(
    scene: &Scene,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
) -> Result<Vec<PlannedRegion>, ScopeError> {
    let mut capabilities = super::svg_capability::SvgCapabilities::default();
    let mut tracker = ScopeTracker::default();
    let mut reason = None;
    let mut blend = false;
    let mut ranges = Vec::new();
    for (index, command) in scene.commands.iter().enumerate() {
        match command {
            SceneCommand::PushLayer {
                opacity,
                blend_mode,
            } => {
                select(
                    &mut reason,
                    *opacity != 1.0,
                    PdfRasterizationReason::GroupOpacity,
                );
                blend |= blend_mode.is_some_and(|mode| mode != BlendMode::Normal);
            }
            SceneCommand::BeginBlur { .. }
            | SceneCommand::BeginShadow { .. }
            | SceneCommand::BeginMask { .. } => {
                select(&mut reason, true, PdfRasterizationReason::Effects)
            }
            SceneCommand::BeginFilter { filters } => select(
                &mut reason,
                !filters.is_empty(),
                PdfRasterizationReason::Effects,
            ),
            SceneCommand::FillRect { paint, .. }
            | SceneCommand::FillRoundedRect { paint, .. }
            | SceneCommand::FillEllipse { paint, .. }
            | SceneCommand::FillPolygon { paint, .. }
            | SceneCommand::FillPath { paint, .. } => match paint {
                zenith_scene::Paint::Solid { .. } => {}
                zenith_scene::Paint::Gradient(gradient) => select(
                    &mut reason,
                    super::gradient::requires_raster(gradient),
                    PdfRasterizationReason::Gradient,
                ),
            },
            SceneCommand::DrawGlyphRun { .. } => select(
                &mut reason,
                crate::glyph_bitmap::preferred_png(command, fonts),
                PdfRasterizationReason::BitmapGlyph,
            ),
            SceneCommand::DrawImage { .. } => {
                select(
                    &mut reason,
                    capabilities.requires_raster(command, fonts, assets),
                    PdfRasterizationReason::SvgAsset,
                );
            }
            SceneCommand::StrokeRect { .. }
            | SceneCommand::StrokeRoundedRect { .. }
            | SceneCommand::StrokeEllipse { .. }
            | SceneCommand::StrokeLine { .. }
            | SceneCommand::StrokePolyline { .. }
            | SceneCommand::StrokePath { .. }
            | SceneCommand::DrawSvgAsset { .. }
            | SceneCommand::PushClip { .. }
            | SceneCommand::PushClipRoundedRect { .. }
            | SceneCommand::PopClip
            | SceneCommand::PopLayer
            | SceneCommand::PushTransform { .. }
            | SceneCommand::PushScaleTranslate { .. }
            | SceneCommand::PushTransformMatrix { .. }
            | SceneCommand::PopTransform
            | SceneCommand::EndShadow
            | SceneCommand::EndBlur
            | SceneCommand::EndFilter
            | SceneCommand::EndMask => {}
        }
        let step = tracker.advance(index, command)?;
        select(
            &mut reason,
            step.crossed_now,
            PdfRasterizationReason::CrossedScopes,
        );
        if let Some(range) = step.completed {
            if let Some(reason) = reason {
                ranges.push(PlannedRegion { range, reason });
            }
            reason = None;
        }
    }
    tracker.finish()?;
    if blend {
        Ok(std::iter::once(PlannedRegion {
            range: 0..scene.commands.len(),
            reason: PdfRasterizationReason::NonNormalBlend,
        })
        .collect())
    } else {
        Ok(ranges)
    }
}

fn select(
    reason: &mut Option<PdfRasterizationReason>,
    selected: bool,
    candidate: PdfRasterizationReason,
) {
    if selected && reason.is_none_or(|current| candidate < current) {
        *reason = Some(candidate);
    }
}
