use crate::{
    ClosedPolyline, ClosedPolylineIntersectionEvent, ClosedPolylineRelation, GeometryError, Point2,
    PointLocation, classify_closed_polyline_relation, collect_closed_polyline_intersection_events,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContourSegmentSplit {
    pub segment_index: usize,
    pub t: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContourBooleanSplits {
    pub first: Vec<ContourSegmentSplit>,
    pub second: Vec<ContourSegmentSplit>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContourSegmentSpan {
    pub segment_index: usize,
    pub start_t: f64,
    pub end_t: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContourBooleanSpans {
    pub first: Vec<ContourSegmentSpan>,
    pub second: Vec<ContourSegmentSpan>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClassifiedContourSpan {
    pub span: ContourSegmentSpan,
    pub midpoint: Point2,
    pub location: PointLocation,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassifiedContourBooleanSpans {
    pub first: Vec<ClassifiedContourSpan>,
    pub second: Vec<ClassifiedContourSpan>,
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

pub fn classify_contour_boolean_spans(
    first: &ClosedPolyline,
    second: &ClosedPolyline,
    tolerance: f64,
) -> Result<ClassifiedContourBooleanSpans, GeometryError> {
    let spans = collect_contour_boolean_spans(first, second)?;
    Ok(ClassifiedContourBooleanSpans {
        first: classify_spans_against_contour(first, second, &spans.first, tolerance)?,
        second: classify_spans_against_contour(second, first, &spans.second, tolerance)?,
    })
}

pub fn collect_contour_boolean_spans(
    first: &ClosedPolyline,
    second: &ClosedPolyline,
) -> Result<ContourBooleanSpans, GeometryError> {
    let splits = collect_contour_boolean_splits(first, second)?;
    Ok(ContourBooleanSpans {
        first: spans_for_contour(first, &splits.first),
        second: spans_for_contour(second, &splits.second),
    })
}

fn classify_spans_against_contour(
    source: &ClosedPolyline,
    target: &ClosedPolyline,
    spans: &[ContourSegmentSpan],
    tolerance: f64,
) -> Result<Vec<ClassifiedContourSpan>, GeometryError> {
    let mut classified = Vec::with_capacity(spans.len());
    for span in spans {
        let midpoint = span_midpoint(source, *span)?;
        classified.push(ClassifiedContourSpan {
            span: *span,
            midpoint,
            location: target.locate_point(midpoint, tolerance)?,
        });
    }
    Ok(classified)
}

fn span_midpoint(
    contour: &ClosedPolyline,
    span: ContourSegmentSpan,
) -> Result<Point2, GeometryError> {
    let (start, end) = contour_segment(contour, span.segment_index)?;
    Ok(start.lerp(end, (span.start_t + span.end_t) * 0.5))
}

pub fn collect_contour_boolean_splits(
    first: &ClosedPolyline,
    second: &ClosedPolyline,
) -> Result<ContourBooleanSplits, GeometryError> {
    let mut first_splits = Vec::new();
    let mut second_splits = Vec::new();

    for event in collect_closed_polyline_intersection_events(first, second)? {
        match event {
            ClosedPolylineIntersectionEvent::Point {
                point,
                first_segment_indices,
                second_segment_indices,
            } => {
                push_point_splits(first, &mut first_splits, &first_segment_indices, point)?;
                push_point_splits(second, &mut second_splits, &second_segment_indices, point)?;
            }
            ClosedPolylineIntersectionEvent::Overlap {
                first_segment_index,
                second_segment_index,
                start,
                end,
            } => {
                push_split(&mut first_splits, first_segment_index, start.first_t);
                push_split(&mut first_splits, first_segment_index, end.first_t);
                push_split(&mut second_splits, second_segment_index, start.second_t);
                push_split(&mut second_splits, second_segment_index, end.second_t);
            }
        }
    }

    sort_splits(&mut first_splits);
    sort_splits(&mut second_splits);
    Ok(ContourBooleanSplits {
        first: first_splits,
        second: second_splits,
    })
}

fn spans_for_contour(
    contour: &ClosedPolyline,
    splits: &[ContourSegmentSplit],
) -> Vec<ContourSegmentSpan> {
    let mut spans = Vec::new();
    for segment_index in 0..contour.segment_count() {
        let mut parameters = vec![0.0, 1.0];
        for split in splits {
            if split.segment_index == segment_index {
                push_parameter(&mut parameters, split.t);
            }
        }
        parameters.sort_by(|a, b| a.total_cmp(b));
        for pair in parameters.windows(2) {
            let [start_t, end_t] = pair else {
                continue;
            };
            if start_t < end_t {
                spans.push(ContourSegmentSpan {
                    segment_index,
                    start_t: *start_t,
                    end_t: *end_t,
                });
            }
        }
    }
    spans
}

fn push_parameter(parameters: &mut Vec<f64>, t: f64) {
    if !parameters.contains(&t) {
        parameters.push(t);
    }
}

fn push_point_splits(
    contour: &ClosedPolyline,
    splits: &mut Vec<ContourSegmentSplit>,
    segment_indices: &[usize],
    point: Point2,
) -> Result<(), GeometryError> {
    for segment_index in segment_indices {
        let (start, end) = contour_segment(contour, *segment_index)?;
        let projection = point.project_onto_segment(start, end);
        push_split(splits, *segment_index, projection.t);
    }
    Ok(())
}

fn push_split(splits: &mut Vec<ContourSegmentSplit>, segment_index: usize, t: f64) {
    if !splits
        .iter()
        .any(|split| split.segment_index == segment_index && split.t == t)
    {
        splits.push(ContourSegmentSplit { segment_index, t });
    }
}

fn sort_splits(splits: &mut [ContourSegmentSplit]) {
    splits.sort_by(|a, b| {
        a.segment_index
            .cmp(&b.segment_index)
            .then_with(|| a.t.total_cmp(&b.t))
    });
}

fn contour_segment(
    contour: &ClosedPolyline,
    segment_index: usize,
) -> Result<(Point2, Point2), GeometryError> {
    let start = contour
        .points()
        .get(segment_index)
        .copied()
        .ok_or(GeometryError::CountOutOfRange)?;
    let next_index = if segment_index + 1 == contour.segment_count() {
        0
    } else {
        segment_index + 1
    };
    let end = contour
        .points()
        .get(next_index)
        .copied()
        .ok_or(GeometryError::CountOutOfRange)?;
    Ok((start, end))
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
    fn split_collection_finds_crossing_parameters() {
        let first = square(0.0, 0.0, 10.0);
        let second = square(5.0, 5.0, 10.0);

        assert_eq!(
            collect_contour_boolean_splits(&first, &second),
            Ok(ContourBooleanSplits {
                first: vec![
                    ContourSegmentSplit {
                        segment_index: 1,
                        t: 0.5,
                    },
                    ContourSegmentSplit {
                        segment_index: 2,
                        t: 0.5,
                    },
                ],
                second: vec![
                    ContourSegmentSplit {
                        segment_index: 0,
                        t: 0.5,
                    },
                    ContourSegmentSplit {
                        segment_index: 3,
                        t: 0.5,
                    },
                ],
            })
        );
    }

    #[test]
    fn split_collection_merges_endpoint_touch_indices() {
        let first = square(0.0, 0.0, 10.0);
        let second = contour(&[point(10.0, 0.0), point(14.0, -2.0), point(12.0, 2.0)]);

        assert_eq!(
            collect_contour_boolean_splits(&first, &second),
            Ok(ContourBooleanSplits {
                first: vec![
                    ContourSegmentSplit {
                        segment_index: 0,
                        t: 1.0,
                    },
                    ContourSegmentSplit {
                        segment_index: 1,
                        t: 0.0,
                    },
                ],
                second: vec![
                    ContourSegmentSplit {
                        segment_index: 0,
                        t: 0.0,
                    },
                    ContourSegmentSplit {
                        segment_index: 2,
                        t: 1.0,
                    },
                ],
            })
        );
    }

    #[test]
    fn split_collection_preserves_overlap_endpoints() {
        let first = square(0.0, 0.0, 10.0);
        let second = contour(&[point(4.0, 0.0), point(8.0, 0.0), point(6.0, -3.0)]);

        assert_eq!(
            collect_contour_boolean_splits(&first, &second),
            Ok(ContourBooleanSplits {
                first: vec![
                    ContourSegmentSplit {
                        segment_index: 0,
                        t: 0.4,
                    },
                    ContourSegmentSplit {
                        segment_index: 0,
                        t: 0.8,
                    },
                ],
                second: vec![
                    ContourSegmentSplit {
                        segment_index: 0,
                        t: 0.0,
                    },
                    ContourSegmentSplit {
                        segment_index: 0,
                        t: 1.0,
                    },
                    ContourSegmentSplit {
                        segment_index: 1,
                        t: 0.0,
                    },
                    ContourSegmentSplit {
                        segment_index: 2,
                        t: 1.0,
                    },
                ],
            })
        );
    }

    #[test]
    fn span_collection_subdivides_segments_at_split_parameters() {
        let first = square(0.0, 0.0, 10.0);
        let second = square(5.0, 5.0, 10.0);

        let spans = collect_contour_boolean_spans(&first, &second).expect("spans");
        assert_eq!(
            spans.first,
            vec![
                ContourSegmentSpan {
                    segment_index: 0,
                    start_t: 0.0,
                    end_t: 1.0,
                },
                ContourSegmentSpan {
                    segment_index: 1,
                    start_t: 0.0,
                    end_t: 0.5,
                },
                ContourSegmentSpan {
                    segment_index: 1,
                    start_t: 0.5,
                    end_t: 1.0,
                },
                ContourSegmentSpan {
                    segment_index: 2,
                    start_t: 0.0,
                    end_t: 0.5,
                },
                ContourSegmentSpan {
                    segment_index: 2,
                    start_t: 0.5,
                    end_t: 1.0,
                },
                ContourSegmentSpan {
                    segment_index: 3,
                    start_t: 0.0,
                    end_t: 1.0,
                },
            ]
        );
        assert_eq!(
            spans.second,
            vec![
                ContourSegmentSpan {
                    segment_index: 0,
                    start_t: 0.0,
                    end_t: 0.5,
                },
                ContourSegmentSpan {
                    segment_index: 0,
                    start_t: 0.5,
                    end_t: 1.0,
                },
                ContourSegmentSpan {
                    segment_index: 1,
                    start_t: 0.0,
                    end_t: 1.0,
                },
                ContourSegmentSpan {
                    segment_index: 2,
                    start_t: 0.0,
                    end_t: 1.0,
                },
                ContourSegmentSpan {
                    segment_index: 3,
                    start_t: 0.0,
                    end_t: 0.5,
                },
                ContourSegmentSpan {
                    segment_index: 3,
                    start_t: 0.5,
                    end_t: 1.0,
                },
            ]
        );
    }

    #[test]
    fn span_collection_returns_whole_segments_without_intersections() {
        let first = square(0.0, 0.0, 2.0);
        let second = square(10.0, 0.0, 2.0);

        assert_eq!(
            collect_contour_boolean_spans(&first, &second),
            Ok(ContourBooleanSpans {
                first: vec![
                    ContourSegmentSpan {
                        segment_index: 0,
                        start_t: 0.0,
                        end_t: 1.0,
                    },
                    ContourSegmentSpan {
                        segment_index: 1,
                        start_t: 0.0,
                        end_t: 1.0,
                    },
                    ContourSegmentSpan {
                        segment_index: 2,
                        start_t: 0.0,
                        end_t: 1.0,
                    },
                    ContourSegmentSpan {
                        segment_index: 3,
                        start_t: 0.0,
                        end_t: 1.0,
                    },
                ],
                second: vec![
                    ContourSegmentSpan {
                        segment_index: 0,
                        start_t: 0.0,
                        end_t: 1.0,
                    },
                    ContourSegmentSpan {
                        segment_index: 1,
                        start_t: 0.0,
                        end_t: 1.0,
                    },
                    ContourSegmentSpan {
                        segment_index: 2,
                        start_t: 0.0,
                        end_t: 1.0,
                    },
                    ContourSegmentSpan {
                        segment_index: 3,
                        start_t: 0.0,
                        end_t: 1.0,
                    },
                ],
            })
        );
    }

    #[test]
    fn classified_spans_sample_midpoints_against_opposite_contour() {
        let first = square(0.0, 0.0, 10.0);
        let second = square(5.0, 5.0, 10.0);

        let classified = classify_contour_boolean_spans(&first, &second, 0.001).expect("spans");

        assert_eq!(
            classified.first,
            vec![
                ClassifiedContourSpan {
                    span: ContourSegmentSpan {
                        segment_index: 0,
                        start_t: 0.0,
                        end_t: 1.0,
                    },
                    midpoint: point(5.0, 0.0),
                    location: PointLocation::Outside,
                },
                ClassifiedContourSpan {
                    span: ContourSegmentSpan {
                        segment_index: 1,
                        start_t: 0.0,
                        end_t: 0.5,
                    },
                    midpoint: point(10.0, 2.5),
                    location: PointLocation::Outside,
                },
                ClassifiedContourSpan {
                    span: ContourSegmentSpan {
                        segment_index: 1,
                        start_t: 0.5,
                        end_t: 1.0,
                    },
                    midpoint: point(10.0, 7.5),
                    location: PointLocation::Inside,
                },
                ClassifiedContourSpan {
                    span: ContourSegmentSpan {
                        segment_index: 2,
                        start_t: 0.0,
                        end_t: 0.5,
                    },
                    midpoint: point(7.5, 10.0),
                    location: PointLocation::Inside,
                },
                ClassifiedContourSpan {
                    span: ContourSegmentSpan {
                        segment_index: 2,
                        start_t: 0.5,
                        end_t: 1.0,
                    },
                    midpoint: point(2.5, 10.0),
                    location: PointLocation::Outside,
                },
                ClassifiedContourSpan {
                    span: ContourSegmentSpan {
                        segment_index: 3,
                        start_t: 0.0,
                        end_t: 1.0,
                    },
                    midpoint: point(0.0, 5.0),
                    location: PointLocation::Outside,
                },
            ]
        );
    }

    #[test]
    fn classified_spans_can_report_boundary_midpoints() {
        let first = square(0.0, 0.0, 10.0);
        let second = square(2.0, 0.0, 4.0);

        let classified = classify_contour_boolean_spans(&first, &second, 0.001).expect("spans");

        assert!(
            classified
                .first
                .iter()
                .any(|span| span.location == PointLocation::Boundary)
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
