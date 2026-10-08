use super::document::{SvgRasterizationReason as Reason, SvgRasterizedRegion};
use crate::RenderError;
use std::collections::BTreeMap;
use zenith_core::FontProvider;
use zenith_scene::{BlendMode, Scene, SceneCommand};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Scope {
    Clip,
    Layer,
    Transform,
    Shadow,
    Blur,
    Filter,
    Mask,
}

enum Action {
    Open(Scope),
    Close(Scope),
    Draw,
}

fn action(command: &SceneCommand) -> Action {
    match command {
        SceneCommand::PushClip { .. } | SceneCommand::PushClipRoundedRect { .. } => {
            Action::Open(Scope::Clip)
        }
        SceneCommand::PopClip => Action::Close(Scope::Clip),
        SceneCommand::PushLayer { .. } => Action::Open(Scope::Layer),
        SceneCommand::PopLayer => Action::Close(Scope::Layer),
        SceneCommand::PushTransform { .. }
        | SceneCommand::PushScaleTranslate { .. }
        | SceneCommand::PushTransformMatrix { .. } => Action::Open(Scope::Transform),
        SceneCommand::PopTransform => Action::Close(Scope::Transform),
        SceneCommand::BeginShadow { .. } => Action::Open(Scope::Shadow),
        SceneCommand::EndShadow => Action::Close(Scope::Shadow),
        SceneCommand::BeginBlur { .. } => Action::Open(Scope::Blur),
        SceneCommand::EndBlur => Action::Close(Scope::Blur),
        SceneCommand::BeginFilter { .. } => Action::Open(Scope::Filter),
        SceneCommand::EndFilter => Action::Close(Scope::Filter),
        SceneCommand::BeginMask { .. } => Action::Open(Scope::Mask),
        SceneCommand::EndMask => Action::Close(Scope::Mask),
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
        | SceneCommand::DrawGlyphRun { .. } => Action::Draw,
    }
}

pub(super) fn plan(
    scene: &Scene,
    fonts: &dyn FontProvider,
) -> Result<Vec<SvgRasterizedRegion>, RenderError> {
    let mut stack = Vec::new();
    let mut counts: BTreeMap<Scope, usize> = BTreeMap::new();
    let mut crossed = false;
    let mut regions = Vec::new();
    let mut start = 0;
    let mut reason = None;
    let mut blend = false;
    for (index, command) in scene.commands.iter().enumerate() {
        if counts.is_empty() {
            start = index;
        }
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
        match action(command) {
            Action::Open(kind) => {
                let count = counts.entry(kind).or_default();
                *count = count
                    .checked_add(1)
                    .ok_or_else(|| RenderError::new("SVG scope depth overflow"))?;
                if !crossed {
                    stack.push(kind);
                }
            }
            Action::Close(kind) => {
                let count = counts.get_mut(&kind).ok_or_else(|| {
                    RenderError::new(format!("unmatched SVG scope close at command {index}"))
                })?;
                *count = count.checked_sub(1).ok_or_else(|| {
                    RenderError::new(format!("unmatched SVG scope close at command {index}"))
                })?;
                if *count == 0 {
                    counts.remove(&kind);
                }
                if !crossed {
                    if stack.last() == Some(&kind) {
                        stack.pop();
                    } else {
                        crossed = true;
                        stack.clear();
                        if reason.is_none() {
                            reason = Some(Reason::CrossedScopes);
                        }
                    }
                }
            }
            Action::Draw => {}
        }
        if counts.is_empty() {
            crossed = false;
        }
        if counts.is_empty()
            && let Some(reason) = reason.take()
        {
            regions.push(SvgRasterizedRegion {
                command_start: start,
                command_end: index + 1,
                reason,
            });
        }
    }
    if !counts.is_empty() {
        return Err(RenderError::new("unfinished SVG structural scope"));
    }
    if blend {
        return Ok(vec![SvgRasterizedRegion {
            command_start: 0,
            command_end: scene.commands.len(),
            reason: Reason::NonNormalBlend,
        }]);
    }
    Ok(regions)
}
