//! Raster-local adjustment layers.

use zenith_core::GradientStop;

use crate::surface::{LinearRgba, RasterError, Surface};
use crate::transfer::color_to_linear_rgba;

/// A borrowed raster-local adjustment operation.
#[derive(Debug, Clone, Copy)]
pub enum Adjustment<'a> {
    GradientMap { stops: &'a [GradientStop] },
}

impl<'a> Adjustment<'a> {
    /// Create a gradient-map adjustment.
    pub const fn gradient_map(stops: &'a [GradientStop]) -> Self {
        Self::GradientMap { stops }
    }

    pub(crate) fn apply(self, input: &Surface) -> Result<Surface, RasterError> {
        match self {
            Self::GradientMap { stops } => gradient_map(input, stops),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct StopColor {
    offset: f32,
    color: StraightRgb,
}

#[derive(Debug, Clone, Copy)]
struct StraightRgb {
    r: f32,
    g: f32,
    b: f32,
}

fn gradient_map(input: &Surface, stops: &[GradientStop]) -> Result<Surface, RasterError> {
    let stops = validated_stop_colors(stops)?;
    let mut pixels = Vec::with_capacity(input.pixels().len());

    for pixel in input.pixels() {
        let alpha = pixel.a();
        let luminance = linear_luminance(*pixel);
        let color = sample_stops(&stops, luminance)?;
        pixels.push(LinearRgba::premultiplied(
            color.r * alpha,
            color.g * alpha,
            color.b * alpha,
            alpha,
        )?);
    }

    Surface::from_pixels(input.width(), input.height(), pixels)
}

fn validated_stop_colors(stops: &[GradientStop]) -> Result<Vec<StopColor>, RasterError> {
    if stops.len() < 2 {
        return Err(RasterError::InvalidGradientStops);
    }

    let mut previous_offset = None;
    let mut colors = Vec::with_capacity(stops.len());

    for stop in stops {
        let offset = stop.offset;
        if !offset.is_finite() || !(0.0..=1.0).contains(&offset) {
            return Err(RasterError::InvalidGradientStops);
        }
        if let Some(previous_offset) = previous_offset {
            if offset < previous_offset {
                return Err(RasterError::InvalidGradientStops);
            }
        }
        previous_offset = Some(offset);

        let pixel = color_to_linear_rgba(stop.color)?;
        let color = straight_rgb(pixel);
        colors.push(StopColor {
            offset: offset as f32,
            color,
        });
    }

    Ok(colors)
}

fn sample_stops(stops: &[StopColor], sample: f32) -> Result<StraightRgb, RasterError> {
    let first = stops.first().ok_or(RasterError::InvalidGradientStops)?;
    if sample < first.offset {
        return Ok(first.color);
    }

    let upper = stops.partition_point(|stop| stop.offset <= sample);
    if upper == stops.len() {
        return Ok(stops
            .last()
            .copied()
            .ok_or(RasterError::InvalidGradientStops)?
            .color);
    }

    let previous = stops
        .get(upper.saturating_sub(1))
        .copied()
        .ok_or(RasterError::InvalidGradientStops)?;
    let next = stops
        .get(upper)
        .copied()
        .ok_or(RasterError::InvalidGradientStops)?;

    if next.offset <= previous.offset {
        return Ok(next.color);
    }

    let t = (sample - previous.offset) / (next.offset - previous.offset);
    Ok(lerp_rgb(previous.color, next.color, t))
}

fn linear_luminance(pixel: LinearRgba) -> f32 {
    let rgb = straight_rgb(pixel);
    clamp_unit(0.2126 * rgb.r + 0.7152 * rgb.g + 0.0722 * rgb.b)
}

fn straight_rgb(pixel: LinearRgba) -> StraightRgb {
    let alpha = pixel.a();
    if alpha <= 0.0 {
        StraightRgb {
            r: 0.0,
            g: 0.0,
            b: 0.0,
        }
    } else {
        StraightRgb {
            r: clamp_unit(pixel.r() / alpha),
            g: clamp_unit(pixel.g() / alpha),
            b: clamp_unit(pixel.b() / alpha),
        }
    }
}

fn lerp_rgb(start: StraightRgb, end: StraightRgb, t: f32) -> StraightRgb {
    StraightRgb {
        r: start.r + (end.r - start.r) * t,
        g: start.g + (end.g - start.g) * t,
        b: start.b + (end.b - start.b) * t,
    }
}

fn clamp_unit(channel: f32) -> f32 {
    if !channel.is_finite() || channel <= 0.0 {
        0.0
    } else if channel >= 1.0 {
        1.0
    } else {
        channel
    }
}
