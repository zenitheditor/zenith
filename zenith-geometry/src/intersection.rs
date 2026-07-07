use crate::{GeometryError, Point2};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntersectionPoint {
    pub point: Point2,
    pub first_t: f64,
    pub second_t: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineIntersection {
    Point(IntersectionPoint),
    Parallel,
    Collinear,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SegmentIntersection {
    Point(IntersectionPoint),
    Overlap {
        start: IntersectionPoint,
        end: IntersectionPoint,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolylineIntersection {
    pub first_segment_index: usize,
    pub second_segment_index: usize,
    pub intersection: SegmentIntersection,
}

pub fn intersect_lines(
    first_start: Point2,
    first_end: Point2,
    second_start: Point2,
    second_end: Point2,
) -> Result<LineIntersection, GeometryError> {
    let inputs = validate_inputs(first_start, first_end, second_start, second_end)?;
    line_intersection(inputs)
}

pub fn intersect_segments(
    first_start: Point2,
    first_end: Point2,
    second_start: Point2,
    second_end: Point2,
) -> Result<Option<SegmentIntersection>, GeometryError> {
    let inputs = validate_inputs(first_start, first_end, second_start, second_end)?;
    segment_intersection(inputs)
}

pub fn collect_open_polyline_intersections(
    first: &[Point2],
    second: &[Point2],
) -> Result<Vec<PolylineIntersection>, GeometryError> {
    validate_points(first)?;
    validate_points(second)?;

    if first.len() < 2 || second.len() < 2 {
        return Ok(Vec::new());
    }

    let mut intersections = Vec::new();
    for (first_segment_index, first_segment) in first.windows(2).enumerate() {
        let Some(first_start) = first_segment.first().copied() else {
            continue;
        };
        let Some(first_end) = first_segment.get(1).copied() else {
            continue;
        };
        for (second_segment_index, second_segment) in second.windows(2).enumerate() {
            let Some(second_start) = second_segment.first().copied() else {
                continue;
            };
            let Some(second_end) = second_segment.get(1).copied() else {
                continue;
            };
            let inputs = line_inputs(first_start, first_end, second_start, second_end)?;
            if let Some(intersection) = segment_intersection(inputs)? {
                intersections.push(PolylineIntersection {
                    first_segment_index,
                    second_segment_index,
                    intersection,
                });
            }
        }
    }

    Ok(intersections)
}

#[derive(Debug, Clone, Copy)]
struct LineInputs {
    first_start: Point2,
    second_start: Point2,
    second_end: Point2,
    first_vector: Point2,
    second_vector: Point2,
    start_delta: Point2,
}

fn validate_inputs(
    first_start: Point2,
    first_end: Point2,
    second_start: Point2,
    second_end: Point2,
) -> Result<LineInputs, GeometryError> {
    first_start.validate()?;
    first_end.validate()?;
    second_start.validate()?;
    second_end.validate()?;

    line_inputs(first_start, first_end, second_start, second_end)
}

fn line_inputs(
    first_start: Point2,
    first_end: Point2,
    second_start: Point2,
    second_end: Point2,
) -> Result<LineInputs, GeometryError> {
    let first_vector = subtract(first_end, first_start)?;
    let second_vector = subtract(second_end, second_start)?;
    if first_vector.x == 0.0 && first_vector.y == 0.0 {
        return Err(GeometryError::DegenerateLine);
    }
    if second_vector.x == 0.0 && second_vector.y == 0.0 {
        return Err(GeometryError::DegenerateLine);
    }

    Ok(LineInputs {
        first_start,
        second_start,
        second_end,
        first_vector,
        second_vector,
        start_delta: subtract(second_start, first_start)?,
    })
}

fn segment_intersection(inputs: LineInputs) -> Result<Option<SegmentIntersection>, GeometryError> {
    match line_intersection(inputs)? {
        LineIntersection::Point(point) => {
            if in_unit_range(point.first_t) && in_unit_range(point.second_t) {
                Ok(Some(SegmentIntersection::Point(point)))
            } else {
                Ok(None)
            }
        }
        LineIntersection::Parallel => Ok(None),
        LineIntersection::Collinear => collinear_segment_intersection(inputs),
    }
}

fn validate_points(points: &[Point2]) -> Result<(), GeometryError> {
    points.iter().try_for_each(|point| point.validate())
}

fn line_intersection(inputs: LineInputs) -> Result<LineIntersection, GeometryError> {
    let denominator = cross(inputs.first_vector, inputs.second_vector)?;
    let collinearity = cross(inputs.start_delta, inputs.first_vector)?;
    if denominator == 0.0 {
        return if collinearity == 0.0 {
            Ok(LineIntersection::Collinear)
        } else {
            Ok(LineIntersection::Parallel)
        };
    }

    let first_t = canonical_t(cross(inputs.start_delta, inputs.second_vector)? / denominator)?;
    let second_t = canonical_t(cross(inputs.start_delta, inputs.first_vector)? / denominator)?;
    Ok(LineIntersection::Point(intersection_point(
        inputs, first_t, second_t,
    )?))
}

fn collinear_segment_intersection(
    inputs: LineInputs,
) -> Result<Option<SegmentIntersection>, GeometryError> {
    let second_start_t = parameter_on_first(inputs, inputs.second_start)?;
    let second_end_t = parameter_on_first(inputs, inputs.second_end)?;
    let overlap_start_t = second_start_t.min(second_end_t).max(0.0);
    let overlap_end_t = second_start_t.max(second_end_t).min(1.0);

    if overlap_start_t > overlap_end_t {
        return Ok(None);
    }

    let start = collinear_point(inputs, overlap_start_t)?;
    if overlap_start_t == overlap_end_t {
        return Ok(Some(SegmentIntersection::Point(start)));
    }

    let end = collinear_point(inputs, overlap_end_t)?;
    Ok(Some(SegmentIntersection::Overlap { start, end }))
}

fn collinear_point(inputs: LineInputs, first_t: f64) -> Result<IntersectionPoint, GeometryError> {
    let point = point_on_first(inputs, first_t)?;
    let second_t = parameter_on_second(inputs, point)?;
    Ok(IntersectionPoint {
        point,
        first_t: canonical_t(first_t)?,
        second_t,
    })
}

fn intersection_point(
    inputs: LineInputs,
    first_t: f64,
    second_t: f64,
) -> Result<IntersectionPoint, GeometryError> {
    Ok(IntersectionPoint {
        point: point_on_first(inputs, first_t)?,
        first_t,
        second_t,
    })
}

fn point_on_first(inputs: LineInputs, first_t: f64) -> Result<Point2, GeometryError> {
    finite_point(Point2::new_unchecked(
        inputs.first_start.x + inputs.first_vector.x * first_t,
        inputs.first_start.y + inputs.first_vector.y * first_t,
    ))
}

fn parameter_on_first(inputs: LineInputs, point: Point2) -> Result<f64, GeometryError> {
    parameter_on_axis(inputs.first_start, inputs.first_vector, point)
}

fn parameter_on_second(inputs: LineInputs, point: Point2) -> Result<f64, GeometryError> {
    parameter_on_axis(inputs.second_start, inputs.second_vector, point)
}

fn parameter_on_axis(start: Point2, vector: Point2, point: Point2) -> Result<f64, GeometryError> {
    let use_x = vector.x.abs() >= vector.y.abs();
    let numerator = if use_x {
        point.x - start.x
    } else {
        point.y - start.y
    };
    let denominator = if use_x { vector.x } else { vector.y };
    if !numerator.is_finite() || !denominator.is_finite() {
        return Err(GeometryError::CountOutOfRange);
    }
    if denominator == 0.0 {
        return Err(GeometryError::DegenerateLine);
    }

    canonical_t(numerator / denominator)
}

fn subtract(a: Point2, b: Point2) -> Result<Point2, GeometryError> {
    finite_point(Point2::new_unchecked(a.x - b.x, a.y - b.y))
}

fn cross(a: Point2, b: Point2) -> Result<f64, GeometryError> {
    finite_f64(a.x.mul_add(b.y, -(a.y * b.x)))
}

fn canonical_t(t: f64) -> Result<f64, GeometryError> {
    finite_f64(t).map(|value| if value == 0.0 { 0.0 } else { value })
}

fn in_unit_range(t: f64) -> bool {
    (0.0..=1.0).contains(&t)
}

fn finite_point(point: Point2) -> Result<Point2, GeometryError> {
    if point.is_finite() {
        Ok(point)
    } else {
        Err(GeometryError::CountOutOfRange)
    }
}

fn finite_f64(value: f64) -> Result<f64, GeometryError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(GeometryError::CountOutOfRange)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1.0e-10;

    fn point(x: f64, y: f64) -> Point2 {
        Point2::new_unchecked(x, y)
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= EPSILON,
            "expected {actual} to be within {EPSILON} of {expected}"
        );
    }

    fn assert_point_close(actual: Point2, expected: Point2) {
        assert_close(actual.x, expected.x);
        assert_close(actual.y, expected.y);
    }

    fn assert_intersection_close(actual: IntersectionPoint, expected: IntersectionPoint) {
        assert_point_close(actual.point, expected.point);
        assert_close(actual.first_t, expected.first_t);
        assert_close(actual.second_t, expected.second_t);
    }

    #[test]
    fn lines_intersect_with_unbounded_parameters() {
        let intersection = intersect_lines(
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(2.0, -1.0),
            point(2.0, 1.0),
        )
        .expect("valid lines");

        let LineIntersection::Point(intersection) = intersection else {
            panic!("expected point intersection");
        };
        assert_intersection_close(
            intersection,
            IntersectionPoint {
                point: point(2.0, 0.0),
                first_t: 2.0,
                second_t: 0.5,
            },
        );
    }

    #[test]
    fn parallel_lines_are_classified() {
        assert_eq!(
            intersect_lines(
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(0.0, 1.0),
                point(1.0, 1.0)
            ),
            Ok(LineIntersection::Parallel)
        );
    }

    #[test]
    fn collinear_lines_are_classified() {
        assert_eq!(
            intersect_lines(
                point(0.0, 0.0),
                point(2.0, 0.0),
                point(1.0, 0.0),
                point(3.0, 0.0)
            ),
            Ok(LineIntersection::Collinear)
        );
    }

    #[test]
    fn segments_cross_at_interior_point() {
        assert_eq!(
            intersect_segments(
                point(0.0, 0.0),
                point(10.0, 10.0),
                point(0.0, 10.0),
                point(10.0, 0.0)
            ),
            Ok(Some(SegmentIntersection::Point(IntersectionPoint {
                point: point(5.0, 5.0),
                first_t: 0.5,
                second_t: 0.5,
            })))
        );
    }

    #[test]
    fn endpoint_touch_counts_as_point_intersection() {
        assert_eq!(
            intersect_segments(
                point(0.0, 0.0),
                point(10.0, 0.0),
                point(10.0, 0.0),
                point(10.0, 10.0)
            ),
            Ok(Some(SegmentIntersection::Point(IntersectionPoint {
                point: point(10.0, 0.0),
                first_t: 1.0,
                second_t: 0.0,
            })))
        );
    }

    #[test]
    fn disjoint_segments_return_none() {
        assert_eq!(
            intersect_segments(
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(2.0, -1.0),
                point(2.0, 1.0)
            ),
            Ok(None)
        );
    }

    #[test]
    fn parallel_disjoint_segments_return_none() {
        assert_eq!(
            intersect_segments(
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(0.0, 1.0),
                point(1.0, 1.0)
            ),
            Ok(None)
        );
    }

    #[test]
    fn collinear_disjoint_segments_return_none() {
        assert_eq!(
            intersect_segments(
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(2.0, 0.0),
                point(3.0, 0.0)
            ),
            Ok(None)
        );
    }

    #[test]
    fn collinear_overlap_is_ordered_by_first_segment() {
        assert_eq!(
            intersect_segments(
                point(0.0, 0.0),
                point(10.0, 0.0),
                point(4.0, 0.0),
                point(12.0, 0.0)
            ),
            Ok(Some(SegmentIntersection::Overlap {
                start: IntersectionPoint {
                    point: point(4.0, 0.0),
                    first_t: 0.4,
                    second_t: 0.0,
                },
                end: IntersectionPoint {
                    point: point(10.0, 0.0),
                    first_t: 1.0,
                    second_t: 0.75,
                },
            }))
        );
    }

    #[test]
    fn reversed_collinear_overlap_preserves_second_parameters() {
        assert_eq!(
            intersect_segments(
                point(0.0, 0.0),
                point(10.0, 0.0),
                point(12.0, 0.0),
                point(4.0, 0.0)
            ),
            Ok(Some(SegmentIntersection::Overlap {
                start: IntersectionPoint {
                    point: point(4.0, 0.0),
                    first_t: 0.4,
                    second_t: 1.0,
                },
                end: IntersectionPoint {
                    point: point(10.0, 0.0),
                    first_t: 1.0,
                    second_t: 0.25,
                },
            }))
        );
    }

    #[test]
    fn identical_segments_return_full_overlap() {
        assert_eq!(
            intersect_segments(
                point(0.0, 0.0),
                point(10.0, 0.0),
                point(0.0, 0.0),
                point(10.0, 0.0)
            ),
            Ok(Some(SegmentIntersection::Overlap {
                start: IntersectionPoint {
                    point: point(0.0, 0.0),
                    first_t: 0.0,
                    second_t: 0.0,
                },
                end: IntersectionPoint {
                    point: point(10.0, 0.0),
                    first_t: 1.0,
                    second_t: 1.0,
                },
            }))
        );
    }

    #[test]
    fn collinear_endpoint_touch_returns_point() {
        assert_eq!(
            intersect_segments(
                point(0.0, 0.0),
                point(10.0, 0.0),
                point(10.0, 0.0),
                point(20.0, 0.0)
            ),
            Ok(Some(SegmentIntersection::Point(IntersectionPoint {
                point: point(10.0, 0.0),
                first_t: 1.0,
                second_t: 0.0,
            })))
        );
    }

    #[test]
    fn vertical_and_horizontal_segments_intersect() {
        assert_eq!(
            intersect_segments(
                point(2.0, -2.0),
                point(2.0, 2.0),
                point(0.0, 1.0),
                point(4.0, 1.0)
            ),
            Ok(Some(SegmentIntersection::Point(IntersectionPoint {
                point: point(2.0, 1.0),
                first_t: 0.75,
                second_t: 0.5,
            })))
        );
    }

    #[test]
    fn rejects_non_finite_points() {
        assert_eq!(
            intersect_lines(
                point(f64::NAN, 0.0),
                point(1.0, 0.0),
                point(0.0, 0.0),
                point(0.0, 1.0)
            ),
            Err(GeometryError::NonFinitePoint)
        );
    }

    #[test]
    fn rejects_degenerate_inputs() {
        assert_eq!(
            intersect_segments(
                point(0.0, 0.0),
                point(0.0, 0.0),
                point(0.0, 0.0),
                point(1.0, 0.0)
            ),
            Err(GeometryError::DegenerateLine)
        );
    }

    #[test]
    fn empty_and_single_point_polylines_have_no_intersections() {
        assert_eq!(
            collect_open_polyline_intersections(&[], &[point(0.0, 0.0), point(1.0, 0.0)]),
            Ok(Vec::new())
        );
        assert_eq!(
            collect_open_polyline_intersections(&[point(0.0, 0.0)], &[point(0.0, 0.0)]),
            Ok(Vec::new())
        );
    }

    #[test]
    fn polyline_collection_rejects_invalid_points_and_degenerate_segments() {
        assert_eq!(
            collect_open_polyline_intersections(
                &[point(f64::NAN, 0.0)],
                &[point(0.0, 0.0), point(1.0, 0.0)]
            ),
            Err(GeometryError::NonFinitePoint)
        );
        assert_eq!(
            collect_open_polyline_intersections(
                &[point(0.0, 0.0), point(0.0, 0.0)],
                &[point(0.0, 0.0), point(1.0, 0.0)]
            ),
            Err(GeometryError::DegenerateLine)
        );
    }

    #[test]
    fn polyline_collection_finds_crossing_with_segment_indices() {
        assert_eq!(
            collect_open_polyline_intersections(
                &[point(0.0, 0.0), point(10.0, 10.0)],
                &[point(0.0, 10.0), point(10.0, 0.0)]
            ),
            Ok(vec![PolylineIntersection {
                first_segment_index: 0,
                second_segment_index: 0,
                intersection: SegmentIntersection::Point(IntersectionPoint {
                    point: point(5.0, 5.0),
                    first_t: 0.5,
                    second_t: 0.5,
                }),
            }])
        );
    }

    #[test]
    fn polyline_collection_keeps_shared_endpoint_touch() {
        assert_eq!(
            collect_open_polyline_intersections(
                &[point(0.0, 0.0), point(10.0, 0.0)],
                &[point(10.0, 0.0), point(10.0, 10.0)]
            ),
            Ok(vec![PolylineIntersection {
                first_segment_index: 0,
                second_segment_index: 0,
                intersection: SegmentIntersection::Point(IntersectionPoint {
                    point: point(10.0, 0.0),
                    first_t: 1.0,
                    second_t: 0.0,
                }),
            }])
        );
    }

    #[test]
    fn polyline_collection_preserves_overlap_results() {
        assert_eq!(
            collect_open_polyline_intersections(
                &[point(0.0, 0.0), point(10.0, 0.0)],
                &[point(4.0, 0.0), point(12.0, 0.0)]
            ),
            Ok(vec![PolylineIntersection {
                first_segment_index: 0,
                second_segment_index: 0,
                intersection: SegmentIntersection::Overlap {
                    start: IntersectionPoint {
                        point: point(4.0, 0.0),
                        first_t: 0.4,
                        second_t: 0.0,
                    },
                    end: IntersectionPoint {
                        point: point(10.0, 0.0),
                        first_t: 1.0,
                        second_t: 0.75,
                    },
                },
            }])
        );
    }

    #[test]
    fn polyline_collection_preserves_segment_pair_order() {
        assert_eq!(
            collect_open_polyline_intersections(
                &[point(0.0, 0.0), point(10.0, 0.0), point(10.0, 10.0)],
                &[point(5.0, -5.0), point(5.0, 5.0), point(15.0, 5.0)]
            ),
            Ok(vec![
                PolylineIntersection {
                    first_segment_index: 0,
                    second_segment_index: 0,
                    intersection: SegmentIntersection::Point(IntersectionPoint {
                        point: point(5.0, 0.0),
                        first_t: 0.5,
                        second_t: 0.5,
                    }),
                },
                PolylineIntersection {
                    first_segment_index: 1,
                    second_segment_index: 1,
                    intersection: SegmentIntersection::Point(IntersectionPoint {
                        point: point(10.0, 5.0),
                        first_t: 0.5,
                        second_t: 0.5,
                    }),
                },
            ])
        );
    }
}
