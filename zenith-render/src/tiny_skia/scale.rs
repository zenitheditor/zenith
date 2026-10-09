//! Raster-time output scale: device dimensions and effect radii.
//!
//! A scaled render draws the same scene under a root `scale × scale`
//! transform. Geometry, strokes, glyphs, clips, and images follow that
//! transform. Effects that run in device pixels after capture (shadow offset
//! and blur, Gaussian blur, mask box and feather, noise cell size) are
//! multiplied by `scale` here, so a scaled render matches the full render
//! scaled down. At `scale = 1.0` every multiplication is exact, so the output
//! is byte-identical to an unscaled render.

use zenith_scene::{FilterSpec, MaskSpec, ShadowSpec};

use super::pixels::f64_to_px;
use crate::error::RenderError;

/// Check that `scale` is a usable output scale: finite and `> 0`.
pub(crate) fn check_scale(scale: f64) -> Result<(), RenderError> {
    if scale.is_finite() && scale > 0.0 {
        return Ok(());
    }
    Err(RenderError::new(format!(
        "raster scale {scale} is invalid; pass a finite scale greater than 0"
    )))
}

/// Device size in pixels of a scene axis of length `value` at `scale`.
///
/// Rule: `max(1, round(value × scale))`, with `f64::round` (half away from
/// zero). The unscaled `value` must itself round to a valid page size, so an
/// invalid scene fails exactly as it does at scale 1. At `scale = 1.0` the
/// result equals the unscaled size.
pub(crate) fn scaled_px(value: f64, scale: f64, axis: &str) -> Result<u32, RenderError> {
    check_scale(scale)?;
    let base = f64_to_px(value, axis)?;
    if scale == 1.0 {
        return Ok(base);
    }
    let scaled = (value * scale).round().max(1.0);
    f64_to_px(scaled, axis)
}

/// Device size in pixels of a page axis of length `value` at `scale`, with no
/// cap on the page or the result.
///
/// Same rule as [`scaled_px`]: `max(1, round(value × scale))`, and `value`
/// must round to a positive size. Region renders use it: they never allocate
/// the whole page, so the full-page dimension cap does not apply. Returns
/// `None` for a result that does not fit `u32`.
pub(crate) fn page_px(value: f64, scale: f64, axis: &str) -> Result<Option<u32>, RenderError> {
    check_scale(scale)?;
    if !value.is_finite() || value.round() <= 0.0 {
        return Err(RenderError::new(format!(
            "scene {axis} {value} is not a positive finite size"
        )));
    }
    let scaled = if scale == 1.0 {
        value.round()
    } else {
        (value * scale).round().max(1.0)
    };
    if scaled <= f64::from(u32::MAX) {
        Ok(Some(scaled as u32))
    } else {
        Ok(None)
    }
}

/// Shadow layers with offset and blur multiplied by `scale`.
pub(super) fn scale_shadows(shadows: &[ShadowSpec], scale: f64) -> Vec<ShadowSpec> {
    shadows
        .iter()
        .map(|s| ShadowSpec {
            dx: s.dx * scale,
            dy: s.dy * scale,
            blur: s.blur * scale,
            color: s.color,
        })
        .collect()
}

/// Mask with box, corner radius, and feather multiplied by `scale`.
pub(super) fn scale_mask(mask: MaskSpec, scale: f64) -> MaskSpec {
    MaskSpec {
        shape: mask.shape,
        radius: mask.radius * scale,
        feather: mask.feather * scale,
        invert: mask.invert,
        x: mask.x * scale,
        y: mask.y * scale,
        w: mask.w * scale,
        h: mask.h * scale,
    }
}

/// Filters with every pixel-sized parameter multiplied by `scale`.
///
/// Only `Noise` carries a pixel size (its grain cell). Every other filter is a
/// per-pixel color transform and is unchanged.
pub(super) fn scale_filters(filters: &[FilterSpec], scale: f64) -> Vec<FilterSpec> {
    filters
        .iter()
        .map(|f| match *f {
            FilterSpec::Noise {
                amount,
                seed,
                scale: cell,
            } => FilterSpec::Noise {
                amount,
                seed,
                scale: cell * scale,
            },
            FilterSpec::Grayscale(a) => FilterSpec::Grayscale(a),
            FilterSpec::Invert(a) => FilterSpec::Invert(a),
            FilterSpec::Sepia(a) => FilterSpec::Sepia(a),
            FilterSpec::Saturate(a) => FilterSpec::Saturate(a),
            FilterSpec::Brightness(a) => FilterSpec::Brightness(a),
            FilterSpec::Contrast(a) => FilterSpec::Contrast(a),
            FilterSpec::HueRotate(a) => FilterSpec::HueRotate(a),
            FilterSpec::Duotone {
                amount,
                shadow,
                highlight,
            } => FilterSpec::Duotone {
                amount,
                shadow,
                highlight,
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_px_rounds_half_away_from_zero() {
        assert_eq!(scaled_px(1920.0, 0.5, "width").unwrap(), 960);
        assert_eq!(scaled_px(1081.0, 0.5, "height").unwrap(), 541);
        assert_eq!(scaled_px(100.0, 0.333, "width").unwrap(), 33);
        assert_eq!(scaled_px(3.0, 0.01, "width").unwrap(), 1);
    }

    #[test]
    fn scaled_px_at_one_matches_unscaled() {
        assert_eq!(scaled_px(100.4, 1.0, "width").unwrap(), 100);
        assert_eq!(scaled_px(100.6, 1.0, "width").unwrap(), 101);
    }

    #[test]
    fn invalid_scale_is_rejected() {
        for s in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert!(scaled_px(100.0, s, "width").is_err(), "scale {s}");
        }
    }

    #[test]
    fn page_px_matches_scaled_px_without_the_page_cap() {
        for (v, s) in [
            (1920.0, 0.5),
            (1081.0, 0.5),
            (100.4, 1.0),
            (3.0, 0.01),
            (333.3, 2.7),
        ] {
            assert_eq!(
                page_px(v, s, "w").unwrap(),
                Some(scaled_px(v, s, "w").unwrap())
            );
        }
        assert_eq!(page_px(20_000.0, 8.0, "w").unwrap(), Some(160_000));
        assert!(page_px(0.4, 2.0, "w").is_err());
        assert!(page_px(f64::NAN, 2.0, "w").is_err());
        assert_eq!(page_px(1e300, 1e10, "w").unwrap(), None);
    }

    #[test]
    fn scaled_px_rejects_oversized_output() {
        assert!(scaled_px(10_000.0, 4.0, "width").is_err());
    }
}
