//! sRGB transfer helpers for deterministic raster conversion.

use zenith_core::Color;

use crate::surface::{LinearRgba, RasterError};

/// Decode an 8-bit sRGB channel to linear light.
pub fn decode_srgb_u8(channel: u8) -> f32 {
    let srgb = f32::from(channel) / 255.0;
    if srgb <= 0.04045 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    }
}

/// Encode a linear-light channel to 8-bit sRGB.
pub fn encode_linear_to_srgb_u8(channel: f32) -> u8 {
    let linear = clamp_finite_unit(channel);
    let srgb = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    quantize_unit_to_u8(srgb)
}

/// Convert a Zenith straight-alpha sRGB color to premultiplied linear RGBA.
pub fn color_to_linear_rgba(color: Color) -> Result<LinearRgba, RasterError> {
    let alpha = f32::from(color.a) / 255.0;
    LinearRgba::straight(
        decode_srgb_u8(color.r),
        decode_srgb_u8(color.g),
        decode_srgb_u8(color.b),
        alpha,
    )
}

fn clamp_finite_unit(channel: f32) -> f32 {
    if !channel.is_finite() || channel <= 0.0 {
        0.0
    } else if channel >= 1.0 {
        1.0
    } else {
        channel
    }
}

fn quantize_unit_to_u8(channel: f32) -> u8 {
    let scaled = clamp_finite_unit(channel) * 255.0;
    let lower = scaled.floor();
    let fraction = scaled - lower;
    let lower_int = lower as u16;

    let rounded = if fraction < 0.5 {
        lower_int
    } else if fraction > 0.5 {
        lower_int + 1
    } else if lower_int % 2 == 0 {
        lower_int
    } else {
        lower_int + 1
    };

    if rounded >= 255 { 255 } else { rounded as u8 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_srgb_endpoints_and_midpoint_are_stable() {
        assert_eq!(decode_srgb_u8(0), 0.0);
        assert_eq!(decode_srgb_u8(255), 1.0);
        assert!((decode_srgb_u8(128) - 0.215_860_53).abs() < 0.000_001);
    }

    #[test]
    fn encode_linear_endpoints_and_midpoint_are_stable() {
        assert_eq!(encode_linear_to_srgb_u8(0.0), 0);
        assert_eq!(encode_linear_to_srgb_u8(1.0), 255);
        assert_eq!(encode_linear_to_srgb_u8(0.214_041_14), 128);
    }

    #[test]
    fn quantization_uses_half_to_even() {
        assert_eq!(quantize_unit_to_u8(10.5 / 255.0), 10);
        assert_eq!(quantize_unit_to_u8(11.5 / 255.0), 12);
    }

    #[test]
    fn encode_clamps_non_finite_and_out_of_range_values() {
        assert_eq!(encode_linear_to_srgb_u8(f32::NAN), 0);
        assert_eq!(encode_linear_to_srgb_u8(-1.0), 0);
        assert_eq!(encode_linear_to_srgb_u8(2.0), 255);
    }

    #[test]
    fn color_conversion_preserves_alpha_premultiplication() {
        let color = Color::srgb(255, 128, 0, 128);
        let pixel = color_to_linear_rgba(color).unwrap();
        let alpha = 128.0 / 255.0;

        assert_eq!(pixel.a(), alpha);
        assert_eq!(pixel.r(), alpha);
        assert!((pixel.g() - decode_srgb_u8(128) * alpha).abs() < 0.000_001);
        assert_eq!(pixel.b(), 0.0);
    }
}
