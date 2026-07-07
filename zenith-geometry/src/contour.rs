use crate::{
    GeometryError, PathGeometry, Point2, PolylineIntersection, RectBounds, SegmentIntersection,
    intersect_segments, validation::validate_tolerance,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ClosedPolyline {
    points: Vec<Point2>,
    bounds: RectBounds,
    signed_area: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosedPolylineWinding {
    Clockwise,
    CounterClockwise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointLocation {
    Inside,
    Outside,
    Boundary,
}

impl ClosedPolyline {
    pub fn new(points: Vec<Point2>) -> Result<Self, GeometryError> {
        let points = normalized_points(points)?;
        if points.len() < 3 {
            return Err(GeometryError::CountOutOfRange);
        }

        validate_edges(&points)?;
        let signed_area = signed_area_for(&points)?;
        if signed_area == 0.0 {
            return Err(GeometryError::InvalidContour);
        }
        validate_simple_contour(&points)?;

        Ok(Self {
            bounds: bounds_for(&points)?,
            points,
            signed_area,
        })
    }

    pub fn from_path(path: &PathGeometry, tolerance: f64) -> Result<Option<Self>, GeometryError> {
        validate_tolerance(tolerance)?;
        if path.topology().closed_subpath_count == 0 {
            return Ok(None);
        }

        Self::new(path.flatten(tolerance)?).map(Some)
    }

    #[must_use]
    pub fn points(&self) -> &[Point2] {
        &self.points
    }

    #[must_use]
    pub fn segment_count(&self) -> usize {
        self.points.len()
    }

    #[must_use]
    pub const fn bounds(&self) -> RectBounds {
        self.bounds
    }

    #[must_use]
    pub const fn signed_area(&self) -> f64 {
        self.signed_area
    }

    #[must_use]
    pub const fn winding(&self) -> ClosedPolylineWinding {
        if self.signed_area < 0.0 {
            ClosedPolylineWinding::Clockwise
        } else {
            ClosedPolylineWinding::CounterClockwise
        }
    }

    pub fn locate_point(
        &self,
        point: Point2,
        tolerance: f64,
    ) -> Result<PointLocation, GeometryError> {
        point.validate()?;
        validate_tolerance(tolerance)?;
        let tolerance_squared = tolerance * tolerance;
        if !tolerance_squared.is_finite() {
            return Err(GeometryError::CountOutOfRange);
        }

        if self.point_is_on_boundary(point, tolerance_squared) {
            return Ok(PointLocation::Boundary);
        }

        if self.point_is_inside(point) {
            Ok(PointLocation::Inside)
        } else {
            Ok(PointLocation::Outside)
        }
    }

    fn point_is_on_boundary(&self, point: Point2, tolerance_squared: f64) -> bool {
        for index in 0..self.segment_count() {
            let Some((start, end)) = segment_points(&self.points, index) else {
                continue;
            };
            if point.distance_squared_to_segment(start, end) <= tolerance_squared {
                return true;
            }
        }
        false
    }

    fn point_is_inside(&self, point: Point2) -> bool {
        let mut inside = false;
        for index in 0..self.segment_count() {
            let Some((start, end)) = segment_points(&self.points, index) else {
                continue;
            };
            if (start.y > point.y) == (end.y > point.y) {
                continue;
            }

            let y_delta = end.y - start.y;
            if y_delta == 0.0 {
                continue;
            }
            let crossing_x = start.x + (point.y - start.y) * (end.x - start.x) / y_delta;
            if point.x < crossing_x {
                inside = !inside;
            }
        }
        inside
    }
}

/// Collects raw segment-pair intersections in deterministic first-segment, then second-segment order.
///
/// This intentionally does not deduplicate shared vertices, merge overlap endpoints, or assign boolean event identity.
/// Boolean and collision layers should build their own normalized event model over this raw substrate.
pub fn collect_raw_closed_polyline_intersections(
    first: &ClosedPolyline,
    second: &ClosedPolyline,
) -> Result<Vec<PolylineIntersection>, GeometryError> {
    let mut intersections = Vec::new();
    for first_segment_index in 0..first.segment_count() {
        let Some((first_start, first_end)) = segment_points(first.points(), first_segment_index)
        else {
            continue;
        };
        for second_segment_index in 0..second.segment_count() {
            let Some((second_start, second_end)) =
                segment_points(second.points(), second_segment_index)
            else {
                continue;
            };
            if let Some(intersection) =
                intersect_segments(first_start, first_end, second_start, second_end)?
            {
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

fn normalized_points(mut points: Vec<Point2>) -> Result<Vec<Point2>, GeometryError> {
    for point in &points {
        point.validate()?;
    }

    if points.len() > 1 && points.first().copied() == points.last().copied() {
        points.pop();
    }
    Ok(points)
}

fn validate_edges(points: &[Point2]) -> Result<(), GeometryError> {
    for index in 0..points.len() {
        let Some((start, end)) = segment_points(points, index) else {
            continue;
        };
        if start == end {
            return Err(GeometryError::DegenerateLine);
        }
    }
    Ok(())
}

fn validate_simple_contour(points: &[Point2]) -> Result<(), GeometryError> {
    for first_index in 0..points.len() {
        let Some((first_start, first_end)) = segment_points(points, first_index) else {
            continue;
        };
        for second_index in first_index.saturating_add(1)..points.len() {
            let Some((second_start, second_end)) = segment_points(points, second_index) else {
                continue;
            };
            let Some(intersection) =
                intersect_segments(first_start, first_end, second_start, second_end)?
            else {
                continue;
            };

            if adjacent_segments(points.len(), first_index, second_index) {
                if adjacent_intersection_is_shared_vertex(
                    points,
                    first_index,
                    second_index,
                    intersection,
                ) {
                    continue;
                }
                return Err(GeometryError::InvalidContour);
            }

            return Err(GeometryError::InvalidContour);
        }
    }
    Ok(())
}

fn adjacent_segments(segment_count: usize, first_index: usize, second_index: usize) -> bool {
    first_index + 1 == second_index || (first_index == 0 && second_index + 1 == segment_count)
}

fn adjacent_intersection_is_shared_vertex(
    points: &[Point2],
    first_index: usize,
    second_index: usize,
    intersection: SegmentIntersection,
) -> bool {
    let SegmentIntersection::Point(point) = intersection else {
        return false;
    };
    shared_vertex(points, first_index, second_index) == Some(point.point)
}

fn shared_vertex(points: &[Point2], first_index: usize, second_index: usize) -> Option<Point2> {
    let (first_start, first_end) = segment_points(points, first_index)?;
    let (second_start, second_end) = segment_points(points, second_index)?;
    if first_start == second_start || first_start == second_end {
        Some(first_start)
    } else if first_end == second_start || first_end == second_end {
        Some(first_end)
    } else {
        None
    }
}

fn bounds_for(points: &[Point2]) -> Result<RectBounds, GeometryError> {
    let first = points
        .first()
        .copied()
        .ok_or(GeometryError::CountOutOfRange)?;
    let mut bounds = RectBounds::from_point(first);
    for point in points.iter().copied().skip(1) {
        bounds = bounds.include_point(point);
    }
    Ok(bounds)
}

fn signed_area_for(points: &[Point2]) -> Result<f64, GeometryError> {
    let mut doubled_area = 0.0;
    for index in 0..points.len() {
        let Some((start, end)) = segment_points(points, index) else {
            continue;
        };
        doubled_area += start.x.mul_add(end.y, -(end.x * start.y));
        if !doubled_area.is_finite() {
            return Err(GeometryError::CountOutOfRange);
        }
    }

    let area = doubled_area * 0.5;
    if area.is_finite() {
        Ok(area)
    } else {
        Err(GeometryError::CountOutOfRange)
    }
}

fn segment_points(points: &[Point2], index: usize) -> Option<(Point2, Point2)> {
    let start = points.get(index).copied()?;
    let next_index = if index + 1 == points.len() {
        0
    } else {
        index + 1
    };
    let end = points.get(next_index).copied()?;
    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IntersectionPoint, PathAnchor, SegmentIntersection};

    fn point(x: f64, y: f64) -> Point2 {
        Point2::new_unchecked(x, y)
    }

    fn contour(points: &[Point2]) -> ClosedPolyline {
        ClosedPolyline::new(points.to_vec()).expect("valid contour")
    }

    #[test]
    fn canonicalization_removes_repeated_closing_point_only() {
        let contour = contour(&[
            point(0.0, 0.0),
            point(4.0, 0.0),
            point(0.0, 4.0),
            point(0.0, 0.0),
        ]);

        assert_eq!(
            contour.points(),
            &[point(0.0, 0.0), point(4.0, 0.0), point(0.0, 4.0)]
        );
        assert_eq!(contour.segment_count(), 3);
    }

    #[test]
    fn winding_uses_signed_area() {
        let ccw = contour(&[point(0.0, 0.0), point(4.0, 0.0), point(0.0, 4.0)]);
        let cw = contour(&[point(0.0, 0.0), point(0.0, 4.0), point(4.0, 0.0)]);

        assert_eq!(ccw.signed_area(), 8.0);
        assert_eq!(ccw.winding(), ClosedPolylineWinding::CounterClockwise);
        assert_eq!(cw.signed_area(), -8.0);
        assert_eq!(cw.winding(), ClosedPolylineWinding::Clockwise);
    }

    #[test]
    fn bounds_include_all_vertices() {
        let contour = contour(&[
            point(-2.0, 3.0),
            point(4.0, -1.0),
            point(8.0, 9.0),
            point(1.0, 12.0),
        ]);

        assert_eq!(
            contour.bounds(),
            RectBounds {
                min_x: -2.0,
                min_y: -1.0,
                max_x: 8.0,
                max_y: 12.0,
            }
        );
    }

    #[test]
    fn locate_point_classifies_square_positions() {
        let contour = contour(&[
            point(0.0, 0.0),
            point(10.0, 0.0),
            point(10.0, 10.0),
            point(0.0, 10.0),
        ]);

        assert_eq!(
            contour.locate_point(point(5.0, 5.0), 0.001),
            Ok(PointLocation::Inside)
        );
        assert_eq!(
            contour.locate_point(point(15.0, 5.0), 0.001),
            Ok(PointLocation::Outside)
        );
        assert_eq!(
            contour.locate_point(point(5.0, 0.0), 0.001),
            Ok(PointLocation::Boundary)
        );
        assert_eq!(
            contour.locate_point(point(10.0, 10.0), 0.001),
            Ok(PointLocation::Boundary)
        );
    }

    #[test]
    fn locate_point_handles_concave_and_horizontal_edges() {
        let contour = contour(&[
            point(0.0, 0.0),
            point(8.0, 0.0),
            point(8.0, 4.0),
            point(4.0, 4.0),
            point(4.0, 8.0),
            point(0.0, 8.0),
        ]);

        assert_eq!(
            contour.locate_point(point(2.0, 2.0), 0.001),
            Ok(PointLocation::Inside)
        );
        assert_eq!(
            contour.locate_point(point(6.0, 6.0), 0.001),
            Ok(PointLocation::Outside)
        );
        assert_eq!(
            contour.locate_point(point(6.0, 4.0), 0.001),
            Ok(PointLocation::Boundary)
        );
    }

    #[test]
    fn intersection_collection_includes_implicit_closing_edges_in_order() {
        let first = contour(&[
            point(0.0, 0.0),
            point(10.0, 0.0),
            point(10.0, 10.0),
            point(0.0, 10.0),
        ]);
        let second = contour(&[point(-2.0, 4.0), point(2.0, 4.0), point(-2.0, 6.0)]);

        assert_eq!(
            collect_raw_closed_polyline_intersections(&first, &second),
            Ok(vec![
                PolylineIntersection {
                    first_segment_index: 3,
                    second_segment_index: 0,
                    intersection: SegmentIntersection::Point(IntersectionPoint {
                        point: point(0.0, 4.0),
                        first_t: 0.6,
                        second_t: 0.5,
                    }),
                },
                PolylineIntersection {
                    first_segment_index: 3,
                    second_segment_index: 1,
                    intersection: SegmentIntersection::Point(IntersectionPoint {
                        point: point(0.0, 5.0),
                        first_t: 0.5,
                        second_t: 0.5,
                    }),
                },
            ])
        );
    }

    #[test]
    fn intersection_collection_preserves_overlaps_and_endpoint_touches() {
        let first = contour(&[
            point(0.0, 0.0),
            point(10.0, 0.0),
            point(10.0, 10.0),
            point(0.0, 10.0),
        ]);
        let second = contour(&[point(4.0, 0.0), point(8.0, 0.0), point(6.0, -3.0)]);

        assert_eq!(
            collect_raw_closed_polyline_intersections(&first, &second),
            Ok(vec![
                PolylineIntersection {
                    first_segment_index: 0,
                    second_segment_index: 0,
                    intersection: SegmentIntersection::Overlap {
                        start: IntersectionPoint {
                            point: point(4.0, 0.0),
                            first_t: 0.4,
                            second_t: 0.0,
                        },
                        end: IntersectionPoint {
                            point: point(8.0, 0.0),
                            first_t: 0.8,
                            second_t: 1.0,
                        },
                    },
                },
                PolylineIntersection {
                    first_segment_index: 0,
                    second_segment_index: 1,
                    intersection: SegmentIntersection::Point(IntersectionPoint {
                        point: point(8.0, 0.0),
                        first_t: 0.8,
                        second_t: 0.0,
                    }),
                },
                PolylineIntersection {
                    first_segment_index: 0,
                    second_segment_index: 2,
                    intersection: SegmentIntersection::Point(IntersectionPoint {
                        point: point(4.0, 0.0),
                        first_t: 0.4,
                        second_t: 1.0,
                    }),
                },
            ])
        );
    }

    #[test]
    fn from_path_flattens_closed_cubic_and_skips_open_paths() {
        let open_path = PathGeometry::new(
            vec![
                PathAnchor::new(point(0.0, 0.0), None, None).expect("anchor"),
                PathAnchor::new(point(10.0, 0.0), None, None).expect("anchor"),
            ],
            false,
        )
        .expect("path");
        assert_eq!(ClosedPolyline::from_path(&open_path, 0.25), Ok(None));

        let closed_path = PathGeometry::new(
            vec![
                PathAnchor::new(point(0.0, 0.0), None, Some(point(3.0, 6.0))).expect("anchor"),
                PathAnchor::new(point(10.0, 0.0), Some(point(7.0, 6.0)), None).expect("anchor"),
                PathAnchor::new(point(5.0, -8.0), None, None).expect("anchor"),
            ],
            true,
        )
        .expect("path");

        let contour = ClosedPolyline::from_path(&closed_path, 0.25)
            .expect("valid contour")
            .expect("closed contour");

        assert!(contour.segment_count() > 3);
        assert_eq!(contour.points().first().copied(), Some(point(0.0, 0.0)));
    }

    #[test]
    fn invalid_inputs_return_stable_errors() {
        assert_eq!(
            ClosedPolyline::new(vec![point(0.0, 0.0), point(10.0, 0.0), point(0.0, 0.0)]),
            Err(GeometryError::CountOutOfRange)
        );
        assert_eq!(
            ClosedPolyline::new(vec![
                point(0.0, 0.0),
                point(10.0, 0.0),
                point(10.0, 0.0),
                point(0.0, 10.0)
            ]),
            Err(GeometryError::DegenerateLine)
        );
        assert_eq!(
            ClosedPolyline::new(vec![point(0.0, 0.0), point(5.0, 0.0), point(10.0, 0.0)]),
            Err(GeometryError::InvalidContour)
        );
        assert_eq!(
            ClosedPolyline::new(vec![
                point(0.0, 0.0),
                point(10.0, 10.0),
                point(0.0, 10.0),
                point(10.0, 0.0)
            ]),
            Err(GeometryError::InvalidContour)
        );
        assert_eq!(
            ClosedPolyline::new(vec![
                point(0.0, 0.0),
                point(10.0, 0.0),
                point(4.0, 0.0),
                point(0.0, 10.0)
            ]),
            Err(GeometryError::InvalidContour)
        );
        assert_eq!(
            ClosedPolyline::new(vec![point(f64::NAN, 0.0), point(1.0, 0.0), point(0.0, 1.0)]),
            Err(GeometryError::NonFinitePoint)
        );

        let contour = contour(&[point(0.0, 0.0), point(1.0, 0.0), point(0.0, 1.0)]);
        assert_eq!(
            contour.locate_point(point(0.1, 0.1), 0.0),
            Err(GeometryError::NonPositiveTolerance)
        );
        assert_eq!(
            contour.locate_point(point(f64::INFINITY, 0.1), 0.1),
            Err(GeometryError::NonFinitePoint)
        );
    }
}
