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
    let mut outer_clips: usize = 0;
    let mut tracker = ScopeTracker::default();
    let mut in_non_clip = false;
    let mut reason = None;
    let mut blend = false;
    let mut ranges = Vec::new();
    for (index, command) in scene.commands.iter().enumerate() {
        let cmd_reason = command_reason(command, &mut capabilities, fonts, assets, &mut blend);
        if !in_non_clip {
            match command {
                SceneCommand::PushClip { .. } | SceneCommand::PushClipRoundedRect { .. } => {
                    outer_clips = outer_clips
                        .checked_add(1)
                        .ok_or(ScopeError::DepthOverflow)?;
                    continue;
                }
                SceneCommand::PopClip => {
                    outer_clips = outer_clips
                        .checked_sub(1)
                        .ok_or(ScopeError::UnmatchedClose { index })?;
                    continue;
                }
                _ if is_draw(command) => {
                    if let Some(r) = cmd_reason {
                        ranges.push(PlannedRegion {
                            range: index..index.checked_add(1).ok_or(ScopeError::DepthOverflow)?,
                            reason: r,
                        });
                    }
                    continue;
                }
                _ => {
                    in_non_clip = true;
                }
            }
        }

        if let Some(r) = cmd_reason {
            select(&mut reason, true, r);
        }
        let step = match tracker.advance(index, command) {
            Ok(step) => step,
            Err(ScopeError::UnmatchedClose { .. }) if outer_clips > 0 => {
                // A PopClip closed an outer clip while a non-clip scope was open (crossed scopes).
                // Fall back to the full-scene scope tracker so crossed scopes are captured intact.
                return plan_report_full(scene, fonts, assets);
            }
            Err(err) => return Err(err),
        };
        select(
            &mut reason,
            step.crossed_now,
            PdfRasterizationReason::CrossedScopes,
        );
        if let Some(range) = step.completed {
            if let Some(r) = reason {
                ranges.push(PlannedRegion { range, reason: r });
            }
            reason = None;
            in_non_clip = false;
        }
    }
    tracker.finish()?;
    if outer_clips != 0 {
        return Err(ScopeError::Unfinished);
    }
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

fn plan_report_full(
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
        if let Some(r) = command_reason(command, &mut capabilities, fonts, assets, &mut blend) {
            select(&mut reason, true, r);
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

fn is_draw(command: &SceneCommand) -> bool {
    matches!(
        command,
        SceneCommand::FillRect { .. }
            | SceneCommand::StrokeRect { .. }
            | SceneCommand::FillRoundedRect { .. }
            | SceneCommand::StrokeRoundedRect { .. }
            | SceneCommand::FillEllipse { .. }
            | SceneCommand::StrokeEllipse { .. }
            | SceneCommand::StrokeLine { .. }
            | SceneCommand::FillPolygon { .. }
            | SceneCommand::StrokePolyline { .. }
            | SceneCommand::FillPath { .. }
            | SceneCommand::StrokePath { .. }
            | SceneCommand::DrawImage { .. }
            | SceneCommand::DrawSvgAsset { .. }
            | SceneCommand::DrawGlyphRun { .. }
    )
}

fn command_reason(
    command: &SceneCommand,
    capabilities: &mut super::svg_capability::SvgCapabilities,
    fonts: &dyn FontProvider,
    assets: &dyn AssetProvider,
    blend: &mut bool,
) -> Option<PdfRasterizationReason> {
    match command {
        SceneCommand::PushLayer {
            opacity,
            blend_mode,
        } => {
            *blend |= blend_mode.is_some_and(|mode| mode != BlendMode::Normal);
            (*opacity != 1.0).then_some(PdfRasterizationReason::GroupOpacity)
        }
        SceneCommand::BeginBlur { .. }
        | SceneCommand::BeginShadow { .. }
        | SceneCommand::BeginMask { .. } => Some(PdfRasterizationReason::Effects),
        SceneCommand::BeginFilter { filters } => {
            (!filters.is_empty()).then_some(PdfRasterizationReason::Effects)
        }
        SceneCommand::FillRect { paint, .. }
        | SceneCommand::FillRoundedRect { paint, .. }
        | SceneCommand::FillEllipse { paint, .. }
        | SceneCommand::FillPolygon { paint, .. }
        | SceneCommand::FillPath { paint, .. } => match paint {
            zenith_scene::Paint::Solid { .. } => None,
            zenith_scene::Paint::Gradient(gradient) => super::gradient::requires_raster(gradient)
                .then_some(PdfRasterizationReason::Gradient),
        },
        SceneCommand::DrawGlyphRun { .. } => crate::glyph_bitmap::preferred_png(command, fonts)
            .then_some(PdfRasterizationReason::BitmapGlyph),
        SceneCommand::DrawImage { .. } => capabilities
            .requires_raster(command, fonts, assets)
            .then_some(PdfRasterizationReason::SvgAsset),
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
        | SceneCommand::EndMask => None,
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
