use crate::{GeometryError, LineIntersection, Point2, intersect_lines};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmentOffset {
    pub source_start: Point2,
    pub source_end: Point2,
    pub start: Point2,
    pub end: Point2,
    pub unit_normal: Point2,
    pub distance: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OffsetRailJoin {
    Point {
        point: Point2,
        first_on_segment: bool,
        second_on_segment: bool,
    },
    Parallel,
    Collinear,
}

pub fn offset_segment(
    start: Point2,
    end: Point2,
    distance: f64,
) -> Result<SegmentOffset, GeometryError> {
    start.validate()?;
    end.validate()?;
    if !distance.is_finite() {
        return Err(GeometryError::NonFiniteParameter);
    }

    let dx = end.x - start.x;
    let dy = end.y - start.y;
    if !dx.is_finite() || !dy.is_finite() {
        return Err(GeometryError::CountOutOfRange);
    }

    let length = dx.hypot(dy);
    if !length.is_finite() {
        return Err(GeometryError::CountOutOfRange);
    }
    if length == 0.0 {
        return Err(GeometryError::DegenerateLine);
    }

    let unit_normal = Point2::new(-dy / length, dx / length)?;
    let offset_x = unit_normal.x * distance;
    let offset_y = unit_normal.y * distance;
    if !offset_x.is_finite() || !offset_y.is_finite() {
        return Err(GeometryError::CountOutOfRange);
    }

    Ok(SegmentOffset {
        source_start: start,
        source_end: end,
        start: Point2::new(start.x + offset_x, start.y + offset_y)?,
        end: Point2::new(end.x + offset_x, end.y + offset_y)?,
        unit_normal,
        distance,
    })
}

pub fn join_adjacent_segment_offsets(
    first: SegmentOffset,
    second: SegmentOffset,
) -> Result<OffsetRailJoin, GeometryError> {
    match intersect_lines(first.start, first.end, second.start, second.end)? {
        LineIntersection::Point(intersection) => Ok(OffsetRailJoin::Point {
            point: intersection.point,
            first_on_segment: in_unit_range(intersection.first_t),
            second_on_segment: in_unit_range(intersection.second_t),
        }),
        LineIntersection::Parallel => Ok(OffsetRailJoin::Parallel),
        LineIntersection::Collinear => Ok(OffsetRailJoin::Collinear),
    }
}

pub fn offset_open_polyline_segments(
    points: &[Point2],
    distance: f64,
) -> Result<Vec<SegmentOffset>, GeometryError> {
    if points.len() < 2 {
        validate_points(points)?;
        if !distance.is_finite() {
            return Err(GeometryError::NonFiniteParameter);
        }
        return Ok(Vec::new());
    }

    let mut offsets = Vec::with_capacity(points.len().saturating_sub(1));
    for segment in points.windows(2) {
        let [start, end] = segment else {
            continue;
        };
        offsets.push(offset_segment(*start, *end, distance)?);
    }

    Ok(offsets)
}

fn validate_points(points: &[Point2]) -> Result<(), GeometryError> {
    points.iter().try_for_each(|point| point.validate())
}

fn in_unit_range(t: f64) -> bool {
    (0.0..=1.0).contains(&t)
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

    #[test]
    fn offsets_horizontal_segment_to_left() {
        let offset = offset_segment(point(0.0, 0.0), point(10.0, 0.0), 2.0).expect("valid offset");

        assert_eq!(offset.source_start, point(0.0, 0.0));
        assert_eq!(offset.source_end, point(10.0, 0.0));
        assert_eq!(offset.start, point(0.0, 2.0));
        assert_eq!(offset.end, point(10.0, 2.0));
        assert_eq!(offset.unit_normal, point(0.0, 1.0));
        assert_eq!(offset.distance, 2.0);
    }

    #[test]
    fn negative_distance_offsets_to_right() {
        let offset = offset_segment(point(0.0, 0.0), point(10.0, 0.0), -3.0).expect("valid offset");

        assert_eq!(offset.start, point(0.0, -3.0));
        assert_eq!(offset.end, point(10.0, -3.0));
        assert_eq!(offset.unit_normal, point(0.0, 1.0));
        assert_eq!(offset.distance, -3.0);
    }

    #[test]
    fn offsets_vertical_segment_to_left() {
        let offset = offset_segment(point(0.0, 0.0), point(0.0, 5.0), 2.0).expect("valid offset");

        assert_eq!(offset.start, point(-2.0, 0.0));
        assert_eq!(offset.end, point(-2.0, 5.0));
        assert_eq!(offset.unit_normal, point(-1.0, 0.0));
    }

    #[test]
    fn offsets_diagonal_segment_with_unit_normal() {
        let offset = offset_segment(point(0.0, 0.0), point(3.0, 4.0), 5.0).expect("valid offset");

        assert_point_close(offset.unit_normal, point(-0.8, 0.6));
        assert_point_close(offset.start, point(-4.0, 3.0));
        assert_point_close(offset.end, point(-1.0, 7.0));
    }

    #[test]
    fn zero_distance_preserves_segment() {
        let offset = offset_segment(point(2.0, 3.0), point(5.0, 7.0), 0.0).expect("valid offset");

        assert_eq!(offset.start, point(2.0, 3.0));
        assert_eq!(offset.end, point(5.0, 7.0));
        assert_point_close(offset.unit_normal, point(-0.8, 0.6));
    }

    #[test]
    fn offsets_open_polyline_segments_without_joining() {
        let offsets = offset_open_polyline_segments(
            &[point(0.0, 0.0), point(10.0, 0.0), point(10.0, 10.0)],
            2.0,
        )
        .expect("valid offsets");

        assert_eq!(offsets.len(), 2);
        assert_eq!(offsets[0].start, point(0.0, 2.0));
        assert_eq!(offsets[0].end, point(10.0, 2.0));
        assert_eq!(offsets[1].start, point(8.0, 0.0));
        assert_eq!(offsets[1].end, point(8.0, 10.0));
        assert_ne!(offsets[0].end, offsets[1].start);
    }

    #[test]
    fn joins_perpendicular_adjacent_offsets_at_point() {
        let offsets = offset_open_polyline_segments(
            &[point(0.0, 0.0), point(10.0, 0.0), point(10.0, 10.0)],
            2.0,
        )
        .expect("valid offsets");

        assert_eq!(
            join_adjacent_segment_offsets(offsets[0], offsets[1]),
            Ok(OffsetRailJoin::Point {
                point: point(8.0, 2.0),
                first_on_segment: true,
                second_on_segment: true,
            })
        );
    }

    #[test]
    fn join_reports_when_point_is_outside_finite_rails() {
        let first = offset_segment(point(0.0, 0.0), point(10.0, 0.0), 0.0).expect("valid offset");
        let second =
            offset_segment(point(20.0, 10.0), point(20.0, 20.0), 0.0).expect("valid offset");

        assert_eq!(
            join_adjacent_segment_offsets(first, second),
            Ok(OffsetRailJoin::Point {
                point: point(20.0, 0.0),
                first_on_segment: false,
                second_on_segment: false,
            })
        );
    }

    #[test]
    fn parallel_rails_are_classified() {
        let first = offset_segment(point(0.0, 0.0), point(10.0, 0.0), 2.0).expect("valid offset");
        let second = offset_segment(point(0.0, 5.0), point(10.0, 5.0), 2.0).expect("valid offset");

        assert_eq!(
            join_adjacent_segment_offsets(first, second),
            Ok(OffsetRailJoin::Parallel)
        );
    }

    #[test]
    fn collinear_rails_are_classified() {
        let first = offset_segment(point(0.0, 0.0), point(10.0, 0.0), 0.0).expect("valid offset");
        let second = offset_segment(point(10.0, 0.0), point(20.0, 0.0), 0.0).expect("valid offset");

        assert_eq!(
            join_adjacent_segment_offsets(first, second),
            Ok(OffsetRailJoin::Collinear)
        );
    }

    #[test]
    fn joins_are_deterministic_for_same_inputs() {
        let first = offset_segment(point(0.0, 0.0), point(8.0, 2.0), 1.0).expect("valid offset");
        let second = offset_segment(point(8.0, 2.0), point(10.0, 9.0), 1.0).expect("valid offset");

        assert_eq!(
            join_adjacent_segment_offsets(first, second),
            join_adjacent_segment_offsets(first, second)
        );
    }

    #[test]
    fn empty_and_single_point_polylines_return_empty() {
        assert_eq!(
            offset_open_polyline_segments(&[], 2.0),
            Ok(Vec::<SegmentOffset>::new())
        );
        assert_eq!(
            offset_open_polyline_segments(&[point(0.0, 0.0)], 2.0),
            Ok(Vec::<SegmentOffset>::new())
        );
    }

    #[test]
    fn rejects_invalid_inputs() {
        assert_eq!(
            offset_segment(point(0.0, 0.0), point(1.0, 0.0), f64::NAN),
            Err(GeometryError::NonFiniteParameter)
        );
        assert_eq!(
            offset_segment(point(f64::NAN, 0.0), point(1.0, 0.0), 1.0),
            Err(GeometryError::NonFinitePoint)
        );
        assert_eq!(
            offset_segment(point(0.0, 0.0), point(0.0, 0.0), 1.0),
            Err(GeometryError::DegenerateLine)
        );
        assert_eq!(
            offset_open_polyline_segments(&[point(0.0, 0.0), point(0.0, 0.0)], 1.0),
            Err(GeometryError::DegenerateLine)
        );
        assert_eq!(
            offset_open_polyline_segments(&[point(0.0, 0.0)], f64::INFINITY),
            Err(GeometryError::NonFiniteParameter)
        );
    }
}
