use crate::{
    ClosedPolyline, ClosedPolylineRelation, GeometryError, classify_closed_polyline_relation,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosedPolylineBooleanOp {
    Union,
    Intersect,
    Subtract,
    Exclude,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClosedPolylineBooleanResult {
    Empty,
    One(ClosedPolyline),
    Two {
        first: ClosedPolyline,
        second: ClosedPolyline,
    },
}

pub fn boolean_closed_polylines(
    first: &ClosedPolyline,
    second: &ClosedPolyline,
    operation: ClosedPolylineBooleanOp,
    tolerance: f64,
) -> Result<Option<ClosedPolylineBooleanResult>, GeometryError> {
    match classify_closed_polyline_relation(first, second, tolerance)? {
        ClosedPolylineRelation::Intersecting => Ok(None),
        ClosedPolylineRelation::Disjoint => Ok(Some(disjoint_result(first, second, operation))),
        ClosedPolylineRelation::FirstContainsSecond => {
            Ok(Some(first_contains_second_result(first, second, operation)))
        }
        ClosedPolylineRelation::SecondContainsFirst => {
            Ok(Some(second_contains_first_result(first, second, operation)))
        }
    }
}

fn disjoint_result(
    first: &ClosedPolyline,
    second: &ClosedPolyline,
    operation: ClosedPolylineBooleanOp,
) -> ClosedPolylineBooleanResult {
    match operation {
        ClosedPolylineBooleanOp::Union | ClosedPolylineBooleanOp::Exclude => {
            ClosedPolylineBooleanResult::Two {
                first: first.clone(),
                second: second.clone(),
            }
        }
        ClosedPolylineBooleanOp::Intersect => ClosedPolylineBooleanResult::Empty,
        ClosedPolylineBooleanOp::Subtract => ClosedPolylineBooleanResult::One(first.clone()),
    }
}

fn first_contains_second_result(
    first: &ClosedPolyline,
    second: &ClosedPolyline,
    operation: ClosedPolylineBooleanOp,
) -> ClosedPolylineBooleanResult {
    match operation {
        ClosedPolylineBooleanOp::Union => ClosedPolylineBooleanResult::One(first.clone()),
        ClosedPolylineBooleanOp::Intersect => ClosedPolylineBooleanResult::One(second.clone()),
        ClosedPolylineBooleanOp::Subtract | ClosedPolylineBooleanOp::Exclude => {
            ClosedPolylineBooleanResult::Two {
                first: first.clone(),
                second: second.clone(),
            }
        }
    }
}

fn second_contains_first_result(
    first: &ClosedPolyline,
    second: &ClosedPolyline,
    operation: ClosedPolylineBooleanOp,
) -> ClosedPolylineBooleanResult {
    match operation {
        ClosedPolylineBooleanOp::Union => ClosedPolylineBooleanResult::One(second.clone()),
        ClosedPolylineBooleanOp::Intersect => ClosedPolylineBooleanResult::One(first.clone()),
        ClosedPolylineBooleanOp::Subtract => ClosedPolylineBooleanResult::Empty,
        ClosedPolylineBooleanOp::Exclude => ClosedPolylineBooleanResult::Two {
            first: second.clone(),
            second: first.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Point2;

    fn point(x: f64, y: f64) -> Point2 {
        Point2::new_unchecked(x, y)
    }

    fn contour(points: &[Point2]) -> ClosedPolyline {
        ClosedPolyline::new(points.to_vec()).expect("valid contour")
    }

    fn square(x: f64, y: f64, size: f64) -> ClosedPolyline {
        contour(&[
            point(x, y),
            point(x + size, y),
            point(x + size, y + size),
            point(x, y + size),
        ])
    }

    #[test]
    fn disjoint_union_and_exclude_preserve_both_contours() {
        let first = square(0.0, 0.0, 4.0);
        let second = square(10.0, 0.0, 4.0);

        assert_eq!(
            boolean_closed_polylines(&first, &second, ClosedPolylineBooleanOp::Union, 0.001),
            Ok(Some(ClosedPolylineBooleanResult::Two {
                first: first.clone(),
                second: second.clone(),
            }))
        );
        assert_eq!(
            boolean_closed_polylines(&first, &second, ClosedPolylineBooleanOp::Exclude, 0.001),
            Ok(Some(ClosedPolylineBooleanResult::Two { first, second }))
        );
    }

    #[test]
    fn disjoint_intersect_is_empty_and_subtract_keeps_first() {
        let first = square(0.0, 0.0, 4.0);
        let second = square(10.0, 0.0, 4.0);

        assert_eq!(
            boolean_closed_polylines(&first, &second, ClosedPolylineBooleanOp::Intersect, 0.001),
            Ok(Some(ClosedPolylineBooleanResult::Empty))
        );
        assert_eq!(
            boolean_closed_polylines(&first, &second, ClosedPolylineBooleanOp::Subtract, 0.001),
            Ok(Some(ClosedPolylineBooleanResult::One(first)))
        );
    }

    #[test]
    fn first_contains_second_maps_non_intersecting_operations() {
        let outer = square(0.0, 0.0, 10.0);
        let inner = square(2.0, 2.0, 2.0);

        assert_eq!(
            boolean_closed_polylines(&outer, &inner, ClosedPolylineBooleanOp::Union, 0.001),
            Ok(Some(ClosedPolylineBooleanResult::One(outer.clone())))
        );
        assert_eq!(
            boolean_closed_polylines(&outer, &inner, ClosedPolylineBooleanOp::Intersect, 0.001),
            Ok(Some(ClosedPolylineBooleanResult::One(inner.clone())))
        );
        assert_eq!(
            boolean_closed_polylines(&outer, &inner, ClosedPolylineBooleanOp::Subtract, 0.001),
            Ok(Some(ClosedPolylineBooleanResult::Two {
                first: outer,
                second: inner,
            }))
        );
    }

    #[test]
    fn second_contains_first_maps_subtract_to_empty() {
        let inner = square(2.0, 2.0, 2.0);
        let outer = square(0.0, 0.0, 10.0);

        assert_eq!(
            boolean_closed_polylines(&inner, &outer, ClosedPolylineBooleanOp::Union, 0.001),
            Ok(Some(ClosedPolylineBooleanResult::One(outer.clone())))
        );
        assert_eq!(
            boolean_closed_polylines(&inner, &outer, ClosedPolylineBooleanOp::Intersect, 0.001),
            Ok(Some(ClosedPolylineBooleanResult::One(inner.clone())))
        );
        assert_eq!(
            boolean_closed_polylines(&inner, &outer, ClosedPolylineBooleanOp::Subtract, 0.001),
            Ok(Some(ClosedPolylineBooleanResult::Empty))
        );
        assert_eq!(
            boolean_closed_polylines(&inner, &outer, ClosedPolylineBooleanOp::Exclude, 0.001),
            Ok(Some(ClosedPolylineBooleanResult::Two {
                first: outer,
                second: inner,
            }))
        );
    }

    #[test]
    fn intersecting_contours_defer_until_event_splitting_exists() {
        let first = square(0.0, 0.0, 10.0);
        let second = square(5.0, 5.0, 10.0);

        assert_eq!(
            boolean_closed_polylines(&first, &second, ClosedPolylineBooleanOp::Union, 0.001),
            Ok(None)
        );
    }

    #[test]
    fn invalid_tolerance_is_rejected() {
        let first = square(0.0, 0.0, 4.0);
        let second = square(10.0, 0.0, 4.0);

        assert_eq!(
            boolean_closed_polylines(&first, &second, ClosedPolylineBooleanOp::Union, 0.0),
            Err(GeometryError::NonPositiveTolerance)
        );
    }
}
