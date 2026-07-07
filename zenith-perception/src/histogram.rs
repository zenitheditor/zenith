use zenith_raster::Surface;

use crate::scalar::{luminance, pixel_count};

pub const HISTOGRAM_BINS: usize = 10;

/// Ten-bin luminance histogram over the closed `[0, 1]` input range.
#[derive(Debug, Clone, PartialEq)]
pub struct Histogram {
    pub bins: [u64; HISTOGRAM_BINS],
    pub total_pixels: u64,
    pub min: f32,
    pub max: f32,
    pub mean: f32,
}

pub fn histogram(surface: &Surface) -> Histogram {
    let mut bins = [0_u64; HISTOGRAM_BINS];
    let mut min = 1.0_f32;
    let mut max = 0.0_f32;
    let mut total = 0.0_f32;

    for pixel in surface.pixels() {
        let value = luminance(*pixel);
        min = min.min(value);
        max = max.max(value);
        total += value;
        if let Some(bin) = bins.get_mut(histogram_bin(value)) {
            *bin += 1;
        }
    }

    let total_pixels = pixel_count(surface);
    Histogram {
        bins,
        total_pixels,
        min,
        max,
        mean: total / total_pixels as f32,
    }
}

fn histogram_bin(value: f32) -> usize {
    if value >= 1.0 {
        return HISTOGRAM_BINS - 1;
    }

    let scaled = (value * HISTOGRAM_BINS as f32).floor() as usize;
    scaled.min(HISTOGRAM_BINS - 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_raster::{LinearRgba, Surface};

    #[test]
    fn exact_boundaries_are_lower_inclusive_except_final_bin() {
        assert_eq!(histogram_bin(0.0), 0);
        assert_eq!(histogram_bin(0.1), 1);
        assert_eq!(histogram_bin(0.2), 2);
        assert_eq!(histogram_bin(0.3), 3);
        assert_eq!(histogram_bin(0.4), 4);
        assert_eq!(histogram_bin(0.5), 5);
        assert_eq!(histogram_bin(0.6), 6);
        assert_eq!(histogram_bin(0.7), 7);
        assert_eq!(histogram_bin(0.8), 8);
        assert_eq!(histogram_bin(0.9), 9);
        assert_eq!(histogram_bin(1.0), 9);
    }

    #[test]
    fn transparent_premultiplied_pixels_land_in_zero_bin() {
        let surface = Surface::from_pixels(2, 1, vec![LinearRgba::TRANSPARENT, gray(1.0)]).unwrap();

        let report = histogram(&surface);

        assert_eq!(report.bins, [1, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        assert_eq!(report.min, 0.0);
    }

    fn gray(value: f32) -> LinearRgba {
        LinearRgba::straight(value, value, value, 1.0).unwrap()
    }
}
