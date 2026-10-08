use super::document::{SvgRasterizationReason as Reason, SvgRasterizedRegion};
use crate::RenderError;
use crate::scopes::{ScopeError, ScopeTracker};
use zenith_core::FontProvider;
use zenith_scene::{BlendMode, Scene, SceneCommand};

pub(super) fn plan(
    scene: &Scene,
    fonts: &dyn FontProvider,
) -> Result<Vec<SvgRasterizedRegion>, RenderError> {
    let mut tracker = ScopeTracker::default();
    let mut regions = Vec::new();
    let mut reason = None;
    let mut blend = false;
    for (index, command) in scene.commands.iter().enumerate() {
        match command {
            SceneCommand::PushLayer {
                blend_mode: Some(mode),
                ..
            } if *mode != BlendMode::Normal => blend = true,
            SceneCommand::BeginShadow { shadows } if !shadows.is_empty() => {
                reason = Some(Reason::Effects)
            }
            SceneCommand::BeginBlur { radius } if *radius > 0.0 => reason = Some(Reason::Effects),
            SceneCommand::BeginFilter { filters } if !filters.is_empty() => {
                reason = Some(Reason::Effects)
            }
            SceneCommand::BeginMask { .. } => reason = Some(Reason::Effects),
            SceneCommand::DrawGlyphRun { .. } => {
                if super::text::bitmap_glyphs(command, fonts)? && reason.is_none() {
                    reason = Some(Reason::BitmapGlyph);
                }
            }
            SceneCommand::PushLayer { .. }
            | SceneCommand::BeginShadow { .. }
            | SceneCommand::BeginBlur { .. }
            | SceneCommand::BeginFilter { .. }
            | SceneCommand::FillRect { .. }
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
        let step = tracker.advance(index, command).map_err(scope_error)?;
        if step.crossed_now && reason.is_none() {
            reason = Some(Reason::CrossedScopes);
        }
        if let Some(range) = step.completed
            && let Some(reason) = reason.take()
        {
            regions.push(SvgRasterizedRegion {
                command_start: range.start,
                command_end: range.end,
                reason,
            });
        }
    }
    tracker.finish().map_err(scope_error)?;
    if blend {
        return Ok(vec![SvgRasterizedRegion {
            command_start: 0,
            command_end: scene.commands.len(),
            reason: Reason::NonNormalBlend,
        }]);
    }
    Ok(regions)
}

fn scope_error(error: ScopeError) -> RenderError {
    match error {
        ScopeError::DepthOverflow => RenderError::new("SVG scope depth overflow"),
        ScopeError::UnmatchedClose { index } => {
            RenderError::new(format!("unmatched SVG scope close at command {index}"))
        }
        ScopeError::Unfinished => RenderError::new("unfinished SVG structural scope"),
    }
}
