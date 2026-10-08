use super::geometry::finite;
use crate::RenderError;
use zenith_scene::{FilterSpec, SceneCommand};

pub(super) fn check(command: &SceneCommand) -> Result<(), RenderError> {
    match command {
        SceneCommand::BeginBlur { radius } => {
            finite(&[*radius])?;
            if *radius < 0.0 {
                return Err(RenderError::new(format!(
                    "SVG blur radius must be nonnegative: {radius}"
                )));
            }
        }
        SceneCommand::BeginShadow { shadows } => {
            for shadow in shadows {
                finite(&[shadow.dx, shadow.dy, shadow.blur])?;
                if shadow.blur < 0.0 {
                    return Err(RenderError::new(format!(
                        "SVG shadow blur must be nonnegative: {}",
                        shadow.blur
                    )));
                }
            }
        }
        SceneCommand::BeginFilter { filters } => {
            for filter in filters {
                check_filter(*filter)?;
            }
        }
        SceneCommand::BeginMask { mask } => {
            finite(&[mask.x, mask.y, mask.w, mask.h, mask.radius, mask.feather])?;
            if mask.feather < 0.0 {
                return Err(RenderError::new(format!(
                    "SVG mask feather must be nonnegative: {}",
                    mask.feather
                )));
            }
        }
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
        | SceneCommand::PushLayer { .. }
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
    Ok(())
}

fn check_filter(filter: FilterSpec) -> Result<(), RenderError> {
    match filter {
        FilterSpec::Grayscale(amount)
        | FilterSpec::Invert(amount)
        | FilterSpec::Sepia(amount)
        | FilterSpec::Saturate(amount)
        | FilterSpec::Brightness(amount)
        | FilterSpec::Contrast(amount)
        | FilterSpec::HueRotate(amount)
        | FilterSpec::Duotone { amount, .. } => finite(&[amount]),
        FilterSpec::Noise { amount, scale, .. } => finite(&[amount, scale]),
    }
}
