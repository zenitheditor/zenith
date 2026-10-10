use crate::math;
use crate::{
    ClosedPolyline, CompoundFillRule, CompoundPathGeometry, FilledContourBoundaryRole,
    GeometryError, Point2, PointLocation, PolylineProjection, RectBounds,
    classify_closed_polyline_fill_topology,
    contour::{bounds_for, segment_points, signed_area_for},
    fill_topology::winding_delta,
    project_onto_polyline,
    validation::validate_tolerance,
};

/// Contract flatten tolerance for path fill/outline consumption (document units).
pub const PATH_CONSUMPTION_TOLERANCE: f64 = 0.25;

/// Locates `point` against the filled region of a compound path.
///
/// Only closed contours contribute. Open subpaths are ignored. Empty or open-only
/// geometry is `Outside`. Boundary (within `tolerance` of any closed edge) wins.
/// NonZero sums winding of containing rings; EvenOdd uses parity of containing rings.
pub fn locate_point_in_compound_path(
    geometry: &CompoundPathGeometry,
    point: Point2,
    rule: CompoundFillRule,
    tolerance: f64,
) -> Result<PointLocation, GeometryError> {
    point.validate()?;
    validate_tolerance(tolerance)?;

    let rings = closed_rings_from_compound(geometry, tolerance)?;
    if rings.is_empty() {
        return Ok(PointLocation::Outside);
    }

    let mut winding_number = 0_i32;
    let mut inside_count = 0_usize;

    for ring in &rings {
        match ring.locate_point(point, tolerance)? {
            PointLocation::Boundary => {
                // Boundary wins over fill classification; stop scanning remaining rings.
                return Ok(PointLocation::Boundary);
            }
            PointLocation::Inside => {
                inside_count = inside_count.saturating_add(1);
                winding_number = winding_number.saturating_add(winding_delta(ring.winding()));
            }
            PointLocation::Outside => {}
        }
    }

    match rule {
        CompoundFillRule::NonZero => {
            if winding_number != 0 {
                Ok(PointLocation::Inside)
            } else {
                Ok(PointLocation::Outside)
            }
        }
        CompoundFillRule::EvenOdd => {
            if inside_count % 2 == 1 {
                Ok(PointLocation::Inside)
            } else {
                Ok(PointLocation::Outside)
            }
        }
    }
}

/// Extrema-aware axis-aligned bounds of closed fill contours only.
///
/// Open subpaths are ignored. Returns `None` when there is no closed contour.
/// `tolerance` is validated for API consistency with other consumption queries
/// but is unused for extrema bounds (curve extrema come from path segments).
pub fn compound_path_fill_bounds(
    geometry: &CompoundPathGeometry,
    tolerance: f64,
) -> Result<Option<RectBounds>, GeometryError> {
    validate_tolerance(tolerance)?;

    let mut bounds: Option<RectBounds> = None;
    for contour in geometry.contours() {
        if !contour.closed() {
            continue;
        }
        let Some(contour_bounds) = contour.bounds()? else {
            continue;
        };
        bounds = Some(match bounds {
            Some(existing) => existing.include_bounds(contour_bounds),
            None => contour_bounds,
        });
    }

    Ok(bounds)
}

/// Samples the exterior outline of a compound path at divided-anchor fraction `index/count`.
///
/// Exterior = outermost paint ring after fill-topology classification on closed contours
/// only (open subpaths are filtered first). Walk is CW in y-down coordinates starting at
/// the AABB top-mid projection onto the ring.
pub fn sample_outline_perimeter(
    geometry: &CompoundPathGeometry,
    index: usize,
    count: usize,
    tolerance: f64,
    rule: CompoundFillRule,
) -> Result<Point2, GeometryError> {
    if count == 0 {
        return Err(GeometryError::NonPositiveCount);
    }
    validate_tolerance(tolerance)?;

    let rings = closed_rings_from_compound(geometry, tolerance)?;
    if rings.is_empty() {
        return Err(GeometryError::InvalidContour);
    }

    let exterior_index = select_exterior_ring_index(&rings, rule, tolerance)?;
    let Some(exterior) = rings.get(exterior_index) else {
        return Err(GeometryError::InvalidContour);
    };

    sample_closed_polyline_perimeter(exterior.points(), index, count)
}

/// Samples a closed polygon ring at fraction `index/count` of perimeter length.
///
/// Start is the AABB top-mid projected onto the ring; walk is CW in y-down
/// (top mid → +x first, matching box divided anchors).
pub fn sample_closed_polyline_perimeter(
    points: &[Point2],
    index: usize,
    count: usize,
) -> Result<Point2, GeometryError> {
    if count == 0 {
        return Err(GeometryError::NonPositiveCount);
    }
    validate_polyline_points(points)?;
    if points.len() < 3 {
        return Err(GeometryError::CountOutOfRange);
    }

    let oriented = orient_closed_cw_y_down(points)?;
    let perimeter = closed_ring_length(&oriented)?;
    if perimeter <= 0.0 || !perimeter.is_finite() {
        return Err(GeometryError::InvalidContour);
    }

    let mut projection_ring = oriented.clone();
    let Some(first) = oriented.first().copied() else {
        return Err(GeometryError::CountOutOfRange);
    };
    projection_ring.push(first);

    let bounds = bounds_for(&oriented)?;
    let top_mid = Point2::new(bounds.center_x(), bounds.min_y)?;
    let projection =
        project_onto_polyline(top_mid, &projection_ring)?.ok_or(GeometryError::InvalidContour)?;

    let start_arc = arc_length_to_projection(&oriented, &projection)?;
    let fraction = index as f64 / count as f64;
    if !fraction.is_finite() {
        return Err(GeometryError::CountOutOfRange);
    }

    let mut target = start_arc + perimeter * fraction;
    if !target.is_finite() {
        return Err(GeometryError::CountOutOfRange);
    }
    target %= perimeter;
    if target < 0.0 {
        target += perimeter;
    }
    // Floating-point: treat a full wrap as the start point.
    if target >= perimeter {
        target = 0.0;
    }

    point_at_distance_closed(&oriented, target)
}

/// Samples an open polyline at divided-anchor fraction along its path length (start→end).
///
/// Open walks use inclusive endpoints: `index/(count-1)` for `count > 1` (so `0/2` is
/// the start and `1/2` is the end). Closed rings use `index/count` instead so the loop
/// does not double-count the start vertex.
pub fn sample_open_polyline_perimeter(
    points: &[Point2],
    index: usize,
    count: usize,
) -> Result<Point2, GeometryError> {
    if count == 0 {
        return Err(GeometryError::NonPositiveCount);
    }
    validate_polyline_points(points)?;
    if points.len() < 2 {
        return Err(GeometryError::CountOutOfRange);
    }

    let total = open_polyline_length(points)?;
    if total <= 0.0 || !total.is_finite() {
        let Some(first) = points.first().copied() else {
            return Err(GeometryError::CountOutOfRange);
        };
        return Ok(first);
    }

    let fraction = if count == 1 {
        0.0
    } else {
        index as f64 / (count - 1) as f64
    };
    if !fraction.is_finite() {
        return Err(GeometryError::CountOutOfRange);
    }

    let target = total * fraction;
    if !target.is_finite() {
        return Err(GeometryError::CountOutOfRange);
    }

    point_at_distance_open(points, target)
}

fn closed_rings_from_compound(
    geometry: &CompoundPathGeometry,
    tolerance: f64,
) -> Result<Vec<ClosedPolyline>, GeometryError> {
    let contours = geometry.contours();
    let mut rings = Vec::with_capacity(contours.len());
    for contour in contours {
        if !contour.closed() {
            continue;
        }
        if let Some(ring) = ClosedPolyline::from_path(contour, tolerance)? {
            rings.push(ring);
        }
    }
    Ok(rings)
}

fn select_exterior_ring_index(
    rings: &[ClosedPolyline],
    rule: CompoundFillRule,
    tolerance: f64,
) -> Result<usize, GeometryError> {
    match classify_closed_polyline_fill_topology(rings, rule, tolerance) {
        Ok(topology) => {
            let mut best: Option<(usize, f64)> = None;
            for contour in &topology.contours {
                if contour.depth != 0 {
                    continue;
                }
                match contour.role {
                    FilledContourBoundaryRole::Paint => {}
                    FilledContourBoundaryRole::Hole | FilledContourBoundaryRole::NoFillChange => {
                        continue;
                    }
                }

                let Some(ring) = rings.get(contour.contour_index) else {
                    return Err(GeometryError::InvalidContour);
                };
                let area = ring.signed_area().abs();
                best = prefer_largest_area(best, contour.contour_index, area);
            }

            if let Some((index, _)) = best {
                Ok(index)
            } else {
                largest_area_ring_index(rings)
            }
        }
        Err(_) => largest_area_ring_index(rings),
    }
}

fn largest_area_ring_index(rings: &[ClosedPolyline]) -> Result<usize, GeometryError> {
    let mut best: Option<(usize, f64)> = None;
    for (index, ring) in rings.iter().enumerate() {
        let area = ring.signed_area().abs();
        best = prefer_largest_area(best, index, area);
    }
    best.map(|(index, _)| index)
        .ok_or(GeometryError::InvalidContour)
}

/// Keep the larger absolute area; on ties prefer the lower index (deterministic).
fn prefer_largest_area(
    best: Option<(usize, f64)>,
    index: usize,
    area: f64,
) -> Option<(usize, f64)> {
    match best {
        Some((best_index, best_area))
            if area < best_area || (area == best_area && index >= best_index) =>
        {
            Some((best_index, best_area))
        }
        Some(_) | None => Some((index, area)),
    }
}

fn orient_closed_cw_y_down(points: &[Point2]) -> Result<Vec<Point2>, GeometryError> {
    let signed_area = signed_area_for(points)?;
    if signed_area == 0.0 {
        return Err(GeometryError::InvalidContour);
    }

    let mut oriented = points.to_vec();
    // Positive signed area is math CCW, which is CW when y increases downward.
    if signed_area < 0.0 {
        oriented.reverse();
    }
    Ok(oriented)
}

fn closed_ring_length(points: &[Point2]) -> Result<f64, GeometryError> {
    let mut length = 0.0;
    for index in 0..points.len() {
        let Some((start, end)) = segment_points(points, index) else {
            continue;
        };
        length += segment_length(start, end);
        if !length.is_finite() {
            return Err(GeometryError::CountOutOfRange);
        }
    }
    Ok(length)
}

fn open_polyline_length(points: &[Point2]) -> Result<f64, GeometryError> {
    let mut length = 0.0;
    for segment in points.windows(2) {
        let Some(start) = segment.first().copied() else {
            continue;
        };
        let Some(end) = segment.get(1).copied() else {
            continue;
        };
        length += segment_length(start, end);
        if !length.is_finite() {
            return Err(GeometryError::CountOutOfRange);
        }
    }
    Ok(length)
}

fn arc_length_to_projection(
    oriented: &[Point2],
    projection: &PolylineProjection,
) -> Result<f64, GeometryError> {
    if projection.segment_index >= oriented.len() {
        return Err(GeometryError::CountOutOfRange);
    }

    let mut length = 0.0;
    for index in 0..projection.segment_index {
        let Some((start, end)) = segment_points(oriented, index) else {
            continue;
        };
        length += segment_length(start, end);
        if !length.is_finite() {
            return Err(GeometryError::CountOutOfRange);
        }
    }

    let Some((start, end)) = segment_points(oriented, projection.segment_index) else {
        return Err(GeometryError::CountOutOfRange);
    };
    let segment_len = segment_length(start, end);
    let t = projection.segment_t;
    if !t.is_finite() {
        return Err(GeometryError::ParameterOutOfRange);
    }
    length += segment_len * t.clamp(0.0, 1.0);
    if length.is_finite() {
        Ok(length)
    } else {
        Err(GeometryError::CountOutOfRange)
    }
}

fn point_at_distance_closed(points: &[Point2], mut distance: f64) -> Result<Point2, GeometryError> {
    let segment_count = points.len();
    if segment_count == 0 {
        return Err(GeometryError::CountOutOfRange);
    }

    for index in 0..segment_count {
        let Some((start, end)) = segment_points(points, index) else {
            continue;
        };
        let segment_len = segment_length(start, end);
        let is_last = index + 1 == segment_count;
        if distance <= segment_len || is_last {
            let t = if segment_len <= 0.0 {
                if is_last { 1.0 } else { 0.0 }
            } else {
                (distance / segment_len).clamp(0.0, 1.0)
            };
            return Ok(start.lerp(end, t));
        }
        distance -= segment_len;
    }

    points
        .first()
        .copied()
        .ok_or(GeometryError::CountOutOfRange)
}

fn point_at_distance_open(points: &[Point2], mut distance: f64) -> Result<Point2, GeometryError> {
    let Some(last_segment_index) = points.len().checked_sub(2) else {
        return Err(GeometryError::CountOutOfRange);
    };

    for (index, segment) in points.windows(2).enumerate() {
        let Some(start) = segment.first().copied() else {
            continue;
        };
        let Some(end) = segment.get(1).copied() else {
            continue;
        };
        let segment_len = segment_length(start, end);
        let is_last = index == last_segment_index;
        if distance <= segment_len || is_last {
            let t = if segment_len <= 0.0 {
                if is_last { 1.0 } else { 0.0 }
            } else {
                (distance / segment_len).clamp(0.0, 1.0)
            };
            return Ok(start.lerp(end, t));
        }
        distance -= segment_len;
    }

    points.last().copied().ok_or(GeometryError::CountOutOfRange)
}

fn segment_length(start: Point2, end: Point2) -> f64 {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    math::hypot(dx, dy)
}

fn validate_polyline_points(points: &[Point2]) -> Result<(), GeometryError> {
    for point in points {
        point.validate()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PathAnchor, PathGeometry};

    const TOLERANCE: f64 = PATH_CONSUMPTION_TOLERANCE;

    #[test]
    fn locate_square_inside_outside_boundary() {
        let geometry = CompoundPathGeometry::new(vec![square_path(0.0, 0.0, 10.0)]);

        assert_eq!(
            locate_point_in_compound_path(
                &geometry,
                point(5.0, 5.0),
                CompoundFillRule::NonZero,
                TOLERANCE,
            ),
            Ok(PointLocation::Inside)
        );
        assert_eq!(
            locate_point_in_compound_path(
                &geometry,
                point(15.0, 5.0),
                CompoundFillRule::NonZero,
                TOLERANCE,
            ),
            Ok(PointLocation::Outside)
        );
        assert_eq!(
            locate_point_in_compound_path(
                &geometry,
                point(5.0, 0.0),
                CompoundFillRule::NonZero,
                TOLERANCE,
            ),
            Ok(PointLocation::Boundary)
        );
    }

    #[test]
    fn locate_and_fill_bounds_ignore_open_subpaths() {
        let geometry = CompoundPathGeometry::new(vec![
            open_path(&[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)]),
            square_path(0.0, 0.0, 10.0),
        ]);

        assert_eq!(
            locate_point_in_compound_path(
                &geometry,
                point(50.0, 1.0),
                CompoundFillRule::NonZero,
                TOLERANCE,
            ),
            Ok(PointLocation::Outside)
        );
        assert_eq!(
            locate_point_in_compound_path(
                &geometry,
                point(5.0, 5.0),
                CompoundFillRule::NonZero,
                TOLERANCE,
            ),
            Ok(PointLocation::Inside)
        );

        let bounds = compound_path_fill_bounds(&geometry, TOLERANCE)
            .expect("bounds")
            .expect("closed bounds");
        assert_eq!(bounds.min_x, 0.0);
        assert_eq!(bounds.min_y, 0.0);
        assert_eq!(bounds.max_x, 10.0);
        assert_eq!(bounds.max_y, 10.0);
    }

    #[test]
    fn evenodd_nested_hole_is_outside() {
        let geometry = CompoundPathGeometry::new(vec![
            square_path(0.0, 0.0, 30.0),
            square_path(5.0, 5.0, 20.0),
        ]);

        assert_eq!(
            locate_point_in_compound_path(
                &geometry,
                point(15.0, 15.0),
                CompoundFillRule::EvenOdd,
                TOLERANCE,
            ),
            Ok(PointLocation::Outside)
        );
        assert_eq!(
            locate_point_in_compound_path(
                &geometry,
                point(2.0, 2.0),
                CompoundFillRule::EvenOdd,
                TOLERANCE,
            ),
            Ok(PointLocation::Inside)
        );
    }

    #[test]
    fn nonzero_opposite_winding_hole_is_outside() {
        let geometry = CompoundPathGeometry::new(vec![
            square_path(0.0, 0.0, 20.0),
            reversed_square_path(5.0, 5.0, 10.0),
        ]);

        assert_eq!(
            locate_point_in_compound_path(
                &geometry,
                point(10.0, 10.0),
                CompoundFillRule::NonZero,
                TOLERANCE,
            ),
            Ok(PointLocation::Outside)
        );
        assert_eq!(
            locate_point_in_compound_path(
                &geometry,
                point(2.0, 2.0),
                CompoundFillRule::NonZero,
                TOLERANCE,
            ),
            Ok(PointLocation::Inside)
        );
    }

    #[test]
    fn fill_bounds_use_cubic_extrema_not_control_hull() {
        // Closed path: top edge bows upward via a cubic whose control hull reaches y=-20,
        // but the curve extrema only reach y=-15.
        let geometry = CompoundPathGeometry::new(vec![
            PathGeometry::new(
                vec![
                    PathAnchor::new(point(0.0, 0.0), None, Some(point(0.0, -20.0)))
                        .expect("anchor"),
                    PathAnchor::new(point(10.0, 0.0), Some(point(10.0, -20.0)), None)
                        .expect("anchor"),
                    PathAnchor::new(point(10.0, 10.0), None, None).expect("anchor"),
                    PathAnchor::new(point(0.0, 10.0), None, None).expect("anchor"),
                ],
                true,
            )
            .expect("path"),
        ]);

        let bounds = compound_path_fill_bounds(&geometry, TOLERANCE)
            .expect("bounds")
            .expect("closed bounds");

        assert_eq!(bounds.min_x, 0.0);
        assert_eq!(bounds.max_x, 10.0);
        assert_eq!(bounds.max_y, 10.0);
        // Extrema-aware peak is -15; control hull would be -20.
        assert!((bounds.min_y - (-15.0)).abs() < 1e-9);
        assert!(bounds.min_y > -20.0 + 1e-9);
    }

    #[test]
    fn fill_bounds_none_when_no_closed_contour() {
        let geometry = CompoundPathGeometry::new(vec![open_path(&[(0.0, 0.0), (10.0, 5.0)])]);

        assert_eq!(compound_path_fill_bounds(&geometry, TOLERANCE), Ok(None));
        assert_eq!(
            compound_path_fill_bounds(&CompoundPathGeometry::new(Vec::new()), TOLERANCE),
            Ok(None)
        );
    }

    #[test]
    fn closed_square_quarter_samples_match_box_midpoints() {
        let square = [
            point(0.0, 0.0),
            point(10.0, 0.0),
            point(10.0, 10.0),
            point(0.0, 10.0),
        ];

        assert_eq!(
            sample_closed_polyline_perimeter(&square, 0, 4),
            Ok(point(5.0, 0.0))
        );
        assert_eq!(
            sample_closed_polyline_perimeter(&square, 1, 4),
            Ok(point(10.0, 5.0))
        );
        assert_eq!(
            sample_closed_polyline_perimeter(&square, 2, 4),
            Ok(point(5.0, 10.0))
        );
        assert_eq!(
            sample_closed_polyline_perimeter(&square, 3, 4),
            Ok(point(0.0, 5.0))
        );
    }

    #[test]
    fn open_polyline_samples_start_and_end() {
        let line = [point(0.0, 0.0), point(10.0, 0.0)];

        assert_eq!(
            sample_open_polyline_perimeter(&line, 0, 2),
            Ok(point(0.0, 0.0))
        );
        assert_eq!(
            sample_open_polyline_perimeter(&line, 1, 2),
            Ok(point(10.0, 0.0))
        );
    }

    #[test]
    fn count_zero_errors() {
        let square = [
            point(0.0, 0.0),
            point(10.0, 0.0),
            point(10.0, 10.0),
            point(0.0, 10.0),
        ];
        let line = [point(0.0, 0.0), point(10.0, 0.0)];
        let geometry = CompoundPathGeometry::new(vec![square_path(0.0, 0.0, 10.0)]);

        assert_eq!(
            sample_closed_polyline_perimeter(&square, 0, 0),
            Err(GeometryError::NonPositiveCount)
        );
        assert_eq!(
            sample_open_polyline_perimeter(&line, 0, 0),
            Err(GeometryError::NonPositiveCount)
        );
        assert_eq!(
            sample_outline_perimeter(&geometry, 0, 0, TOLERANCE, CompoundFillRule::NonZero),
            Err(GeometryError::NonPositiveCount)
        );
    }

    #[test]
    fn exterior_outline_is_not_the_hole_for_donut() {
        let geometry = CompoundPathGeometry::new(vec![
            square_path(0.0, 0.0, 30.0),
            square_path(10.0, 10.0, 10.0),
        ]);

        // Exterior square quarters: top/right/bottom/left midpoints of the outer ring.
        assert_eq!(
            sample_outline_perimeter(&geometry, 0, 4, TOLERANCE, CompoundFillRule::EvenOdd),
            Ok(point(15.0, 0.0))
        );
        assert_eq!(
            sample_outline_perimeter(&geometry, 1, 4, TOLERANCE, CompoundFillRule::EvenOdd),
            Ok(point(30.0, 15.0))
        );
        assert_eq!(
            sample_outline_perimeter(&geometry, 2, 4, TOLERANCE, CompoundFillRule::EvenOdd),
            Ok(point(15.0, 30.0))
        );
        assert_eq!(
            sample_outline_perimeter(&geometry, 3, 4, TOLERANCE, CompoundFillRule::EvenOdd),
            Ok(point(0.0, 15.0))
        );
    }

    fn square_path(x: f64, y: f64, size: f64) -> PathGeometry {
        path(
            &[(x, y), (x + size, y), (x + size, y + size), (x, y + size)],
            true,
        )
    }

    fn reversed_square_path(x: f64, y: f64, size: f64) -> PathGeometry {
        path(
            &[(x, y), (x, y + size), (x + size, y + size), (x + size, y)],
            true,
        )
    }

    fn open_path(points: &[(f64, f64)]) -> PathGeometry {
        path(points, false)
    }

    fn path(points: &[(f64, f64)], closed: bool) -> PathGeometry {
        PathGeometry::new(anchors(points), closed).expect("valid path")
    }

    fn anchors(points: &[(f64, f64)]) -> Vec<PathAnchor> {
        points
            .iter()
            .map(|(x, y)| PathAnchor::new(point(*x, *y), None, None).expect("valid anchor"))
            .collect()
    }

    fn point(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).expect("valid point")
    }
}
