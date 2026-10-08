//! Shared vector-export capture scale and SVG intermediate checks.
use crate::RenderError;

pub(crate) fn check_scale(scale: f64) -> Result<(), RenderError> {
    if !scale.is_finite() || scale <= 0.0 || scale > 4.0 {
        return Err(RenderError::new(format!(
            "invalid raster capture scale {scale}; supply a finite scale greater than 0 and at most 4"
        )));
    }
    Ok(())
}

/// Check actual capture dimensions and inverse placement before raster allocation.
pub(crate) fn check_capture(page: (f64, f64), scale: f64) -> Result<(u32, u32), RenderError> {
    let device_scale = scale as f32;
    if !device_scale.is_finite() || device_scale <= 0.0 {
        return Err(RenderError::new(format!(
            "Raster capture placement scale error: scale={scale}, f32={device_scale}. Increase capture scale to a supported positive value"
        )));
    }
    let dimensions = crate::scaled_size(page.0, page.1, scale)?;
    for (axis, value) in [
        ("width", f64::from(dimensions.0) / scale),
        ("height", f64::from(dimensions.1) / scale),
    ] {
        if !value.is_finite() || value > f64::from(f32::MAX) {
            return Err(RenderError::new(format!(
                "Raster capture placement error: {axis}={value}, scale={scale}. Increase capture scale to supported coordinates"
            )));
        }
    }
    Ok(dimensions)
}

/// Captured transforms use tiny-skia's narrowed matrix arithmetic.
pub(crate) fn check_transforms(
    commands: &[zenith_scene::SceneCommand],
    start: usize,
    scale: f64,
    context: &str,
) -> Result<(), RenderError> {
    use zenith_scene::SceneCommand;
    let mut current = tiny_skia::Transform::from_scale(scale as f32, scale as f32);
    let mut stack = Vec::new();
    for (offset, command) in commands.iter().enumerate() {
        let matrix = match command {
            SceneCommand::PushTransform { angle_deg, cx, cy } => Some(
                tiny_skia::Transform::from_rotate_at(*angle_deg as f32, *cx as f32, *cy as f32),
            ),
            SceneCommand::PushScaleTranslate { sx, sy, tx, ty } => {
                Some(tiny_skia::Transform::from_row(
                    *sx as f32, 0.0, 0.0, *sy as f32, *tx as f32, *ty as f32,
                ))
            }
            SceneCommand::PushTransformMatrix { a, b, c, d, e, f } => {
                Some(tiny_skia::Transform::from_row(
                    *a as f32, *b as f32, *c as f32, *d as f32, *e as f32, *f as f32,
                ))
            }
            SceneCommand::PopTransform => {
                if let Some(previous) = stack.pop() {
                    current = previous;
                }
                None
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
            | SceneCommand::BeginShadow { .. }
            | SceneCommand::EndShadow
            | SceneCommand::BeginBlur { .. }
            | SceneCommand::EndBlur
            | SceneCommand::BeginFilter { .. }
            | SceneCommand::EndFilter
            | SceneCommand::BeginMask { .. }
            | SceneCommand::EndMask => None,
        };
        if let Some(matrix) = matrix {
            let composed = current.pre_concat(matrix);
            for (field, value) in [
                ("a", composed.sx),
                ("b", composed.ky),
                ("c", composed.kx),
                ("d", composed.sy),
                ("e", composed.tx),
                ("f", composed.ty),
            ] {
                if !value.is_finite() {
                    return Err(RenderError::new(format!(
                        "{context} command {} raster transform error: matrix.{field}={value}. Supply supported capture transform operands",
                        start + offset
                    )));
                }
            }
            stack.push(current);
            current = composed;
        }
    }
    Ok(())
}

pub(crate) fn check_svg_size(
    intrinsic: (f64, f64),
    destination: (f64, f64),
    device_scale: f64,
) -> Result<(), RenderError> {
    let (svw, svh) = intrinsic;
    let (w, h) = destination;
    let scale = ((w / svw).max(h / svh) * device_scale).clamp(0.01, 16.0);
    let width = (svw * scale).ceil();
    let height = (svh * scale).ceil();
    // Match tiny-skia's dimensions and row/storage arithmetic without allocating a pixmap.
    let supported = || {
        if !width.is_finite()
            || !height.is_finite()
            || width < 1.0
            || height < 1.0
            || width > f64::from(u32::MAX)
            || height > f64::from(u32::MAX)
        {
            return None;
        }
        let row = i32::try_from(width as u32).ok()?.checked_mul(4)?;
        (row as usize)
            .checked_mul(height as usize)
            .filter(|bytes| *bytes <= isize::MAX as usize)
    };
    if supported().is_none() {
        return Err(RenderError::new(format!(
            "unsupported intermediate dimensions {width}x{height}; reduce SVG intrinsic dimensions"
        )));
    }
    Ok(())
}
