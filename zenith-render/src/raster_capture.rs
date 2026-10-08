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
