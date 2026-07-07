use crate::{CubicBezier, GeometryError, Point2};

const MIN_HANDLE_SCALE: f64 = 1.0e-6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolylineEndpointTangentDirections {
    pub start: Point2,
    pub end: Point2,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolylineCubicFit {
    pub curve: CubicBezier,
    pub max_error_point_index: usize,
    pub max_error_squared: f64,
}

pub fn chord_length_parameters(points: &[Point2]) -> Result<Option<Vec<f64>>, GeometryError> {
    validate_points(points)?;
    if points.len() < 2 {
        return Ok(None);
    }

    let mut distances = Vec::with_capacity(points.len());
    distances.push(0.0);
    let mut total = 0.0;

    for segment in points.windows(2) {
        let Some(start) = segment.first().copied() else {
            continue;
        };
        let Some(end) = segment.get(1).copied() else {
            continue;
        };
        let length = segment_length(start, end)?;
        total += length;
        if !total.is_finite() {
            return Err(GeometryError::CountOutOfRange);
        }
        distances.push(total);
    }

    if total == 0.0 {
        return Err(GeometryError::DegenerateLine);
    }

    Ok(Some(
        distances
            .into_iter()
            .map(|distance| distance / total)
            .collect(),
    ))
}

pub fn estimate_endpoint_tangent_directions(
    points: &[Point2],
) -> Result<Option<PolylineEndpointTangentDirections>, GeometryError> {
    validate_points(points)?;
    if points.len() < 2 {
        return Ok(None);
    }

    let start = first_non_zero_direction(points)?.ok_or(GeometryError::DegenerateLine)?;
    let end = last_non_zero_direction(points)?.ok_or(GeometryError::DegenerateLine)?;
    Ok(Some(PolylineEndpointTangentDirections { start, end }))
}

pub fn fit_cubic_bezier_to_points(
    points: &[Point2],
) -> Result<Option<PolylineCubicFit>, GeometryError> {
    let Some(parameters) = chord_length_parameters(points)? else {
        return Ok(None);
    };
    let Some(tangents) = estimate_endpoint_tangent_directions(points)? else {
        return Ok(None);
    };
    let total_length = total_chord_length(points)?;

    let Some(start) = points.first().copied() else {
        return Ok(None);
    };
    let Some(end) = points.last().copied() else {
        return Ok(None);
    };

    let handle_lengths = solve_cubic_handle_lengths(points, &parameters, start, end, tangents)
        .unwrap_or_else(|| fallback_handle_lengths(total_length));
    let first_handle = translate(start, tangents.start, handle_lengths.start);
    let second_handle = translate(end, tangents.end, handle_lengths.end);
    let curve = CubicBezier::new(start, first_handle, second_handle, end)?;
    let (max_error_point_index, max_error_squared) = maximum_fit_error(points, &parameters, curve)?;

    Ok(Some(PolylineCubicFit {
        curve,
        max_error_point_index,
        max_error_squared,
    }))
}

#[derive(Debug, Clone, Copy)]
struct CubicHandleLengths {
    start: f64,
    end: f64,
}

#[derive(Debug, Clone, Copy)]
struct BezierBasis {
    b0: f64,
    b1: f64,
    b2: f64,
    b3: f64,
}

fn solve_cubic_handle_lengths(
    points: &[Point2],
    parameters: &[f64],
    start: Point2,
    end: Point2,
    tangents: PolylineEndpointTangentDirections,
) -> Option<CubicHandleLengths> {
    let mut c00 = 0.0;
    let mut c01 = 0.0;
    let mut c11 = 0.0;
    let mut x0 = 0.0;
    let mut x1 = 0.0;

    for (point, t) in points.iter().copied().zip(parameters.iter().copied()) {
        let basis = bezier_basis(t);
        let a1 = scale(tangents.start, basis.b1);
        let a2 = scale(tangents.end, basis.b2);
        let baseline = add(
            scale(start, basis.b0 + basis.b1),
            scale(end, basis.b2 + basis.b3),
        );
        let target = subtract(point, baseline);

        c00 += dot(a1, a1);
        c01 += dot(a1, a2);
        c11 += dot(a2, a2);
        x0 += dot(a1, target);
        x1 += dot(a2, target);
    }

    let determinant = c00.mul_add(c11, -(c01 * c01));
    if determinant == 0.0 || !determinant.is_finite() {
        return None;
    }

    let start_length = (x0 * c11 - x1 * c01) / determinant;
    let end_length = (c00 * x1 - c01 * x0) / determinant;
    if valid_handle_length(start_length) && valid_handle_length(end_length) {
        Some(CubicHandleLengths {
            start: start_length,
            end: end_length,
        })
    } else {
        None
    }
}

fn fallback_handle_lengths(total_length: f64) -> CubicHandleLengths {
    let length = total_length / 3.0;
    CubicHandleLengths {
        start: length,
        end: length,
    }
}

fn valid_handle_length(length: f64) -> bool {
    length.is_finite() && length > MIN_HANDLE_SCALE
}

fn maximum_fit_error(
    points: &[Point2],
    parameters: &[f64],
    curve: CubicBezier,
) -> Result<(usize, f64), GeometryError> {
    let mut max_index = 0;
    let mut max_error = 0.0;

    for (index, (point, t)) in points
        .iter()
        .copied()
        .zip(parameters.iter().copied())
        .enumerate()
    {
        let error = point.distance_squared(curve.evaluate(t));
        if !error.is_finite() {
            return Err(GeometryError::CountOutOfRange);
        }
        if error > max_error {
            max_index = index;
            max_error = error;
        }
    }

    Ok((max_index, max_error))
}

fn bezier_basis(t: f64) -> BezierBasis {
    let one_minus_t = 1.0 - t;
    let one_minus_t_squared = one_minus_t * one_minus_t;
    let t_squared = t * t;

    BezierBasis {
        b0: one_minus_t_squared * one_minus_t,
        b1: 3.0 * t * one_minus_t_squared,
        b2: 3.0 * t_squared * one_minus_t,
        b3: t_squared * t,
    }
}

fn first_non_zero_direction(points: &[Point2]) -> Result<Option<Point2>, GeometryError> {
    let Some(origin) = points.first().copied() else {
        return Ok(None);
    };
    for point in points.iter().copied().skip(1) {
        if let Some(direction) = unit_direction(origin, point)? {
            return Ok(Some(direction));
        }
    }
    Ok(None)
}

fn last_non_zero_direction(points: &[Point2]) -> Result<Option<Point2>, GeometryError> {
    let Some(origin) = points.last().copied() else {
        return Ok(None);
    };
    for point in points.iter().copied().rev().skip(1) {
        if let Some(direction) = unit_direction(origin, point)? {
            return Ok(Some(direction));
        }
    }
    Ok(None)
}

fn unit_direction(start: Point2, end: Point2) -> Result<Option<Point2>, GeometryError> {
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
        return Ok(None);
    }

    Point2::new(dx / length, dy / length).map(Some)
}

fn segment_length(start: Point2, end: Point2) -> Result<f64, GeometryError> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    if !dx.is_finite() || !dy.is_finite() {
        return Err(GeometryError::CountOutOfRange);
    }

    let length = dx.hypot(dy);
    if length.is_finite() {
        Ok(length)
    } else {
        Err(GeometryError::CountOutOfRange)
    }
}

fn total_chord_length(points: &[Point2]) -> Result<f64, GeometryError> {
    let mut total = 0.0;
    for segment in points.windows(2) {
        let Some(start) = segment.first().copied() else {
            continue;
        };
        let Some(end) = segment.get(1).copied() else {
            continue;
        };
        total += segment_length(start, end)?;
        if !total.is_finite() {
            return Err(GeometryError::CountOutOfRange);
        }
    }
    if total == 0.0 {
        Err(GeometryError::DegenerateLine)
    } else {
        Ok(total)
    }
}

fn validate_points(points: &[Point2]) -> Result<(), GeometryError> {
    points.iter().try_for_each(|point| point.validate())
}

fn add(a: Point2, b: Point2) -> Point2 {
    Point2::new_unchecked(a.x + b.x, a.y + b.y)
}

fn subtract(a: Point2, b: Point2) -> Point2 {
    Point2::new_unchecked(a.x - b.x, a.y - b.y)
}

fn scale(point: Point2, scale: f64) -> Point2 {
    Point2::new_unchecked(point.x * scale, point.y * scale)
}

fn translate(point: Point2, direction: Point2, distance: f64) -> Point2 {
    add(point, scale(direction, distance))
}

fn dot(a: Point2, b: Point2) -> f64 {
    a.x.mul_add(b.x, a.y * b.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> Point2 {
        Point2::new_unchecked(x, y)
    }

    fn assert_point_close(actual: Point2, expected: Point2) {
        let distance_squared = actual.distance_squared(expected);
        assert!(
            distance_squared <= 1.0e-24,
            "expected {actual:?} to be close to {expected:?}, distance squared {distance_squared}"
        );
    }

    #[test]
    fn empty_and_single_point_inputs_return_none() {
        assert_eq!(chord_length_parameters(&[]), Ok(None));
        assert_eq!(chord_length_parameters(&[point(0.0, 0.0)]), Ok(None));
    }

    #[test]
    fn parameterizes_by_cumulative_chord_length() {
        assert_eq!(
            chord_length_parameters(&[
                point(0.0, 0.0),
                point(3.0, 4.0),
                point(6.0, 4.0),
                point(6.0, 8.0),
            ]),
            Ok(Some(vec![0.0, 5.0 / 12.0, 8.0 / 12.0, 1.0]))
        );
    }

    #[test]
    fn repeated_interior_points_preserve_parameter_plateaus() {
        assert_eq!(
            chord_length_parameters(&[
                point(0.0, 0.0),
                point(4.0, 0.0),
                point(4.0, 0.0),
                point(8.0, 0.0),
            ]),
            Ok(Some(vec![0.0, 0.5, 0.5, 1.0]))
        );
    }

    #[test]
    fn all_repeated_points_are_degenerate() {
        assert_eq!(
            chord_length_parameters(&[point(1.0, 1.0), point(1.0, 1.0)]),
            Err(GeometryError::DegenerateLine)
        );
    }

    #[test]
    fn rejects_non_finite_points() {
        assert_eq!(
            chord_length_parameters(&[point(0.0, 0.0), point(f64::NAN, 1.0)]),
            Err(GeometryError::NonFinitePoint)
        );
    }

    #[test]
    fn endpoint_tangent_directions_return_none_for_underdefined_input() {
        assert_eq!(estimate_endpoint_tangent_directions(&[]), Ok(None));
        assert_eq!(
            estimate_endpoint_tangent_directions(&[point(0.0, 0.0)]),
            Ok(None)
        );
    }

    #[test]
    fn endpoint_tangent_directions_use_unit_vectors_away_from_endpoints() {
        assert_eq!(
            estimate_endpoint_tangent_directions(&[
                point(0.0, 0.0),
                point(3.0, 4.0),
                point(6.0, 4.0)
            ]),
            Ok(Some(PolylineEndpointTangentDirections {
                start: point(0.6, 0.8),
                end: point(-1.0, 0.0),
            }))
        );
    }

    #[test]
    fn endpoint_tangent_directions_skip_repeated_endpoint_runs() {
        assert_eq!(
            estimate_endpoint_tangent_directions(&[
                point(0.0, 0.0),
                point(0.0, 0.0),
                point(4.0, 0.0),
                point(8.0, 0.0),
                point(8.0, 0.0),
            ]),
            Ok(Some(PolylineEndpointTangentDirections {
                start: point(1.0, 0.0),
                end: point(-1.0, 0.0),
            }))
        );
    }

    #[test]
    fn endpoint_tangent_directions_reject_zero_total_length_and_non_finite_points() {
        assert_eq!(
            estimate_endpoint_tangent_directions(&[point(1.0, 1.0), point(1.0, 1.0)]),
            Err(GeometryError::DegenerateLine)
        );
        assert_eq!(
            estimate_endpoint_tangent_directions(&[point(0.0, 0.0), point(f64::NAN, 1.0)]),
            Err(GeometryError::NonFinitePoint)
        );
    }

    #[test]
    fn cubic_fit_returns_none_for_underdefined_input() {
        assert_eq!(fit_cubic_bezier_to_points(&[]), Ok(None));
        assert_eq!(fit_cubic_bezier_to_points(&[point(0.0, 0.0)]), Ok(None));
    }

    #[test]
    fn cubic_fit_uses_stable_two_point_fallback_handles() {
        let fit = fit_cubic_bezier_to_points(&[point(0.0, 0.0), point(3.0, 0.0)])
            .expect("valid fit")
            .expect("enough points");

        assert_eq!(
            fit,
            PolylineCubicFit {
                curve: CubicBezier::new_unchecked(
                    point(0.0, 0.0),
                    point(1.0, 0.0),
                    point(2.0, 0.0),
                    point(3.0, 0.0),
                ),
                max_error_point_index: 0,
                max_error_squared: 0.0,
            }
        );
        assert_eq!(fit.curve.derivative(1.0), point(3.0, 0.0));
    }

    #[test]
    fn cubic_fit_preserves_collinear_samples() {
        let fit = fit_cubic_bezier_to_points(&[
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(2.0, 0.0),
            point(3.0, 0.0),
        ])
        .expect("valid fit")
        .expect("enough points");

        assert_point_close(fit.curve.p0, point(0.0, 0.0));
        assert_point_close(fit.curve.p1, point(1.0, 0.0));
        assert_point_close(fit.curve.p2, point(2.0, 0.0));
        assert_point_close(fit.curve.p3, point(3.0, 0.0));
        assert!(fit.max_error_squared <= 1.0e-24);
    }

    #[test]
    fn cubic_fit_tolerates_repeated_interior_samples() {
        let fit = fit_cubic_bezier_to_points(&[
            point(0.0, 0.0),
            point(1.0, 0.0),
            point(1.0, 0.0),
            point(2.0, 0.0),
            point(3.0, 0.0),
        ])
        .expect("valid fit")
        .expect("enough points");

        assert_eq!(fit.curve.p0, point(0.0, 0.0));
        assert_eq!(fit.curve.p3, point(3.0, 0.0));
        assert!(fit.curve.derivative(0.0).x > 0.0);
        assert!(fit.curve.derivative(1.0).x > 0.0);
        assert!(fit.max_error_squared.is_finite());
    }

    #[test]
    fn cubic_fit_falls_back_when_solve_points_handles_backward() {
        let fit = fit_cubic_bezier_to_points(&[
            point(0.0, 0.0),
            point(1.0, 4.0),
            point(2.0, -4.0),
            point(3.0, 0.0),
        ])
        .expect("valid fit")
        .expect("enough points");

        assert!(fit.curve.p1.x > fit.curve.p0.x);
        assert!(fit.curve.p2.x < fit.curve.p3.x);
        assert!(fit.max_error_squared.is_finite());
    }

    #[test]
    fn cubic_fit_rejects_non_finite_solve_results() {
        assert_eq!(
            fit_cubic_bezier_to_points(&[
                point(f64::MAX, 0.0),
                point(0.0, f64::MAX),
                point(-f64::MAX, 0.0),
            ]),
            Err(GeometryError::CountOutOfRange)
        );
    }

    #[test]
    fn cubic_fit_reports_largest_residual_point() {
        let fit = fit_cubic_bezier_to_points(&[
            point(0.0, 0.0),
            point(1.0, 1.0),
            point(2.0, 1.0),
            point(3.0, 0.0),
            point(4.0, 2.0),
        ])
        .expect("valid fit")
        .expect("enough points");

        assert_eq!(fit.curve.p0, point(0.0, 0.0));
        assert_eq!(fit.curve.p3, point(4.0, 2.0));
        assert!(fit.max_error_point_index > 0);
        assert!(fit.max_error_squared.is_finite());
        assert!(fit.max_error_squared > 0.0);
    }

    #[test]
    fn cubic_fit_rejects_degenerate_and_non_finite_samples() {
        assert_eq!(
            fit_cubic_bezier_to_points(&[point(1.0, 1.0), point(1.0, 1.0)]),
            Err(GeometryError::DegenerateLine)
        );
        assert_eq!(
            fit_cubic_bezier_to_points(&[point(0.0, 0.0), point(f64::NAN, 1.0)]),
            Err(GeometryError::NonFinitePoint)
        );
    }
}
