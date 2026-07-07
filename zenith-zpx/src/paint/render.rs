use zenith_raster::{LinearRgba, Surface, blend_pixel, color_to_linear_rgba};

use crate::error::ZpxError;
use crate::model::{Brush, Canvas, DabSample, Stroke, StrokeProgram};

const MAX_DABS_PER_STROKE: usize = 1_000_000;

pub fn render_program(program: &StrokeProgram, canvas: &Canvas) -> Result<Surface, ZpxError> {
    validate_program(program)?;
    let mut surface = Surface::new(canvas.width_px, canvas.height_px).map_err(raster_error)?;
    for stroke in &program.strokes {
        render_stroke(&mut surface, stroke)?;
    }
    Ok(surface)
}

pub fn validate_program(program: &StrokeProgram) -> Result<(), ZpxError> {
    for stroke in &program.strokes {
        validate_stroke(stroke)?;
    }
    Ok(())
}

pub fn validate_stroke(stroke: &Stroke) -> Result<(), ZpxError> {
    validate_brush(stroke.brush)?;
    if !is_unit_f64(stroke.opacity) {
        return Err(ZpxError::new("stroke opacity must be finite and in 0..=1"));
    }
    if stroke.path.is_empty() {
        return Err(ZpxError::new("stroke path requires at least one sample"));
    }
    for sample in &stroke.path {
        validate_sample(*sample)?;
    }
    Ok(())
}

pub fn validate_brush(brush: Brush) -> Result<(), ZpxError> {
    match brush {
        Brush::Round {
            radius_px,
            hardness,
            spacing,
        } => {
            if !radius_px.is_finite() || radius_px <= 0.0 {
                return Err(ZpxError::new(
                    "round brush radius must be finite and positive",
                ));
            }
            if !is_unit_f64(hardness) {
                return Err(ZpxError::new(
                    "round brush hardness must be finite and in 0..=1",
                ));
            }
            if !spacing.is_finite() || spacing <= 0.0 {
                return Err(ZpxError::new(
                    "round brush spacing must be finite and positive",
                ));
            }
            Ok(())
        }
    }
}

fn render_stroke(surface: &mut Surface, stroke: &Stroke) -> Result<(), ZpxError> {
    let base_pixel = color_to_linear_rgba(stroke.color).map_err(raster_error)?;
    let Brush::Round {
        radius_px,
        hardness,
        spacing,
    } = stroke.brush;
    let interval = spacing * radius_px * 2.0;
    for dab in dab_positions(&stroke.path, interval)? {
        stamp_round(
            surface,
            dab,
            RoundStamp {
                radius_px,
                hardness,
                opacity: stroke.opacity,
                pixel: base_pixel,
                blend_mode: stroke.blend_mode,
            },
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct RoundStamp {
    radius_px: f64,
    hardness: f64,
    opacity: f64,
    pixel: LinearRgba,
    blend_mode: zenith_core::BlendMode,
}

fn dab_positions(path: &[DabSample], interval: f64) -> Result<Vec<DabSample>, ZpxError> {
    if !interval.is_finite() || interval <= 0.0 {
        return Err(ZpxError::new("dab interval must be finite and positive"));
    }

    let mut positions = Vec::new();
    let mut previous = None;
    let mut next_distance = 0.0;
    let mut walked = 0.0;

    for sample in path {
        let current = *sample;
        if let Some(start) = previous {
            let segment_length = distance(start, current);
            if segment_length > 0.0 {
                while next_distance <= walked + segment_length {
                    let segment_offset = next_distance - walked;
                    let t = segment_offset / segment_length;
                    push_dab_position(&mut positions, interpolate_sample(start, current, t))?;
                    let advanced = next_distance + interval;
                    if advanced <= next_distance {
                        return Err(ZpxError::new("dab interval is too small to advance"));
                    }
                    next_distance = advanced;
                }
                walked += segment_length;
            }
        } else {
            push_dab_position(&mut positions, current)?;
            next_distance = interval;
        }
        previous = Some(current);
    }

    if positions.is_empty()
        && let Some(sample) = path.first()
    {
        push_dab_position(&mut positions, *sample)?;
    }

    Ok(positions)
}

fn push_dab_position(positions: &mut Vec<DabSample>, sample: DabSample) -> Result<(), ZpxError> {
    if positions.len() >= MAX_DABS_PER_STROKE {
        return Err(ZpxError::new("stroke produces too many dabs"));
    }
    positions.push(sample);
    Ok(())
}

fn stamp_round(
    surface: &mut Surface,
    sample: DabSample,
    stamp: RoundStamp,
) -> Result<(), ZpxError> {
    if sample.pressure <= 0.0 || stamp.opacity <= 0.0 {
        return Ok(());
    }

    let min_x = clamp_floor_to_u32(sample.x - stamp.radius_px, surface.width());
    let max_x = clamp_ceil_to_u32(sample.x + stamp.radius_px, surface.width());
    let min_y = clamp_floor_to_u32(sample.y - stamp.radius_px, surface.height());
    let max_y = clamp_ceil_to_u32(sample.y + stamp.radius_px, surface.height());

    for y in min_y..max_y {
        for x in min_x..max_x {
            let pixel_center_x = f64::from(x) + 0.5;
            let pixel_center_y = f64::from(y) + 0.5;
            let dx = pixel_center_x - sample.x;
            let dy = pixel_center_y - sample.y;
            let dist = (dx * dx + dy * dy).sqrt();
            let coverage = round_coverage(dist, stamp.radius_px, stamp.hardness);
            if coverage > 0.0 {
                let source = scale_pixel(stamp.pixel, stamp.opacity * sample.pressure * coverage)?;
                let backdrop = surface
                    .get(x, y)
                    .ok_or_else(|| ZpxError::new("paint target pixel out of bounds"))?;
                let blended =
                    blend_pixel(stamp.blend_mode, backdrop, source).map_err(raster_error)?;
                surface.set(x, y, blended).map_err(raster_error)?;
            }
        }
    }

    Ok(())
}

fn round_coverage(distance: f64, radius: f64, hardness: f64) -> f64 {
    if distance >= radius {
        return 0.0;
    }
    let hard_radius = radius * hardness;
    if distance <= hard_radius {
        return 1.0;
    }
    if hardness >= 1.0 {
        1.0
    } else {
        (radius - distance) / (radius - hard_radius)
    }
}

fn scale_pixel(pixel: LinearRgba, scale: f64) -> Result<LinearRgba, ZpxError> {
    let scale = scale.clamp(0.0, 1.0) as f32;
    LinearRgba::premultiplied(
        pixel.r() * scale,
        pixel.g() * scale,
        pixel.b() * scale,
        pixel.a() * scale,
    )
    .map_err(raster_error)
}

fn clamp_floor_to_u32(value: f64, limit: u32) -> u32 {
    if value <= 0.0 {
        0
    } else if value >= f64::from(limit) {
        limit
    } else {
        value.floor() as u32
    }
}

fn clamp_ceil_to_u32(value: f64, limit: u32) -> u32 {
    if value <= 0.0 {
        0
    } else if value >= f64::from(limit) {
        limit
    } else {
        value.ceil() as u32
    }
}

fn interpolate_sample(start: DabSample, end: DabSample, t: f64) -> DabSample {
    DabSample {
        x: start.x + (end.x - start.x) * t,
        y: start.y + (end.y - start.y) * t,
        pressure: start.pressure + (end.pressure - start.pressure) * t,
    }
}

fn distance(start: DabSample, end: DabSample) -> f64 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    (dx * dx + dy * dy).sqrt()
}

fn validate_sample(sample: DabSample) -> Result<(), ZpxError> {
    if !sample.x.is_finite() || !sample.y.is_finite() {
        return Err(ZpxError::new("stroke sample coordinates must be finite"));
    }
    if !is_unit_f64(sample.pressure) {
        return Err(ZpxError::new(
            "stroke sample pressure must be finite and in 0..=1",
        ));
    }
    Ok(())
}

fn is_unit_f64(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn raster_error(error: zenith_raster::RasterError) -> ZpxError {
    ZpxError::new(format!("raster paint error: {error:?}"))
}

#[cfg(test)]
mod tests {
    use zenith_core::{BlendMode, Color};

    use super::*;

    fn red_stroke(path: Vec<DabSample>, blend_mode: BlendMode) -> Stroke {
        Stroke {
            brush: Brush::Round {
                radius_px: 2.0,
                hardness: 0.5,
                spacing: 0.5,
            },
            path,
            color: Color::srgb(255, 0, 0, 255),
            opacity: 1.0,
            blend_mode,
            seed: 1,
        }
    }

    #[test]
    fn same_program_renders_identical_pixels() {
        let canvas = Canvas::new(8, 8);
        let program = StrokeProgram {
            strokes: vec![red_stroke(
                vec![
                    DabSample {
                        x: 1.0,
                        y: 4.0,
                        pressure: 1.0,
                    },
                    DabSample {
                        x: 6.0,
                        y: 4.0,
                        pressure: 1.0,
                    },
                ],
                BlendMode::Normal,
            )],
        };

        let first = render_program(&program, &canvas).expect("program renders");
        let second = render_program(&program, &canvas).expect("program renders");

        assert_eq!(first.pixels(), second.pixels());
    }

    #[test]
    fn stroke_order_and_blend_change_pixels() {
        let canvas = Canvas::new(8, 8);
        let red = red_stroke(
            vec![DabSample {
                x: 4.0,
                y: 4.0,
                pressure: 1.0,
            }],
            BlendMode::Normal,
        );
        let mut blue = red.clone();
        blue.color = Color::srgb(0, 0, 255, 160);
        blue.blend_mode = BlendMode::Multiply;

        let red_then_blue = render_program(
            &StrokeProgram {
                strokes: vec![red.clone(), blue.clone()],
            },
            &canvas,
        )
        .expect("program renders");
        let blue_then_red = render_program(
            &StrokeProgram {
                strokes: vec![blue, red],
            },
            &canvas,
        )
        .expect("program renders");

        assert_ne!(red_then_blue.pixels(), blue_then_red.pixels());
    }

    #[test]
    fn single_point_and_zero_length_paths_render() {
        let canvas = Canvas::new(5, 5);
        let single = render_program(
            &StrokeProgram {
                strokes: vec![red_stroke(
                    vec![DabSample {
                        x: 2.0,
                        y: 2.0,
                        pressure: 1.0,
                    }],
                    BlendMode::Normal,
                )],
            },
            &canvas,
        )
        .expect("single point renders");
        let zero_length = render_program(
            &StrokeProgram {
                strokes: vec![red_stroke(
                    vec![
                        DabSample {
                            x: 2.0,
                            y: 2.0,
                            pressure: 1.0,
                        },
                        DabSample {
                            x: 2.0,
                            y: 2.0,
                            pressure: 1.0,
                        },
                    ],
                    BlendMode::Normal,
                )],
            },
            &canvas,
        )
        .expect("zero length renders");

        assert!(single.pixels().iter().any(|pixel| pixel.a() > 0.0));
        assert_eq!(single.pixels(), zero_length.pixels());
    }

    #[test]
    fn edge_boundary_strokes_do_not_write_out_of_bounds() {
        let canvas = Canvas::new(3, 3);
        let surface = render_program(
            &StrokeProgram {
                strokes: vec![red_stroke(
                    vec![
                        DabSample {
                            x: -1.0,
                            y: -1.0,
                            pressure: 1.0,
                        },
                        DabSample {
                            x: 3.5,
                            y: 3.5,
                            pressure: 1.0,
                        },
                    ],
                    BlendMode::Normal,
                )],
            },
            &canvas,
        )
        .expect("edge stroke renders");

        assert_eq!(surface.pixels().len(), 9);
        assert!(surface.pixels().iter().any(|pixel| pixel.a() > 0.0));
    }
}
