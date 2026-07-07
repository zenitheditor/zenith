use std::collections::BTreeMap;

use crate::{
    ClassifiedContourSpan, ClosedPolyline, ClosedPolylineBooleanOp, ClosedPolylineBooleanResult,
    ClosedPolylineIntersectionEvent, ClosedPolylineWinding, GeometryError, Point2,
    boolean_closed_polylines, collect_closed_polyline_intersection_events,
    select_contour_boolean_spans,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SourceContour {
    First,
    Second,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct EndpointKey {
    source: SourceContour,
    segment_index: usize,
    t_bits: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct VertexId(usize);

#[derive(Debug, Clone, PartialEq)]
struct VertexRegistry {
    points: Vec<Point2>,
    keys: BTreeMap<EndpointKey, VertexId>,
}

#[derive(Debug, Clone, PartialEq)]
struct DirectedEdge {
    start_vertex: VertexId,
    end_vertex: VertexId,
    start: Point2,
    end: Point2,
}

pub fn reconstruct_contour_boolean_result(
    first: &ClosedPolyline,
    second: &ClosedPolyline,
    operation: ClosedPolylineBooleanOp,
    tolerance: f64,
) -> Result<Vec<ClosedPolyline>, GeometryError> {
    if let Some(result) = boolean_closed_polylines(first, second, operation, tolerance)? {
        return contours_from_result(result);
    }

    match operation {
        ClosedPolylineBooleanOp::Union | ClosedPolylineBooleanOp::Intersect => {
            reconstruct_intersecting(first, second, operation, tolerance)
        }
        ClosedPolylineBooleanOp::Subtract => reconstruct_subtract(first, second, tolerance),
        ClosedPolylineBooleanOp::Exclude => {
            let mut contours = reconstruct_subtract(first, second, tolerance)?;
            contours.extend(reconstruct_subtract(second, first, tolerance)?);
            canonicalize_contours(contours)
        }
    }
}

fn contours_from_result(
    result: ClosedPolylineBooleanResult,
) -> Result<Vec<ClosedPolyline>, GeometryError> {
    match result {
        ClosedPolylineBooleanResult::Empty => Ok(Vec::new()),
        ClosedPolylineBooleanResult::One(contour) => canonicalize_contours(vec![contour]),
        ClosedPolylineBooleanResult::Two { first, second } => {
            canonicalize_contours(vec![first, second])
        }
    }
}

fn reconstruct_intersecting(
    first: &ClosedPolyline,
    second: &ClosedPolyline,
    operation: ClosedPolylineBooleanOp,
    tolerance: f64,
) -> Result<Vec<ClosedPolyline>, GeometryError> {
    let selected = select_contour_boolean_spans(first, second, operation, tolerance)?;
    let mut registry = build_vertex_registry(first, second)?;
    let mut edges = Vec::with_capacity(selected.first.len() + selected.second.len());
    append_edges(
        &mut edges,
        &mut registry,
        first,
        SourceContour::First,
        &selected.first,
        source_forward(first),
    )?;
    append_edges(
        &mut edges,
        &mut registry,
        second,
        SourceContour::Second,
        &selected.second,
        source_forward(second),
    )?;
    reconstruct_edges(&edges)
}

fn reconstruct_subtract(
    first: &ClosedPolyline,
    second: &ClosedPolyline,
    tolerance: f64,
) -> Result<Vec<ClosedPolyline>, GeometryError> {
    let selected =
        select_contour_boolean_spans(first, second, ClosedPolylineBooleanOp::Subtract, tolerance)?;
    let mut registry = build_vertex_registry(first, second)?;
    let mut edges = Vec::with_capacity(selected.first.len() + selected.second.len());
    append_edges(
        &mut edges,
        &mut registry,
        first,
        SourceContour::First,
        &selected.first,
        source_forward(first),
    )?;
    append_edges(
        &mut edges,
        &mut registry,
        second,
        SourceContour::Second,
        &selected.second,
        !source_forward(second),
    )?;
    reconstruct_edges(&edges)
}

fn source_forward(contour: &ClosedPolyline) -> bool {
    match contour.winding() {
        ClosedPolylineWinding::Clockwise => false,
        ClosedPolylineWinding::CounterClockwise => true,
    }
}

fn build_vertex_registry(
    first: &ClosedPolyline,
    second: &ClosedPolyline,
) -> Result<VertexRegistry, GeometryError> {
    let mut registry = VertexRegistry {
        points: Vec::new(),
        keys: BTreeMap::new(),
    };
    add_original_vertices(&mut registry, first, SourceContour::First);
    add_original_vertices(&mut registry, second, SourceContour::Second);

    for event in collect_closed_polyline_intersection_events(first, second)? {
        match event {
            ClosedPolylineIntersectionEvent::Point {
                point,
                first_segment_indices,
                second_segment_indices,
            } => {
                let [first_segment_index] = first_segment_indices.as_slice() else {
                    return Err(GeometryError::InvalidContour);
                };
                let [second_segment_index] = second_segment_indices.as_slice() else {
                    return Err(GeometryError::InvalidContour);
                };
                let first_t = segment_t(first, *first_segment_index, point)?;
                let second_t = segment_t(second, *second_segment_index, point)?;
                if endpoint_t(first_t) || endpoint_t(second_t) {
                    return Err(GeometryError::InvalidContour);
                }
                let vertex = registry.add(point);
                registry.keys.insert(
                    endpoint_key(SourceContour::First, *first_segment_index, first_t),
                    vertex,
                );
                registry.keys.insert(
                    endpoint_key(SourceContour::Second, *second_segment_index, second_t),
                    vertex,
                );
            }
            ClosedPolylineIntersectionEvent::Overlap { .. } => {
                return Err(GeometryError::InvalidContour);
            }
        }
    }

    Ok(registry)
}

fn add_original_vertices(
    registry: &mut VertexRegistry,
    contour: &ClosedPolyline,
    source: SourceContour,
) {
    for (index, point) in contour.points().iter().copied().enumerate() {
        let vertex = registry.add(point);
        registry
            .keys
            .insert(endpoint_key(source, index, 0.0), vertex);
        let previous = if index == 0 {
            contour.segment_count().saturating_sub(1)
        } else {
            index - 1
        };
        registry
            .keys
            .insert(endpoint_key(source, previous, 1.0), vertex);
    }
}

impl VertexRegistry {
    fn add(&mut self, point: Point2) -> VertexId {
        let vertex = VertexId(self.points.len());
        self.points.push(point);
        vertex
    }

    fn endpoint(
        &mut self,
        source: SourceContour,
        segment_index: usize,
        t: f64,
        point: Point2,
    ) -> VertexId {
        let key = endpoint_key(source, segment_index, t);
        if let Some(vertex) = self.keys.get(&key).copied() {
            return vertex;
        }
        let vertex = self.add(point);
        self.keys.insert(key, vertex);
        vertex
    }
}

fn append_edges(
    edges: &mut Vec<DirectedEdge>,
    registry: &mut VertexRegistry,
    contour: &ClosedPolyline,
    source: SourceContour,
    spans: &[ClassifiedContourSpan],
    forward: bool,
) -> Result<(), GeometryError> {
    for classified in spans {
        let (segment_start, segment_end) = contour_segment(contour, classified.span.segment_index)?;
        let start = segment_start.lerp(segment_end, classified.span.start_t);
        let end = segment_start.lerp(segment_end, classified.span.end_t);
        let start_vertex = registry.endpoint(
            source,
            classified.span.segment_index,
            classified.span.start_t,
            start,
        );
        let end_vertex = registry.endpoint(
            source,
            classified.span.segment_index,
            classified.span.end_t,
            end,
        );
        if forward {
            edges.push(DirectedEdge {
                start_vertex,
                end_vertex,
                start,
                end,
            });
        } else {
            edges.push(DirectedEdge {
                start_vertex: end_vertex,
                end_vertex: start_vertex,
                start: end,
                end: start,
            });
        }
    }
    Ok(())
}

fn reconstruct_edges(edges: &[DirectedEdge]) -> Result<Vec<ClosedPolyline>, GeometryError> {
    if edges.is_empty() {
        return Ok(Vec::new());
    }
    validate_degrees(edges)?;
    let mut outgoing = BTreeMap::new();
    for (index, edge) in edges.iter().enumerate() {
        outgoing.insert(edge.start_vertex, index);
    }

    let mut visited = vec![0_u8; edges.len()];
    let mut contours = Vec::new();
    for start_index in 0..edges.len() {
        if visited.get(start_index).is_some_and(|slot| *slot == 1) {
            continue;
        }
        let contour = walk_contour(start_index, edges, &outgoing, &mut visited)?;
        contours.push(contour);
    }
    canonicalize_contours(contours)
}

fn validate_degrees(edges: &[DirectedEdge]) -> Result<(), GeometryError> {
    let mut incoming: BTreeMap<VertexId, usize> = BTreeMap::new();
    let mut outgoing: BTreeMap<VertexId, usize> = BTreeMap::new();
    for edge in edges {
        *incoming.entry(edge.end_vertex).or_default() += 1;
        *outgoing.entry(edge.start_vertex).or_default() += 1;
    }
    if incoming.len() != outgoing.len() {
        return Err(GeometryError::InvalidContour);
    }
    for (vertex, incoming_count) in incoming {
        if incoming_count != 1 || outgoing.get(&vertex).copied() != Some(1) {
            return Err(GeometryError::InvalidContour);
        }
    }
    Ok(())
}

fn walk_contour(
    start_index: usize,
    edges: &[DirectedEdge],
    outgoing: &BTreeMap<VertexId, usize>,
    visited: &mut [u8],
) -> Result<ClosedPolyline, GeometryError> {
    let start_vertex = edges
        .get(start_index)
        .ok_or(GeometryError::CountOutOfRange)?
        .start_vertex;
    let mut points = Vec::new();
    let mut edge_index = start_index;
    loop {
        let Some(visited_slot) = visited.get_mut(edge_index) else {
            return Err(GeometryError::InvalidContour);
        };
        if *visited_slot == 1 {
            return Err(GeometryError::InvalidContour);
        }
        *visited_slot = 1;
        let edge = edges
            .get(edge_index)
            .ok_or(GeometryError::CountOutOfRange)?;
        points.push(edge.start);
        if edge.end_vertex == start_vertex {
            break;
        }
        edge_index = outgoing
            .get(&edge.end_vertex)
            .copied()
            .ok_or(GeometryError::InvalidContour)?;
    }
    canonicalize_contour(ClosedPolyline::new(points)?)
}

fn canonicalize_contours(
    contours: Vec<ClosedPolyline>,
) -> Result<Vec<ClosedPolyline>, GeometryError> {
    let mut normalized = Vec::with_capacity(contours.len());
    for contour in contours {
        normalized.push(canonicalize_contour(contour)?);
    }
    normalized.sort_by(compare_contours);
    Ok(normalized)
}

fn canonicalize_contour(contour: ClosedPolyline) -> Result<ClosedPolyline, GeometryError> {
    let mut points = contour.points().to_vec();
    if contour.winding() == ClosedPolylineWinding::Clockwise {
        points.reverse();
    }
    rotate_to_smallest_point(&mut points);
    ClosedPolyline::new(points)
}

fn rotate_to_smallest_point(points: &mut [Point2]) {
    let Some((index, _)) = points
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| compare_points(left, right))
    else {
        return;
    };
    points.rotate_left(index);
}

fn compare_contours(left: &ClosedPolyline, right: &ClosedPolyline) -> std::cmp::Ordering {
    let left_bounds = left.bounds();
    let right_bounds = right.bounds();
    left_bounds
        .min_x
        .total_cmp(&right_bounds.min_x)
        .then_with(|| left_bounds.min_y.total_cmp(&right_bounds.min_y))
        .then_with(|| left_bounds.max_x.total_cmp(&right_bounds.max_x))
        .then_with(|| left_bounds.max_y.total_cmp(&right_bounds.max_y))
        .then_with(|| compare_point_slices(left.points(), right.points()))
}

fn compare_point_slices(left: &[Point2], right: &[Point2]) -> std::cmp::Ordering {
    for (left_point, right_point) in left.iter().zip(right.iter()) {
        let ordering = compare_points(left_point, right_point);
        if ordering != std::cmp::Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

fn compare_points(left: &Point2, right: &Point2) -> std::cmp::Ordering {
    left.x
        .total_cmp(&right.x)
        .then_with(|| left.y.total_cmp(&right.y))
}

fn segment_t(
    contour: &ClosedPolyline,
    segment_index: usize,
    point: Point2,
) -> Result<f64, GeometryError> {
    let (start, end) = contour_segment(contour, segment_index)?;
    Ok(point.project_onto_segment(start, end).t)
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

fn endpoint_key(source: SourceContour, segment_index: usize, t: f64) -> EndpointKey {
    EndpointKey {
        source,
        segment_index,
        t_bits: t.to_bits(),
    }
}

fn endpoint_t(t: f64) -> bool {
    t == 0.0 || t == 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn points(contour: &ClosedPolyline) -> Vec<Point2> {
        contour.points().to_vec()
    }

    #[test]
    fn reconstructs_overlapping_rectangle_intersection() {
        let first = square(0.0, 0.0, 10.0);
        let second = square(5.0, 5.0, 10.0);

        let contours = reconstruct_contour_boolean_result(
            &first,
            &second,
            ClosedPolylineBooleanOp::Intersect,
            0.001,
        )
        .expect("contours");

        assert_eq!(contours.len(), 1);
        assert_eq!(
            points(&contours[0]),
            vec![
                point(5.0, 5.0),
                point(10.0, 5.0),
                point(10.0, 10.0),
                point(5.0, 10.0),
            ]
        );
    }

    #[test]
    fn reconstructs_overlapping_rectangle_union() {
        let first = square(0.0, 0.0, 10.0);
        let second = square(5.0, 5.0, 10.0);

        let contours = reconstruct_contour_boolean_result(
            &first,
            &second,
            ClosedPolylineBooleanOp::Union,
            0.001,
        )
        .expect("contours");

        assert_eq!(contours.len(), 1);
        assert_eq!(
            points(&contours[0]),
            vec![
                point(0.0, 0.0),
                point(10.0, 0.0),
                point(10.0, 5.0),
                point(15.0, 5.0),
                point(15.0, 15.0),
                point(5.0, 15.0),
                point(5.0, 10.0),
                point(0.0, 10.0),
            ]
        );
    }

    #[test]
    fn reconstructs_overlapping_rectangle_subtract() {
        let first = square(0.0, 0.0, 10.0);
        let second = square(5.0, 5.0, 10.0);

        let contours = reconstruct_contour_boolean_result(
            &first,
            &second,
            ClosedPolylineBooleanOp::Subtract,
            0.001,
        )
        .expect("contours");

        assert_eq!(contours.len(), 1);
        assert_eq!(
            points(&contours[0]),
            vec![
                point(0.0, 0.0),
                point(10.0, 0.0),
                point(10.0, 5.0),
                point(5.0, 5.0),
                point(5.0, 10.0),
                point(0.0, 10.0),
            ]
        );
    }

    #[test]
    fn reconstructs_exclude_as_two_subtract_contours() {
        let first = square(0.0, 0.0, 10.0);
        let second = square(5.0, 5.0, 10.0);

        let contours = reconstruct_contour_boolean_result(
            &first,
            &second,
            ClosedPolylineBooleanOp::Exclude,
            0.001,
        )
        .expect("contours");

        assert_eq!(contours.len(), 2);
    }

    #[test]
    fn reversed_input_winding_keeps_canonical_intersection() {
        let first = contour(&[
            point(0.0, 0.0),
            point(0.0, 10.0),
            point(10.0, 10.0),
            point(10.0, 0.0),
        ]);
        let second = square(5.0, 5.0, 10.0);

        let contours = reconstruct_contour_boolean_result(
            &first,
            &second,
            ClosedPolylineBooleanOp::Intersect,
            0.001,
        )
        .expect("contours");

        assert_eq!(
            points(&contours[0]),
            vec![
                point(5.0, 5.0),
                point(10.0, 5.0),
                point(10.0, 10.0),
                point(5.0, 10.0),
            ]
        );
    }

    #[test]
    fn endpoint_touch_is_deferred() {
        let first = square(0.0, 0.0, 10.0);
        let second = square(10.0, 10.0, 5.0);

        assert_eq!(
            reconstruct_contour_boolean_result(
                &first,
                &second,
                ClosedPolylineBooleanOp::Union,
                0.001,
            ),
            Err(GeometryError::InvalidContour)
        );
    }
}
