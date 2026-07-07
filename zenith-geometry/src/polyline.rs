use crate::{GeometryError, Point2, validation::validate_tolerance};

pub fn simplify_polyline(points: &[Point2], tolerance: f64) -> Result<Vec<Point2>, GeometryError> {
    validate_tolerance(tolerance)?;
    validate_points(points)?;

    if points.len() <= 2 {
        return Ok(points.to_vec());
    }

    let tolerance_squared = tolerance * tolerance;
    let mut keep = vec![false; points.len()];
    if let Some(first) = keep.first_mut() {
        *first = true;
    }
    if let Some(last) = keep.last_mut() {
        *last = true;
    }

    let mut stack = Vec::new();
    let Some(last_index) = points.len().checked_sub(1) else {
        return Ok(points.to_vec());
    };
    stack.push((0_usize, last_index));

    while let Some((start, end)) = stack.pop() {
        let Some(start_point) = points.get(start).copied() else {
            continue;
        };
        let Some(end_point) = points.get(end).copied() else {
            continue;
        };

        let Some(interior_start) = start.checked_add(1) else {
            continue;
        };
        let Some(interior_count) = end.checked_sub(interior_start) else {
            continue;
        };

        let mut farthest = None;
        let mut farthest_distance_squared = tolerance_squared;

        for (index, point) in points
            .iter()
            .enumerate()
            .skip(interior_start)
            .take(interior_count)
        {
            let distance_squared = point.distance_squared_to_segment(start_point, end_point);
            if distance_squared > farthest_distance_squared {
                farthest = Some(index);
                farthest_distance_squared = distance_squared;
            }
        }

        if let Some(split) = farthest {
            if let Some(slot) = keep.get_mut(split) {
                *slot = true;
            }
            stack.push((split, end));
            stack.push((start, split));
        }
    }

    Ok(points
        .iter()
        .zip(keep.iter())
        .filter_map(|(point, should_keep)| should_keep.then_some(*point))
        .collect())
}

fn validate_points(points: &[Point2]) -> Result<(), GeometryError> {
    points.iter().try_for_each(|point| point.validate())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> Point2 {
        Point2::new_unchecked(x, y)
    }

    #[test]
    fn empty_one_and_two_point_inputs_round_trip() {
        assert_eq!(simplify_polyline(&[], 0.1), Ok(Vec::new()));

        let one = vec![point(1.0, 2.0)];
        assert_eq!(simplify_polyline(&one, 0.1), Ok(one.clone()));

        let two = vec![point(1.0, 2.0), point(3.0, 4.0)];
        assert_eq!(simplify_polyline(&two, 0.1), Ok(two));
    }

    #[test]
    fn removes_near_collinear_middle_point() {
        let points = vec![point(0.0, 0.0), point(5.0, 0.01), point(10.0, 0.0)];

        assert_eq!(
            simplify_polyline(&points, 0.1),
            Ok(vec![point(0.0, 0.0), point(10.0, 0.0)])
        );
    }

    #[test]
    fn preserves_far_middle_point() {
        let points = vec![point(0.0, 0.0), point(5.0, 2.0), point(10.0, 0.0)];

        assert_eq!(simplify_polyline(&points, 0.1), Ok(points));
    }

    #[test]
    fn preserves_reversal_beyond_current_segment() {
        let points = vec![point(0.0, 0.0), point(2.0, 0.0), point(1.0, 0.0)];

        assert_eq!(simplify_polyline(&points, 0.1), Ok(points));
    }

    #[test]
    fn propagates_invalid_point() {
        let points = vec![point(0.0, 0.0), point(f64::NAN, 1.0), point(2.0, 0.0)];

        assert_eq!(
            simplify_polyline(&points, 0.1),
            Err(GeometryError::NonFinitePoint)
        );
    }

    #[test]
    fn propagates_invalid_tolerance() {
        let points = vec![point(0.0, 0.0), point(1.0, 1.0)];

        assert_eq!(
            simplify_polyline(&points, f64::NAN),
            Err(GeometryError::NonFiniteTolerance)
        );
        assert_eq!(
            simplify_polyline(&points, 0.0),
            Err(GeometryError::NonPositiveTolerance)
        );
    }

    #[test]
    fn preserves_endpoints_and_order() {
        let points = vec![
            point(0.0, 0.0),
            point(1.0, 0.01),
            point(2.0, 3.0),
            point(3.0, 0.01),
            point(4.0, 0.0),
        ];
        let simplified = simplify_polyline(&points, 0.1).expect("valid simplification");

        assert_eq!(simplified.first(), points.first());
        assert_eq!(simplified.last(), points.last());

        let mut input = points.iter();
        for simplified_point in &simplified {
            assert!(input.any(|point| point == simplified_point));
        }
    }
}
