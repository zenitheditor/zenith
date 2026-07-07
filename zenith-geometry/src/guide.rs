use crate::{GeometryError, Point2, RectBounds};

const MAX_CONSTRUCTION_GUIDES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConstructionGuide {
    Segment { start: Point2, end: Point2 },
    Circle { center: Point2, radius: f64 },
}

impl ConstructionGuide {
    pub fn segment(start: Point2, end: Point2) -> Result<Self, GeometryError> {
        validate_segment(start, end)?;
        Ok(Self::Segment { start, end })
    }

    pub fn circle(center: Point2, radius: f64) -> Result<Self, GeometryError> {
        validate_circle(center, radius)?;
        Ok(Self::Circle { center, radius })
    }

    pub fn bounds(self) -> Result<RectBounds, GeometryError> {
        match self {
            Self::Segment { start, end } => {
                validate_segment(start, end)?;
                Ok(RectBounds::from_point(start).include_point(end))
            }
            Self::Circle { center, radius } => {
                validate_circle(center, radius)?;
                RectBounds::from_xywh(
                    center.x - radius,
                    center.y - radius,
                    radius * 2.0,
                    radius * 2.0,
                )
            }
        }
    }
}

pub fn modular_guides(
    bounds: RectBounds,
    columns: usize,
    rows: usize,
) -> Result<Vec<ConstructionGuide>, GeometryError> {
    validate_rectangular_generator_bounds(bounds)?;
    validate_positive_count(columns)?;
    validate_positive_count(rows)?;
    let guide_count = columns
        .checked_add(rows)
        .and_then(|count| count.checked_add(2))
        .ok_or(GeometryError::CountOutOfRange)?;
    validate_guide_count(guide_count)?;

    let mut guides = Vec::with_capacity(guide_count);
    for column in 0..=columns {
        let ratio = column as f64 / columns as f64;
        guides.push(vertical_guide(bounds, ratio)?);
    }
    for row in 0..=rows {
        let ratio = row as f64 / rows as f64;
        guides.push(horizontal_guide(bounds, ratio)?);
    }

    Ok(guides)
}

pub fn ratio_guides(
    bounds: RectBounds,
    x_ratios: &[f64],
    y_ratios: &[f64],
) -> Result<Vec<ConstructionGuide>, GeometryError> {
    validate_rectangular_generator_bounds(bounds)?;
    let guide_count = x_ratios
        .len()
        .checked_add(y_ratios.len())
        .ok_or(GeometryError::CountOutOfRange)?;
    validate_guide_count(guide_count)?;

    let mut guides = Vec::with_capacity(guide_count);
    for ratio in x_ratios {
        validate_ratio(*ratio)?;
        guides.push(vertical_guide(bounds, *ratio)?);
    }
    for ratio in y_ratios {
        validate_ratio(*ratio)?;
        guides.push(horizontal_guide(bounds, *ratio)?);
    }

    Ok(guides)
}

pub fn polar_guides(
    center: Point2,
    radius: f64,
    rings: usize,
    spokes: usize,
    start_angle_degrees: f64,
) -> Result<Vec<ConstructionGuide>, GeometryError> {
    center.validate()?;
    validate_positive_radius(radius)?;
    validate_positive_count(rings)?;
    validate_positive_count(spokes)?;
    let guide_count = rings
        .checked_add(spokes)
        .ok_or(GeometryError::CountOutOfRange)?;
    validate_guide_count(guide_count)?;
    if !start_angle_degrees.is_finite() {
        return Err(GeometryError::NonFiniteParameter);
    }

    let mut guides = Vec::with_capacity(guide_count);
    for ring in 1..=rings {
        let ring_radius = radius * ring as f64 / rings as f64;
        guides.push(ConstructionGuide::circle(center, ring_radius)?);
    }

    let step_degrees = 360.0 / spokes as f64;
    for spoke in 0..spokes {
        let angle = (start_angle_degrees + step_degrees * spoke as f64).to_radians();
        let end = Point2::new(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
        )?;
        guides.push(ConstructionGuide::segment(center, end)?);
    }

    Ok(guides)
}

fn validate_segment(start: Point2, end: Point2) -> Result<(), GeometryError> {
    start.validate()?;
    end.validate()?;
    if start == end {
        Err(GeometryError::DegenerateLine)
    } else {
        Ok(())
    }
}

fn validate_circle(center: Point2, radius: f64) -> Result<(), GeometryError> {
    center.validate()?;
    validate_positive_radius(radius)
}

fn validate_positive_radius(radius: f64) -> Result<(), GeometryError> {
    if !radius.is_finite() {
        Err(GeometryError::NonFiniteParameter)
    } else if radius <= 0.0 {
        Err(GeometryError::NonPositiveTolerance)
    } else {
        Ok(())
    }
}

fn validate_rectangular_generator_bounds(bounds: RectBounds) -> Result<(), GeometryError> {
    if !bounds.min_x.is_finite()
        || !bounds.min_y.is_finite()
        || !bounds.max_x.is_finite()
        || !bounds.max_y.is_finite()
    {
        return Err(GeometryError::NonFiniteParameter);
    }
    if bounds.max_x < bounds.min_x || bounds.max_y < bounds.min_y {
        return Err(GeometryError::CountOutOfRange);
    }
    if bounds.max_x == bounds.min_x || bounds.max_y == bounds.min_y {
        return Err(GeometryError::DegenerateLine);
    }

    Ok(())
}

fn validate_positive_count(count: usize) -> Result<(), GeometryError> {
    if count == 0 {
        Err(GeometryError::NonPositiveCount)
    } else {
        Ok(())
    }
}

fn validate_guide_count(count: usize) -> Result<(), GeometryError> {
    match count {
        count if count <= MAX_CONSTRUCTION_GUIDES => Ok(()),
        _ => Err(GeometryError::CountOutOfRange),
    }
}

fn validate_ratio(ratio: f64) -> Result<(), GeometryError> {
    if !ratio.is_finite() {
        Err(GeometryError::NonFiniteParameter)
    } else if !(0.0..=1.0).contains(&ratio) {
        Err(GeometryError::ParameterOutOfRange)
    } else {
        Ok(())
    }
}

fn vertical_guide(bounds: RectBounds, ratio: f64) -> Result<ConstructionGuide, GeometryError> {
    let x = bounds.min_x + bounds.width() * ratio;
    ConstructionGuide::segment(Point2::new(x, bounds.min_y)?, Point2::new(x, bounds.max_y)?)
}

fn horizontal_guide(bounds: RectBounds, ratio: f64) -> Result<ConstructionGuide, GeometryError> {
    let y = bounds.min_y + bounds.height() * ratio;
    ConstructionGuide::segment(Point2::new(bounds.min_x, y)?, Point2::new(bounds.max_x, y)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).expect("valid point")
    }

    fn bounds() -> RectBounds {
        RectBounds::from_xywh(10.0, 20.0, 80.0, 40.0).expect("valid bounds")
    }

    #[test]
    fn constructs_segments_and_circles_with_bounds() {
        let segment =
            ConstructionGuide::segment(point(1.0, 2.0), point(4.0, 6.0)).expect("valid segment");
        let circle = ConstructionGuide::circle(point(5.0, 7.0), 3.0).expect("valid circle");

        assert_eq!(
            segment.bounds(),
            Ok(RectBounds {
                min_x: 1.0,
                min_y: 2.0,
                max_x: 4.0,
                max_y: 6.0
            })
        );
        assert_eq!(
            circle.bounds(),
            Ok(RectBounds {
                min_x: 2.0,
                min_y: 4.0,
                max_x: 8.0,
                max_y: 10.0
            })
        );
    }

    #[test]
    fn rejects_invalid_guides() {
        assert_eq!(
            ConstructionGuide::segment(point(1.0, 2.0), point(1.0, 2.0)),
            Err(GeometryError::DegenerateLine)
        );
        assert_eq!(
            ConstructionGuide::segment(point(1.0, 2.0), Point2::new_unchecked(f64::NAN, 2.0)),
            Err(GeometryError::NonFinitePoint)
        );
        assert_eq!(
            ConstructionGuide::circle(point(1.0, 2.0), 0.0),
            Err(GeometryError::NonPositiveTolerance)
        );
        assert_eq!(
            ConstructionGuide::circle(point(1.0, 2.0), f64::NAN),
            Err(GeometryError::NonFiniteParameter)
        );
    }

    #[test]
    fn modular_guides_emit_verticals_then_horizontals() {
        let guides = modular_guides(bounds(), 2, 1).expect("valid guides");

        assert_eq!(guides.len(), 5);
        assert_eq!(
            guides,
            vec![
                ConstructionGuide::Segment {
                    start: point(10.0, 20.0),
                    end: point(10.0, 60.0)
                },
                ConstructionGuide::Segment {
                    start: point(50.0, 20.0),
                    end: point(50.0, 60.0)
                },
                ConstructionGuide::Segment {
                    start: point(90.0, 20.0),
                    end: point(90.0, 60.0)
                },
                ConstructionGuide::Segment {
                    start: point(10.0, 20.0),
                    end: point(90.0, 20.0)
                },
                ConstructionGuide::Segment {
                    start: point(10.0, 60.0),
                    end: point(90.0, 60.0)
                }
            ]
        );
    }

    #[test]
    fn ratio_guides_emit_requested_positions() {
        let guides = ratio_guides(bounds(), &[0.25, 1.0], &[0.5]).expect("valid guides");

        assert_eq!(
            guides,
            vec![
                ConstructionGuide::Segment {
                    start: point(30.0, 20.0),
                    end: point(30.0, 60.0)
                },
                ConstructionGuide::Segment {
                    start: point(90.0, 20.0),
                    end: point(90.0, 60.0)
                },
                ConstructionGuide::Segment {
                    start: point(10.0, 40.0),
                    end: point(90.0, 40.0)
                }
            ]
        );
    }

    #[test]
    fn polar_guides_emit_rings_then_spokes() {
        let guides = polar_guides(point(10.0, 20.0), 8.0, 2, 2, 0.0).expect("valid guides");

        assert_eq!(
            guides,
            vec![
                ConstructionGuide::Circle {
                    center: point(10.0, 20.0),
                    radius: 4.0
                },
                ConstructionGuide::Circle {
                    center: point(10.0, 20.0),
                    radius: 8.0
                },
                ConstructionGuide::Segment {
                    start: point(10.0, 20.0),
                    end: point(18.0, 20.0)
                },
                ConstructionGuide::Segment {
                    start: point(10.0, 20.0),
                    end: point(2.0, 20.0)
                }
            ]
        );
    }

    #[test]
    fn generators_reject_invalid_input() {
        let invalid_bounds = RectBounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 0.0,
            max_y: 10.0,
        };

        assert_eq!(
            modular_guides(invalid_bounds, 1, 1),
            Err(GeometryError::DegenerateLine)
        );
        assert_eq!(
            modular_guides(bounds(), 0, 1),
            Err(GeometryError::NonPositiveCount)
        );
        assert_eq!(
            ratio_guides(bounds(), &[f64::NAN], &[]),
            Err(GeometryError::NonFiniteParameter)
        );
        assert_eq!(
            ratio_guides(bounds(), &[1.1], &[]),
            Err(GeometryError::ParameterOutOfRange)
        );
        assert_eq!(
            polar_guides(point(0.0, 0.0), 10.0, 0, 1, 0.0),
            Err(GeometryError::NonPositiveCount)
        );
        assert_eq!(
            polar_guides(point(0.0, 0.0), 10.0, 1, MAX_CONSTRUCTION_GUIDES, 0.0),
            Err(GeometryError::CountOutOfRange)
        );
        assert_eq!(
            polar_guides(point(0.0, 0.0), 10.0, 1, 1, f64::INFINITY),
            Err(GeometryError::NonFiniteParameter)
        );
    }

    #[test]
    fn generators_reject_count_overflow() {
        assert_eq!(
            modular_guides(bounds(), usize::MAX, 1),
            Err(GeometryError::CountOutOfRange)
        );
        assert_eq!(
            polar_guides(point(0.0, 0.0), 10.0, usize::MAX, 1, 0.0),
            Err(GeometryError::CountOutOfRange)
        );
    }
}
