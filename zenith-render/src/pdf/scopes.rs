//! Complete raster ranges under the page transform.

use crate::scopes::{ScopeError, ScopeTracker};
use std::ops::Range;
use zenith_scene::{BlendMode, Scene, SceneCommand};

pub(super) fn plan(scene: &Scene) -> Result<Vec<Range<usize>>, ScopeError> {
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
