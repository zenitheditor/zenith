use zenith_raster::{LinearRgba, Surface};

/// Rec.709 luminance from straight linear RGB reconstructed from premultiplied storage.
pub fn luminance(pixel: LinearRgba) -> f32 {
    if pixel.a() <= 0.0 {
        return 0.0;
    }

    let r = clamp_unit(pixel.r() / pixel.a());
    let g = clamp_unit(pixel.g() / pixel.a());
    let b = clamp_unit(pixel.b() / pixel.a());
    (0.2126 * r) + (0.7152 * g) + (0.0722 * b)
}

pub fn pixel_count(surface: &Surface) -> u64 {
    u64::from(surface.width()) * u64::from(surface.height())
}

pub fn mean_luminance(surface: &Surface) -> f32 {
    let total = surface
        .pixels()
        .iter()
        .map(|pixel| luminance(*pixel))
        .sum::<f32>();
    total / pixel_count(surface) as f32
}

fn clamp_unit(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_pixels_have_zero_luminance() {
        assert_eq!(luminance(LinearRgba::TRANSPARENT), 0.0);
    }

    #[test]
    fn luminance_reconstructs_straight_linear_channels() {
        let pixel = LinearRgba::premultiplied(0.2, 0.1, 0.05, 0.5).unwrap();
        let actual = luminance(pixel);
        let expected = (0.2126 * 0.4) + (0.7152 * 0.2) + (0.0722 * 0.1);
        assert!((actual - expected).abs() < 0.000_001);
    }
}
