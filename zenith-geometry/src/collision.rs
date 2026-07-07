use crate::{
    GeometryError, PathGeometry, Point2, PolylineIntersection, PolylineProjection,
    SegmentIntersection, collect_open_polyline_intersections, validation::validate_tolerance,
};

#[derive(Debug, Clone, PartialEq)]
pub struct PathGeometryIntersections {
    pub first_points: Vec<Point2>,
    pub second_points: Vec<Point2>,
    pub intersections: Vec<PolylineIntersection>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathGeometryNearestPoints {
    pub first_point: Point2,
    pub second_point: Point2,
    pub first_segment_index: usize,
    pub second_segment_index: usize,
    pub first_segment_t: f64,
    pub second_segment_t: f64,
    pub distance_squared: f64,
}

pub fn collect_path_geometry_intersections(
    first: &PathGeometry,
    second: &PathGeometry,
    tolerance: f64,
) -> Result<PathGeometryIntersections, GeometryError> {
    validate_tolerance(tolerance)?;
    let first_points = first.flatten(tolerance)?;
    let second_points = second.flatten(tolerance)?;
    let intersections = collect_open_polyline_intersections(&first_points, &second_points)?;

    Ok(PathGeometryIntersections {
        first_points,
        second_points,
        intersections,
    })
}

pub fn nearest_path_geometry_points(
    first: &PathGeometry,
    second: &PathGeometry,
    tolerance: f64,
) -> Result<Option<PathGeometryNearestPoints>, GeometryError> {
    validate_tolerance(tolerance)?;
    let first_points = first.flatten(tolerance)?;
    let second_points = second.flatten(tolerance)?;

    if first_points.len() < 2 || second_points.len() < 2 {
        return Ok(None);
    }

    let intersections = collect_open_polyline_intersections(&first_points, &second_points)?;
    if let Some(nearest) = nearest_intersection(&intersections) {
        return Ok(Some(nearest));
    }

    let mut nearest = None;
    for (point_index, point) in first_points.iter().copied().enumerate() {
        if let Some(projection) = project_onto_flattened_polyline(point, &second_points) {
            update_nearest(
                &mut nearest,
                PathGeometryNearestPoints {
                    first_point: point,
                    second_point: projection.point,
                    first_segment_index: vertex_segment_index(&first_points, point_index),
                    second_segment_index: projection.segment_index,
                    first_segment_t: vertex_segment_t(&first_points, point_index),
                    second_segment_t: projection.segment_t,
                    distance_squared: projection.distance_squared,
                },
            );
        }
    }

    for (point_index, point) in second_points.iter().copied().enumerate() {
        if let Some(projection) = project_onto_flattened_polyline(point, &first_points) {
            update_nearest(
                &mut nearest,
                PathGeometryNearestPoints {
                    first_point: projection.point,
                    second_point: point,
                    first_segment_index: projection.segment_index,
                    second_segment_index: vertex_segment_index(&second_points, point_index),
                    first_segment_t: projection.segment_t,
                    second_segment_t: vertex_segment_t(&second_points, point_index),
                    distance_squared: projection.distance_squared,
                },
            );
        }
    }

    Ok(nearest)
}

fn project_onto_flattened_polyline(
    point: Point2,
    polyline: &[Point2],
) -> Option<PolylineProjection> {
    if polyline.len() < 2 {
        return None;
    }

    let mut nearest: Option<PolylineProjection> = None;
    for (segment_index, segment) in polyline.windows(2).enumerate() {
        let Some(segment_start) = segment.first().copied() else {
            continue;
        };
        let Some(segment_end) = segment.get(1).copied() else {
            continue;
        };
        let projection = point.project_onto_segment(segment_start, segment_end);
        let candidate = PolylineProjection {
            point: projection.point,
            segment_index,
            segment_t: projection.t,
            distance_squared: projection.distance_squared,
        };
        match nearest {
            Some(current) if candidate.distance_squared >= current.distance_squared => {
                nearest = Some(current);
            }
            Some(_) | None => {
                nearest = Some(candidate);
            }
        }
    }

    nearest
}

fn nearest_intersection(
    intersections: &[PolylineIntersection],
) -> Option<PathGeometryNearestPoints> {
    intersections
        .first()
        .map(|intersection| match intersection.intersection {
            SegmentIntersection::Point(point) => PathGeometryNearestPoints {
                first_point: point.point,
                second_point: point.point,
                first_segment_index: intersection.first_segment_index,
                second_segment_index: intersection.second_segment_index,
                first_segment_t: point.first_t,
                second_segment_t: point.second_t,
                distance_squared: 0.0,
            },
            SegmentIntersection::Overlap { start, .. } => PathGeometryNearestPoints {
                first_point: start.point,
                second_point: start.point,
                first_segment_index: intersection.first_segment_index,
                second_segment_index: intersection.second_segment_index,
                first_segment_t: start.first_t,
                second_segment_t: start.second_t,
                distance_squared: 0.0,
            },
        })
}

fn update_nearest(
    nearest: &mut Option<PathGeometryNearestPoints>,
    candidate: PathGeometryNearestPoints,
) {
    match nearest {
        Some(current) if candidate.distance_squared >= current.distance_squared => {}
        Some(_) | None => *nearest = Some(candidate),
    }
}

fn vertex_segment_index(points: &[Point2], point_index: usize) -> usize {
    if point_index == 0 {
        0
    } else {
        point_index
            .saturating_sub(1)
            .min(points.len().saturating_sub(2))
    }
}

fn vertex_segment_t(points: &[Point2], point_index: usize) -> f64 {
    if point_index == 0 || points.is_empty() {
        0.0
    } else {
        1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CubicBezier, PathAnchor, PathSegment, Point2, SegmentIntersection};

    fn point(x: f64, y: f64) -> Point2 {
        Point2::new_unchecked(x, y)
    }

    fn anchor(x: f64, y: f64) -> PathAnchor {
        PathAnchor::new(point(x, y), None, None).expect("valid anchor")
    }

    #[test]
    fn collects_line_path_intersections() {
        let horizontal =
            PathGeometry::new(vec![anchor(0.0, 0.0), anchor(10.0, 0.0)], false).expect("path");
        let vertical =
            PathGeometry::new(vec![anchor(5.0, -5.0), anchor(5.0, 5.0)], false).expect("path");

        let report = collect_path_geometry_intersections(&horizontal, &vertical, 0.1)
            .expect("intersections");

        assert_eq!(report.first_points, vec![point(0.0, 0.0), point(10.0, 0.0)]);
        assert_eq!(
            report.second_points,
            vec![point(5.0, -5.0), point(5.0, 5.0)]
        );
        assert_eq!(report.intersections.len(), 1);
        assert_eq!(report.intersections[0].first_segment_index, 0);
        assert_eq!(report.intersections[0].second_segment_index, 0);
        assert_eq!(
            report.intersections[0].intersection,
            SegmentIntersection::Point(crate::IntersectionPoint {
                point: point(5.0, 0.0),
                first_t: 0.5,
                second_t: 0.5,
            })
        );
    }

    #[test]
    fn flattens_cubic_paths_before_collecting_intersections() {
        let curve = CubicBezier::new_unchecked(
            point(0.0, 0.0),
            point(0.0, 10.0),
            point(10.0, 10.0),
            point(10.0, 0.0),
        );
        let start = PathAnchor::new(curve.p0, None, Some(curve.p1)).expect("valid anchor");
        let end = PathAnchor::new(curve.p3, Some(curve.p2), None).expect("valid anchor");
        let cubic = PathGeometry::new(vec![start, end], false).expect("path");
        let cutter =
            PathGeometry::new(vec![anchor(5.0, -1.0), anchor(5.0, 9.0)], false).expect("path");

        let report =
            collect_path_geometry_intersections(&cubic, &cutter, 0.5).expect("intersections");

        assert!(report.first_points.len() > 2);
        assert!(
            report.intersections.iter().any(|intersection| matches!(
                intersection.intersection,
                SegmentIntersection::Point(_)
            )),
            "expected at least one point intersection; got {report:?}"
        );
    }

    #[test]
    fn validates_tolerance() {
        let path =
            PathGeometry::new(vec![anchor(0.0, 0.0), anchor(1.0, 0.0)], false).expect("path");

        assert_eq!(
            collect_path_geometry_intersections(&path, &path, 0.0),
            Err(GeometryError::NonPositiveTolerance)
        );
        assert_eq!(
            nearest_path_geometry_points(&path, &path, 0.0),
            Err(GeometryError::NonPositiveTolerance)
        );
    }

    #[test]
    fn empty_paths_have_no_intersections() {
        let empty = PathGeometry::new(Vec::new(), false).expect("path");
        let line =
            PathGeometry::new(vec![anchor(0.0, 0.0), anchor(1.0, 0.0)], false).expect("path");

        assert_eq!(
            collect_path_geometry_intersections(&empty, &line, 0.1)
                .expect("intersections")
                .intersections,
            Vec::new()
        );
    }

    #[test]
    fn keeps_overlap_intersections() {
        let first =
            PathGeometry::new(vec![anchor(0.0, 0.0), anchor(10.0, 0.0)], false).expect("path");
        let second =
            PathGeometry::new(vec![anchor(4.0, 0.0), anchor(8.0, 0.0)], false).expect("path");

        let report =
            collect_path_geometry_intersections(&first, &second, 0.1).expect("intersections");

        assert!(matches!(
            report.intersections[0].intersection,
            SegmentIntersection::Overlap { .. }
        ));
    }

    #[test]
    fn uses_path_segments_without_mutating_inputs() {
        let path =
            PathGeometry::new(vec![anchor(0.0, 0.0), anchor(1.0, 0.0)], false).expect("path");
        let original_segments = path.segments().expect("segments");

        let _ = collect_path_geometry_intersections(&path, &path, 0.1).expect("intersections");

        assert_eq!(path.segments().expect("segments"), original_segments);
        assert_eq!(
            original_segments,
            vec![PathSegment::Line {
                start: point(0.0, 0.0),
                end: point(1.0, 0.0),
            }]
        );
    }

    #[test]
    fn nearest_points_return_none_for_underdefined_paths() {
        let empty = PathGeometry::new(Vec::new(), false).expect("path");
        let line =
            PathGeometry::new(vec![anchor(0.0, 0.0), anchor(1.0, 0.0)], false).expect("path");

        assert_eq!(nearest_path_geometry_points(&empty, &line, 0.1), Ok(None));
    }

    #[test]
    fn nearest_points_prefer_true_intersection() {
        let horizontal =
            PathGeometry::new(vec![anchor(0.0, 0.0), anchor(10.0, 0.0)], false).expect("path");
        let vertical =
            PathGeometry::new(vec![anchor(5.0, -5.0), anchor(5.0, 5.0)], false).expect("path");

        assert_eq!(
            nearest_path_geometry_points(&horizontal, &vertical, 0.1),
            Ok(Some(PathGeometryNearestPoints {
                first_point: point(5.0, 0.0),
                second_point: point(5.0, 0.0),
                first_segment_index: 0,
                second_segment_index: 0,
                first_segment_t: 0.5,
                second_segment_t: 0.5,
                distance_squared: 0.0,
            }))
        );
    }

    #[test]
    fn nearest_points_project_endpoint_to_opposite_path() {
        let first =
            PathGeometry::new(vec![anchor(0.0, 0.0), anchor(4.0, 0.0)], false).expect("path");
        let second =
            PathGeometry::new(vec![anchor(6.0, 3.0), anchor(10.0, 3.0)], false).expect("path");

        assert_eq!(
            nearest_path_geometry_points(&first, &second, 0.1),
            Ok(Some(PathGeometryNearestPoints {
                first_point: point(4.0, 0.0),
                second_point: point(6.0, 3.0),
                first_segment_index: 0,
                second_segment_index: 0,
                first_segment_t: 1.0,
                second_segment_t: 0.0,
                distance_squared: 13.0,
            }))
        );
    }
}
