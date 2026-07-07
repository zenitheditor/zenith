use crate::{
    ClosedPolyline, GeometryError, OffsetRailJoin, PathGeometry, Point2, SegmentOffset,
    join_adjacent_segment_offsets, offset_open_polyline_segments, offset_segment,
    validation::validate_tolerance,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenPolylineCap {
    Butt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenPolylineJoin {
    FiniteRailIntersection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosedPolylineJoin {
    FiniteRailIntersection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenPolylineOutlinePolicy {
    pub cap: OpenPolylineCap,
    pub join: OpenPolylineJoin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClosedPolylineOutlinePolicy {
    pub join: ClosedPolylineJoin,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenPolylineOutline {
    pub points: Vec<Point2>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClosedPolylineOutline {
    pub left_ring: Vec<Point2>,
    pub right_ring: Vec<Point2>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PathOutline {
    Open(OpenPolylineOutline),
    Closed(ClosedPolylineOutline),
}

impl Default for OpenPolylineOutlinePolicy {
    fn default() -> Self {
        Self {
            cap: OpenPolylineCap::Butt,
            join: OpenPolylineJoin::FiniteRailIntersection,
        }
    }
}

impl Default for ClosedPolylineOutlinePolicy {
    fn default() -> Self {
        Self {
            join: ClosedPolylineJoin::FiniteRailIntersection,
        }
    }
}

pub fn outline_open_polyline(
    points: &[Point2],
    stroke_width: f64,
    policy: OpenPolylineOutlinePolicy,
) -> Result<Option<OpenPolylineOutline>, GeometryError> {
    validate_points(points)?;
    validate_policy(policy);

    if !stroke_width.is_finite() {
        return Err(GeometryError::NonFiniteParameter);
    }
    if stroke_width < 0.0 {
        return Err(GeometryError::CountOutOfRange);
    }
    if stroke_width == 0.0 || points.len() < 2 {
        return Ok(None);
    }

    let half_width = stroke_width * 0.5;
    if !half_width.is_finite() {
        return Err(GeometryError::CountOutOfRange);
    }

    let left_rails = offset_open_polyline_segments(points, half_width)?;
    let right_rails = offset_open_polyline_segments(points, -half_width)?;
    let mut outline = Vec::with_capacity(left_rails.len().saturating_mul(4));

    append_side(&mut outline, &left_rails, SideDirection::Forward)?;
    append_side(&mut outline, &right_rails, SideDirection::Reverse)?;
    remove_closing_duplicate(&mut outline);

    if outline.len() < 3 {
        Ok(None)
    } else {
        Ok(Some(OpenPolylineOutline { points: outline }))
    }
}

pub fn outline_closed_polyline(
    contour: &ClosedPolyline,
    stroke_width: f64,
    policy: ClosedPolylineOutlinePolicy,
) -> Result<Option<ClosedPolylineOutline>, GeometryError> {
    validate_closed_policy(policy);
    if !stroke_width.is_finite() {
        return Err(GeometryError::NonFiniteParameter);
    }
    if stroke_width < 0.0 {
        return Err(GeometryError::CountOutOfRange);
    }
    if stroke_width == 0.0 {
        return Ok(None);
    }

    let half_width = stroke_width * 0.5;
    if !half_width.is_finite() {
        return Err(GeometryError::CountOutOfRange);
    }

    let left_rails = offset_closed_polyline_segments(contour.points(), half_width)?;
    let right_rails = offset_closed_polyline_segments(contour.points(), -half_width)?;
    let left_ring = closed_ring_from_rails(&left_rails)?;
    let right_ring = closed_ring_from_rails(&right_rails)?;

    if left_ring.len() < 3 || right_ring.len() < 3 {
        Ok(None)
    } else {
        Ok(Some(ClosedPolylineOutline {
            left_ring,
            right_ring,
        }))
    }
}

pub fn outline_path_geometry(
    path: &PathGeometry,
    tolerance: f64,
    stroke_width: f64,
    open_policy: OpenPolylineOutlinePolicy,
    closed_policy: ClosedPolylineOutlinePolicy,
) -> Result<Option<PathOutline>, GeometryError> {
    validate_tolerance(tolerance)?;

    if path.closed() {
        let Some(contour) = ClosedPolyline::from_path(path, tolerance)? else {
            return Ok(None);
        };
        return Ok(
            outline_closed_polyline(&contour, stroke_width, closed_policy)?
                .map(PathOutline::Closed),
        );
    }

    Ok(
        outline_open_polyline(&path.flatten(tolerance)?, stroke_width, open_policy)?
            .map(PathOutline::Open),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SideDirection {
    Forward,
    Reverse,
}

fn append_side(
    outline: &mut Vec<Point2>,
    rails: &[SegmentOffset],
    direction: SideDirection,
) -> Result<(), GeometryError> {
    match direction {
        SideDirection::Forward => append_forward_side(outline, rails),
        SideDirection::Reverse => append_reverse_side(outline, rails),
    }
}

fn append_forward_side(
    outline: &mut Vec<Point2>,
    rails: &[SegmentOffset],
) -> Result<(), GeometryError> {
    let Some(first) = rails.first() else {
        return Ok(());
    };
    push_unique(outline, first.start);

    for pair in rails.windows(2) {
        if let [first, second] = pair {
            append_forward_bridge_join(outline, *first, *second)?;
        }
    }

    if let Some(last) = rails.last() {
        push_unique(outline, last.end);
    }
    Ok(())
}

fn append_reverse_side(
    outline: &mut Vec<Point2>,
    rails: &[SegmentOffset],
) -> Result<(), GeometryError> {
    let Some(last) = rails.last() else {
        return Ok(());
    };
    push_unique(outline, last.end);

    for pair in rails.windows(2).rev() {
        if let [first, second] = pair {
            append_reverse_join(outline, *second, *first)?;
        }
    }

    if let Some(first) = rails.first() {
        push_unique(outline, first.start);
    }
    Ok(())
}

fn append_forward_bridge_join(
    outline: &mut Vec<Point2>,
    first: SegmentOffset,
    second: SegmentOffset,
) -> Result<(), GeometryError> {
    match join_adjacent_segment_offsets(first, second)? {
        OffsetRailJoin::Point {
            point,
            first_on_segment: true,
            second_on_segment: true,
        } => push_unique(outline, point),
        OffsetRailJoin::Point { .. } | OffsetRailJoin::Parallel => {
            push_unique(outline, first.end);
            push_unique(outline, second.start);
        }
        OffsetRailJoin::Collinear => {}
    }
    Ok(())
}

fn append_reverse_join(
    outline: &mut Vec<Point2>,
    first: SegmentOffset,
    second: SegmentOffset,
) -> Result<(), GeometryError> {
    match join_adjacent_segment_offsets(first, second)? {
        OffsetRailJoin::Point {
            point,
            first_on_segment: true,
            second_on_segment: true,
        } => push_unique(outline, point),
        OffsetRailJoin::Point { .. } | OffsetRailJoin::Parallel => {
            push_unique(outline, first.start);
            push_unique(outline, second.end);
        }
        OffsetRailJoin::Collinear => {}
    }
    Ok(())
}

fn closed_ring_from_rails(rails: &[SegmentOffset]) -> Result<Vec<Point2>, GeometryError> {
    let mut ring = Vec::with_capacity(rails.len());
    for index in 0..rails.len() {
        let Some(previous) = previous_rail(rails, index) else {
            continue;
        };
        let Some(current) = rails.get(index).copied() else {
            continue;
        };
        append_forward_bridge_join(&mut ring, previous, current)?;
    }
    remove_closing_duplicate(&mut ring);
    Ok(ring)
}

fn previous_rail(rails: &[SegmentOffset], index: usize) -> Option<SegmentOffset> {
    if index == 0 {
        rails.last().copied()
    } else {
        rails.get(index.saturating_sub(1)).copied()
    }
}

fn offset_closed_polyline_segments(
    points: &[Point2],
    distance: f64,
) -> Result<Vec<SegmentOffset>, GeometryError> {
    let mut offsets = Vec::with_capacity(points.len());
    for index in 0..points.len() {
        let Some(start) = points.get(index).copied() else {
            continue;
        };
        let next_index = if index + 1 == points.len() {
            0
        } else {
            index + 1
        };
        let Some(end) = points.get(next_index).copied() else {
            continue;
        };
        offsets.push(offset_segment(start, end, distance)?);
    }
    Ok(offsets)
}

fn push_unique(points: &mut Vec<Point2>, point: Point2) {
    if points.last().copied() != Some(point) {
        points.push(point);
    }
}

fn remove_closing_duplicate(points: &mut Vec<Point2>) {
    let Some(first) = points.first().copied() else {
        return;
    };
    if points.last().copied() == Some(first) {
        points.pop();
    }
}

fn validate_points(points: &[Point2]) -> Result<(), GeometryError> {
    points.iter().try_for_each(|point| point.validate())
}

fn validate_policy(policy: OpenPolylineOutlinePolicy) {
    match policy.cap {
        OpenPolylineCap::Butt => {}
    }
    match policy.join {
        OpenPolylineJoin::FiniteRailIntersection => {}
    }
}

fn validate_closed_policy(policy: ClosedPolylineOutlinePolicy) {
    match policy.join {
        ClosedPolylineJoin::FiniteRailIntersection => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> Point2 {
        Point2::new_unchecked(x, y)
    }

    fn outline(
        points: &[Point2],
        stroke_width: f64,
    ) -> Result<Option<OpenPolylineOutline>, GeometryError> {
        outline_open_polyline(points, stroke_width, OpenPolylineOutlinePolicy::default())
    }

    fn closed_contour(points: &[Point2]) -> ClosedPolyline {
        ClosedPolyline::new(points.to_vec()).expect("valid contour")
    }

    fn closed_outline(
        points: &[Point2],
        stroke_width: f64,
    ) -> Result<Option<ClosedPolylineOutline>, GeometryError> {
        outline_closed_polyline(
            &closed_contour(points),
            stroke_width,
            ClosedPolylineOutlinePolicy::default(),
        )
    }

    fn anchor(x: f64, y: f64) -> crate::PathAnchor {
        crate::PathAnchor::new(point(x, y), None, None).expect("valid anchor")
    }

    fn path_outline(
        path: &PathGeometry,
        tolerance: f64,
        stroke_width: f64,
    ) -> Result<Option<PathOutline>, GeometryError> {
        outline_path_geometry(
            path,
            tolerance,
            stroke_width,
            OpenPolylineOutlinePolicy::default(),
            ClosedPolylineOutlinePolicy::default(),
        )
    }

    #[test]
    fn empty_single_and_zero_width_inputs_return_none() {
        assert_eq!(outline(&[], 2.0), Ok(None));
        assert_eq!(outline(&[point(0.0, 0.0)], 2.0), Ok(None));
        assert_eq!(outline(&[point(0.0, 0.0), point(10.0, 0.0)], 0.0), Ok(None));
    }

    #[test]
    fn rejects_invalid_widths_and_points() {
        assert_eq!(
            outline(&[point(0.0, 0.0), point(10.0, 0.0)], f64::NAN),
            Err(GeometryError::NonFiniteParameter)
        );
        assert_eq!(
            outline(&[point(0.0, 0.0), point(10.0, 0.0)], -1.0),
            Err(GeometryError::CountOutOfRange)
        );
        assert_eq!(
            outline(&[point(f64::NAN, 0.0)], 0.0),
            Err(GeometryError::NonFinitePoint)
        );
        assert_eq!(
            outline(&[point(0.0, 0.0), point(0.0, 0.0)], 2.0),
            Err(GeometryError::DegenerateLine)
        );
    }

    #[test]
    fn one_segment_produces_clockwise_butt_cap_ring() {
        let outline = outline(&[point(0.0, 0.0), point(10.0, 0.0)], 4.0)
            .expect("valid outline")
            .expect("outline");

        assert_eq!(
            outline.points,
            vec![
                point(0.0, 2.0),
                point(10.0, 2.0),
                point(10.0, -2.0),
                point(0.0, -2.0),
            ]
        );
    }

    #[test]
    fn collinear_polyline_suppresses_duplicate_join_points() {
        let outline = outline(&[point(0.0, 0.0), point(10.0, 0.0), point(20.0, 0.0)], 4.0)
            .expect("valid outline")
            .expect("outline");

        assert_eq!(
            outline.points,
            vec![
                point(0.0, 2.0),
                point(20.0, 2.0),
                point(20.0, -2.0),
                point(0.0, -2.0),
            ]
        );
    }

    #[test]
    fn perpendicular_bend_uses_inner_intersection_and_outer_bridge() {
        let outline = outline(&[point(0.0, 0.0), point(10.0, 0.0), point(10.0, 10.0)], 4.0)
            .expect("valid outline")
            .expect("outline");

        assert_eq!(
            outline.points,
            vec![
                point(0.0, 2.0),
                point(8.0, 2.0),
                point(8.0, 10.0),
                point(12.0, 10.0),
                point(12.0, 0.0),
                point(10.0, -2.0),
                point(0.0, -2.0),
            ]
        );
    }

    #[test]
    fn u_turn_bridges_parallel_rails_deterministically() {
        let outline = outline(&[point(0.0, 0.0), point(10.0, 0.0), point(0.0, 0.0)], 4.0)
            .expect("valid outline")
            .expect("outline");

        assert_eq!(
            outline.points,
            vec![
                point(0.0, 2.0),
                point(10.0, 2.0),
                point(10.0, -2.0),
                point(0.0, -2.0),
                point(0.0, 2.0),
                point(10.0, 2.0),
                point(10.0, -2.0),
                point(0.0, -2.0),
            ]
        );
    }

    #[test]
    fn repeated_call_is_deterministic() {
        let points = [point(0.0, 0.0), point(6.0, 2.0), point(8.0, 9.0)];

        assert_eq!(outline(&points, 3.0), outline(&points, 3.0));
    }

    #[test]
    fn closed_zero_width_returns_none_and_invalid_widths_error() {
        assert_eq!(
            closed_outline(
                &[
                    point(0.0, 0.0),
                    point(10.0, 0.0),
                    point(10.0, 10.0),
                    point(0.0, 10.0),
                ],
                0.0
            ),
            Ok(None)
        );
        assert_eq!(
            closed_outline(
                &[
                    point(0.0, 0.0),
                    point(10.0, 0.0),
                    point(10.0, 10.0),
                    point(0.0, 10.0),
                ],
                f64::NAN
            ),
            Err(GeometryError::NonFiniteParameter)
        );
        assert_eq!(
            closed_outline(
                &[
                    point(0.0, 0.0),
                    point(10.0, 0.0),
                    point(10.0, 10.0),
                    point(0.0, 10.0),
                ],
                -1.0
            ),
            Err(GeometryError::CountOutOfRange)
        );
    }

    #[test]
    fn closed_rectangle_returns_left_and_right_offset_rings() {
        let outline = closed_outline(
            &[
                point(0.0, 0.0),
                point(10.0, 0.0),
                point(10.0, 10.0),
                point(0.0, 10.0),
            ],
            4.0,
        )
        .expect("valid outline")
        .expect("outline");

        assert_eq!(
            outline.left_ring,
            vec![
                point(2.0, 2.0),
                point(8.0, 2.0),
                point(8.0, 8.0),
                point(2.0, 8.0),
            ]
        );
        assert_eq!(
            outline.right_ring,
            vec![
                point(-2.0, 0.0),
                point(0.0, -2.0),
                point(10.0, -2.0),
                point(12.0, 0.0),
                point(12.0, 10.0),
                point(10.0, 12.0),
                point(0.0, 12.0),
                point(-2.0, 10.0),
            ]
        );
    }

    #[test]
    fn closed_concave_outline_keeps_deterministic_left_and_right_rings() {
        let outline = closed_outline(
            &[
                point(0.0, 0.0),
                point(8.0, 0.0),
                point(8.0, 4.0),
                point(4.0, 4.0),
                point(4.0, 8.0),
                point(0.0, 8.0),
            ],
            2.0,
        )
        .expect("valid outline")
        .expect("outline");

        assert_eq!(
            outline.left_ring,
            vec![
                point(1.0, 1.0),
                point(7.0, 1.0),
                point(7.0, 3.0),
                point(4.0, 3.0),
                point(3.0, 4.0),
                point(3.0, 7.0),
                point(1.0, 7.0),
            ]
        );
        assert_eq!(
            outline.right_ring,
            vec![
                point(-1.0, 0.0),
                point(0.0, -1.0),
                point(8.0, -1.0),
                point(9.0, 0.0),
                point(9.0, 4.0),
                point(8.0, 5.0),
                point(5.0, 5.0),
                point(5.0, 8.0),
                point(4.0, 9.0),
                point(0.0, 9.0),
                point(-1.0, 8.0),
            ]
        );
    }

    #[test]
    fn repeated_closed_call_is_deterministic() {
        let points = [
            point(0.0, 0.0),
            point(6.0, 2.0),
            point(8.0, 9.0),
            point(1.0, 6.0),
        ];

        assert_eq!(closed_outline(&points, 3.0), closed_outline(&points, 3.0));
    }

    #[test]
    fn path_outline_dispatches_open_paths_to_open_outline() {
        let path = PathGeometry::new(vec![anchor(0.0, 0.0), anchor(10.0, 0.0)], false)
            .expect("valid path");

        assert_eq!(
            path_outline(&path, 0.25, 4.0),
            Ok(Some(PathOutline::Open(OpenPolylineOutline {
                points: vec![
                    point(0.0, 2.0),
                    point(10.0, 2.0),
                    point(10.0, -2.0),
                    point(0.0, -2.0),
                ],
            })))
        );
    }

    #[test]
    fn path_outline_dispatches_closed_paths_to_closed_outline() {
        let path = PathGeometry::new(
            vec![
                anchor(0.0, 0.0),
                anchor(10.0, 0.0),
                anchor(10.0, 10.0),
                anchor(0.0, 10.0),
            ],
            true,
        )
        .expect("valid path");

        assert_eq!(
            path_outline(&path, 0.25, 4.0),
            Ok(Some(PathOutline::Closed(ClosedPolylineOutline {
                left_ring: vec![
                    point(2.0, 2.0),
                    point(8.0, 2.0),
                    point(8.0, 8.0),
                    point(2.0, 8.0),
                ],
                right_ring: vec![
                    point(-2.0, 0.0),
                    point(0.0, -2.0),
                    point(10.0, -2.0),
                    point(12.0, 0.0),
                    point(12.0, 10.0),
                    point(10.0, 12.0),
                    point(0.0, 12.0),
                    point(-2.0, 10.0),
                ],
            })))
        );
    }

    #[test]
    fn path_outline_preserves_zero_width_and_tolerance_errors() {
        let path = PathGeometry::new(vec![anchor(0.0, 0.0), anchor(10.0, 0.0)], false)
            .expect("valid path");

        assert_eq!(path_outline(&path, 0.25, 0.0), Ok(None));
        assert_eq!(
            path_outline(&path, 0.0, 4.0),
            Err(GeometryError::NonPositiveTolerance)
        );
    }
}
