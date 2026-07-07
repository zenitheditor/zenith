use crate::{ClassifiedContourSpan, ClosedPolyline, GeometryError, Point2, PointLocation};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContourBooleanPiece {
    pub segment_index: usize,
    pub start_t: f64,
    pub end_t: f64,
    pub start: Point2,
    pub end: Point2,
    pub location: PointLocation,
}

pub fn materialize_contour_boolean_pieces(
    contour: &ClosedPolyline,
    spans: &[ClassifiedContourSpan],
) -> Result<Vec<ContourBooleanPiece>, GeometryError> {
    let mut pieces = Vec::with_capacity(spans.len());
    for classified in spans {
        pieces.push(materialize_piece(contour, *classified)?);
    }
    Ok(pieces)
}

fn materialize_piece(
    contour: &ClosedPolyline,
    classified: ClassifiedContourSpan,
) -> Result<ContourBooleanPiece, GeometryError> {
    let (segment_start, segment_end) = contour_segment(contour, classified.span.segment_index)?;
    Ok(ContourBooleanPiece {
        segment_index: classified.span.segment_index,
        start_t: classified.span.start_t,
        end_t: classified.span.end_t,
        start: segment_start.lerp(segment_end, classified.span.start_t),
        end: segment_start.lerp(segment_end, classified.span.end_t),
        location: classified.location,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ClosedPolylineBooleanOp, ContourSegmentSpan, classify_contour_boolean_spans,
        select_contour_boolean_spans,
    };

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
    fn materializes_classified_spans_to_piece_endpoints() {
        let first = square(0.0, 0.0, 10.0);
        let second = square(5.0, 5.0, 10.0);
        let classified = classify_contour_boolean_spans(&first, &second, 0.001).expect("spans");

        assert_eq!(
            materialize_contour_boolean_pieces(&first, &classified.first[1..3]),
            Ok(vec![
                ContourBooleanPiece {
                    segment_index: 1,
                    start_t: 0.0,
                    end_t: 0.5,
                    start: point(10.0, 0.0),
                    end: point(10.0, 5.0),
                    location: PointLocation::Outside,
                },
                ContourBooleanPiece {
                    segment_index: 1,
                    start_t: 0.5,
                    end_t: 1.0,
                    start: point(10.0, 5.0),
                    end: point(10.0, 10.0),
                    location: PointLocation::Inside,
                },
            ])
        );
    }

    #[test]
    fn materializes_selected_operation_spans() {
        let first = square(0.0, 0.0, 10.0);
        let second = square(5.0, 5.0, 10.0);
        let selected = select_contour_boolean_spans(
            &first,
            &second,
            ClosedPolylineBooleanOp::Intersect,
            0.001,
        )
        .expect("selected");

        let pieces = materialize_contour_boolean_pieces(&first, &selected.first).expect("pieces");
        assert_eq!(pieces.len(), 2);
        assert!(
            pieces
                .iter()
                .all(|piece| piece.location == PointLocation::Inside)
        );
    }

    #[test]
    fn rejects_invalid_segment_index() {
        let contour = square(0.0, 0.0, 10.0);
        let classified = ClassifiedContourSpan {
            span: ContourSegmentSpan {
                segment_index: 4,
                start_t: 0.0,
                end_t: 1.0,
            },
            midpoint: point(0.0, 0.0),
            location: PointLocation::Outside,
        };

        assert_eq!(
            materialize_contour_boolean_pieces(&contour, &[classified]),
            Err(GeometryError::CountOutOfRange)
        );
    }
}
