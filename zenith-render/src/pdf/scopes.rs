//! Complete raster ranges under the page transform.

use crate::scopes::{ScopeError, ScopeTracker};
use std::ops::Range;
use zenith_core::FontProvider;
use zenith_scene::{BlendMode, Scene, SceneCommand};

pub(super) fn plan(
    scene: &Scene,
    fonts: &dyn FontProvider,
) -> Result<Vec<Range<usize>>, ScopeError> {
    let mut tracker = ScopeTracker::default();
    let mut selected = false;
    let mut blend = false;
    let mut ranges = Vec::new();
    for (index, command) in scene.commands.iter().enumerate() {
        match command {
            SceneCommand::PushLayer {
                opacity,
                blend_mode,
            } => {
                selected |= *opacity != 1.0;
                blend |= blend_mode.is_some_and(|mode| mode != BlendMode::Normal);
            }
            SceneCommand::BeginBlur { .. }
            | SceneCommand::BeginShadow { .. }
            | SceneCommand::BeginMask { .. } => selected = true,
            SceneCommand::BeginFilter { filters } => selected |= !filters.is_empty(),
            SceneCommand::FillRect { paint, .. }
            | SceneCommand::FillRoundedRect { paint, .. }
            | SceneCommand::FillEllipse { paint, .. }
            | SceneCommand::FillPolygon { paint, .. }
            | SceneCommand::FillPath { paint, .. } => match paint {
                zenith_scene::Paint::Solid { .. } => {}
                zenith_scene::Paint::Gradient(gradient) => {
                    selected |= super::gradient::requires_raster(gradient)
                }
            },
            SceneCommand::DrawGlyphRun { .. } => {
                selected |= crate::glyph_bitmap::preferred_png(command, fonts)
            }
            SceneCommand::StrokeRect { .. }
            | SceneCommand::StrokeRoundedRect { .. }
            | SceneCommand::StrokeEllipse { .. }
            | SceneCommand::StrokeLine { .. }
            | SceneCommand::StrokePolyline { .. }
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
        let step = tracker.advance(index, command)?;
        selected |= step.crossed_now;
        if let Some(range) = step.completed {
            if selected {
                ranges.push(range);
            }
            selected = false;
        }
    }
    tracker.finish()?;
    if blend {
        Ok(std::iter::once(0..scene.commands.len()).collect())
    } else {
        Ok(ranges)
    }
}
