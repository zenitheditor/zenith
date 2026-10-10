use crate::math;
use crate::{
    AffineTransform, CubicBezier, GeometryError, Point2, RectBounds, project_onto_cubic_bezier,
    validation::{validate_parameter, validate_tolerance},
};

mod join;

const ZERO_LENGTH_EPSILON: f64 = 0.0;

#[derive(Debug, Clone, PartialEq)]
pub struct PathGeometry {
    anchors: Vec<PathAnchor>,
    closed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathAnchor {
    pub point: Point2,
    pub in_handle: Option<Point2>,
    pub out_handle: Option<Point2>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathSegment {
    Line { start: Point2, end: Point2 },
    Cubic { curve: CubicBezier },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathProjection {
    pub point: Point2,
    pub segment_index: usize,
    pub segment_t: f64,
    pub distance_squared: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathTopology {
    pub anchor_count: usize,
    pub segment_count: usize,
    pub open_subpath_count: usize,
    pub closed_subpath_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathJoinVectors {
    pub in_vector: Point2,
    pub out_vector: Point2,
    pub in_length: f64,
    pub out_length: f64,
}

impl PathGeometry {
    pub fn new(anchors: Vec<PathAnchor>, closed: bool) -> Result<Self, GeometryError> {
        for anchor in &anchors {
            anchor.validate()?;
        }

        Ok(Self { anchors, closed })
    }

    #[must_use]
    pub fn anchors(&self) -> &[PathAnchor] {
        &self.anchors
    }

    #[must_use]
    pub const fn closed(&self) -> bool {
        self.closed
    }

    #[must_use]
    pub fn topology(&self) -> PathTopology {
        Self::topology_for(self.anchors.len(), self.closed)
    }

    #[must_use]
    pub fn topology_for(anchor_count: usize, closed: bool) -> PathTopology {
        PathTopology {
            anchor_count,
            segment_count: segment_count(anchor_count, closed),
            open_subpath_count: usize::from(anchor_count > 0 && !closed),
            closed_subpath_count: usize::from(closed && anchor_count >= 3),
        }
    }

    pub fn segments(&self) -> Result<Vec<PathSegment>, GeometryError> {
        let segment_count = self.topology().segment_count;
        let mut segments = Vec::with_capacity(segment_count);

        for index in 0..segment_count {
            let Some((start, end)) = segment_pair(&self.anchors, self.closed, index) else {
                continue;
            };
            segments.push(segment_between(start, end)?);
        }

        Ok(segments)
    }

    pub fn bounds(&self) -> Result<Option<RectBounds>, GeometryError> {
        let Some(first) = self.anchors.first() else {
            return Ok(None);
        };
        let mut bounds = RectBounds::from_point(first.point);

        for segment in self.segments()? {
            bounds = match segment {
                PathSegment::Line { end, .. } => bounds.include_point(end),
                PathSegment::Cubic { curve } => bounds.include_bounds(curve.bounds()?),
            };
        }

        Ok(Some(bounds))
    }

    pub fn split_segment(
        &self,
        segment_index: usize,
        segment_t: f64,
    ) -> Result<(Self, usize), GeometryError> {
        validate_parameter(segment_t)?;

        if segment_index >= self.topology().segment_count {
            return Err(GeometryError::CountOutOfRange);
        }

        let Some((start, end)) = segment_pair(&self.anchors, self.closed, segment_index) else {
            return Err(GeometryError::CountOutOfRange);
        };
        let segment = segment_between(start, end)?;
        let mut anchors = self.anchors.clone();
        let inserted_anchor_index =
            segment_insertion_index(&self.anchors, self.closed, segment_index)
                .ok_or(GeometryError::CountOutOfRange)?;

        match segment {
            PathSegment::Line { start, end } => {
                let inserted = PathAnchor::new(start.lerp(end, segment_t), None, None)?;
                insert_anchor(&mut anchors, inserted_anchor_index, inserted)?;
            }
            PathSegment::Cubic { curve } => {
                let (left, right) = curve.split(segment_t)?;
                update_out_handle(&mut anchors, segment_index, left.p1)?;
                let end_index = segment_end_index(&self.anchors, self.closed, segment_index)
                    .ok_or(GeometryError::CountOutOfRange)?;
                update_in_handle(&mut anchors, end_index, right.p2)?;
                let inserted = PathAnchor::new(left.p3, Some(left.p2), Some(right.p1))?;
                insert_anchor(&mut anchors, inserted_anchor_index, inserted)?;
            }
        }

        Ok((
            Self {
                anchors,
                closed: self.closed,
            },
            inserted_anchor_index,
        ))
    }

    pub fn transform(&self, transform: AffineTransform) -> Result<Self, GeometryError> {
        let mut anchors = Vec::with_capacity(self.anchors.len());
        for anchor in &self.anchors {
            anchors.push(anchor.transform(transform)?);
        }

        Self::new(anchors, self.closed)
    }

    pub fn flatten(&self, tolerance: f64) -> Result<Vec<Point2>, GeometryError> {
        validate_tolerance(tolerance)?;

        let mut points = Vec::with_capacity(self.topology().segment_count.saturating_add(1));
        let Some(first) = self.anchors.first() else {
            return Ok(points);
        };
        points.push(first.point);

        for segment in self.segments()? {
            match segment {
                PathSegment::Line { end, .. } => points.push(end),
                PathSegment::Cubic { curve } => {
                    let flattened = curve.flatten(tolerance)?;
                    points.extend(flattened.into_iter().skip(1));
                }
            }
        }

        Ok(points)
    }

    pub fn project(
        &self,
        point: Point2,
        tolerance: f64,
    ) -> Result<Option<PathProjection>, GeometryError> {
        validate_tolerance(tolerance)?;
        point.validate()?;

        let mut nearest: Option<PathProjection> = None;
        for (segment_index, segment) in self.segments()?.into_iter().enumerate() {
            let candidate = match segment {
                PathSegment::Line { start, end } => {
                    let projection = point.project_onto_segment(start, end);
                    PathProjection {
                        point: projection.point,
                        segment_index,
                        segment_t: projection.t,
                        distance_squared: projection.distance_squared,
                    }
                }
                PathSegment::Cubic { curve } => {
                    let projection = project_onto_cubic_bezier(point, curve, tolerance)?;
                    PathProjection {
                        point: projection.point,
                        segment_index,
                        segment_t: projection.t,
                        distance_squared: projection.distance_squared,
                    }
                }
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

        Ok(nearest)
    }
}

impl PathAnchor {
    pub fn new(
        point: Point2,
        in_handle: Option<Point2>,
        out_handle: Option<Point2>,
    ) -> Result<Self, GeometryError> {
        let anchor = Self {
            point,
            in_handle,
            out_handle,
        };
        anchor.validate()?;
        Ok(anchor)
    }

    #[must_use]
    pub fn complete_handle_count(self) -> usize {
        usize::from(self.in_handle.is_some()) + usize::from(self.out_handle.is_some())
    }

    #[must_use]
    pub fn join_vectors(self) -> Option<PathJoinVectors> {
        let in_handle = self.in_handle?;
        let out_handle = self.out_handle?;
        let in_vector =
            Point2::new_unchecked(in_handle.x - self.point.x, in_handle.y - self.point.y);
        let out_vector =
            Point2::new_unchecked(out_handle.x - self.point.x, out_handle.y - self.point.y);
        if !in_vector.is_finite() || !out_vector.is_finite() {
            return None;
        }

        let in_length = math::hypot(in_vector.x, in_vector.y);
        let out_length = math::hypot(out_vector.x, out_vector.y);
        if !in_length.is_finite() || !out_length.is_finite() {
            return None;
        }

        Some(PathJoinVectors {
            in_vector,
            out_vector,
            in_length,
            out_length,
        })
    }

    fn validate(self) -> Result<(), GeometryError> {
        self.point.validate()?;
        if let Some(point) = self.in_handle {
            point.validate()?;
        }
        if let Some(point) = self.out_handle {
            point.validate()?;
        }
        Ok(())
    }

    fn transform(self, transform: AffineTransform) -> Result<Self, GeometryError> {
        Self::new(
            transform.apply_point(self.point)?,
            self.in_handle
                .map(|point| transform.apply_point(point))
                .transpose()?,
            self.out_handle
                .map(|point| transform.apply_point(point))
                .transpose()?,
        )
    }
}

fn segment_count(anchor_count: usize, closed: bool) -> usize {
    if anchor_count == 0 {
        0
    } else if closed {
        anchor_count
    } else {
        anchor_count.saturating_sub(1)
    }
}

fn segment_pair(
    anchors: &[PathAnchor],
    closed: bool,
    index: usize,
) -> Option<(PathAnchor, PathAnchor)> {
    let start = anchors.get(index).copied()?;
    let end_index = segment_end_index(anchors, closed, index)?;
    let end = anchors.get(end_index).copied()?;
    Some((start, end))
}

fn segment_end_index(anchors: &[PathAnchor], closed: bool, index: usize) -> Option<usize> {
    let next_index = index.checked_add(1)?;
    if next_index < anchors.len() {
        Some(next_index)
    } else if closed {
        Some(0)
    } else {
        None
    }
}

fn segment_insertion_index(anchors: &[PathAnchor], closed: bool, index: usize) -> Option<usize> {
    let end_index = segment_end_index(anchors, closed, index)?;
    if closed && end_index == 0 {
        Some(anchors.len())
    } else {
        Some(end_index)
    }
}

fn segment_between(start: PathAnchor, end: PathAnchor) -> Result<PathSegment, GeometryError> {
    match (start.out_handle, end.in_handle) {
        (None, None) => Ok(PathSegment::Line {
            start: start.point,
            end: end.point,
        }),
        (out_handle, in_handle) => {
            let control_start = match out_handle {
                Some(point) => point,
                None => start.point,
            };
            let control_end = match in_handle {
                Some(point) => point,
                None => end.point,
            };
            Ok(PathSegment::Cubic {
                curve: CubicBezier::new(start.point, control_start, control_end, end.point)?,
            })
        }
    }
}

fn update_out_handle(
    anchors: &mut [PathAnchor],
    index: usize,
    out_handle: Point2,
) -> Result<(), GeometryError> {
    let anchor = anchors
        .get_mut(index)
        .ok_or(GeometryError::CountOutOfRange)?;
    anchor.out_handle = Some(out_handle);
    anchor.validate()
}

fn update_in_handle(
    anchors: &mut [PathAnchor],
    index: usize,
    in_handle: Point2,
) -> Result<(), GeometryError> {
    let anchor = anchors
        .get_mut(index)
        .ok_or(GeometryError::CountOutOfRange)?;
    anchor.in_handle = Some(in_handle);
    anchor.validate()
}

fn insert_anchor(
    anchors: &mut Vec<PathAnchor>,
    index: usize,
    anchor: PathAnchor,
) -> Result<(), GeometryError> {
    if index > anchors.len() {
        return Err(GeometryError::CountOutOfRange);
    }
    anchors.insert(index, anchor);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1.0e-9;

    #[test]
    fn open_path_builds_adjacent_line_segments() {
        let path = PathGeometry::new(
            vec![anchor(0.0, 0.0), anchor(10.0, 0.0), anchor(10.0, 10.0)],
            false,
        )
        .expect("valid path");

        assert_eq!(
            path.topology(),
            PathTopology {
                anchor_count: 3,
                segment_count: 2,
                open_subpath_count: 1,
                closed_subpath_count: 0,
            }
        );
        assert_eq!(
            path.segments().expect("segments"),
            vec![
                PathSegment::Line {
                    start: point(0.0, 0.0),
                    end: point(10.0, 0.0),
                },
                PathSegment::Line {
                    start: point(10.0, 0.0),
                    end: point(10.0, 10.0),
                },
            ]
        );
    }

    #[test]
    fn cubic_segments_fall_back_to_missing_endpoint_controls() {
        let start =
            PathAnchor::new(point(0.0, 0.0), None, Some(point(5.0, 10.0))).expect("valid anchor");
        let end = anchor(10.0, 0.0);
        let path = PathGeometry::new(vec![start, end], false).expect("valid path");

        assert_eq!(
            path.segments().expect("segments"),
            vec![PathSegment::Cubic {
                curve: CubicBezier::new_unchecked(
                    point(0.0, 0.0),
                    point(5.0, 10.0),
                    point(10.0, 0.0),
                    point(10.0, 0.0),
                ),
            }]
        );

        let start = anchor(0.0, 0.0);
        let end =
            PathAnchor::new(point(10.0, 0.0), Some(point(5.0, -10.0)), None).expect("valid anchor");
        let path = PathGeometry::new(vec![start, end], false).expect("valid path");

        assert_eq!(
            path.segments().expect("segments"),
            vec![PathSegment::Cubic {
                curve: CubicBezier::new_unchecked(
                    point(0.0, 0.0),
                    point(0.0, 0.0),
                    point(5.0, -10.0),
                    point(10.0, 0.0),
                ),
            }]
        );
    }

    #[test]
    fn closed_path_reports_topology_and_closing_segment() {
        let path = PathGeometry::new(
            vec![anchor(0.0, 0.0), anchor(10.0, 0.0), anchor(10.0, 10.0)],
            true,
        )
        .expect("valid path");

        assert_eq!(
            path.topology(),
            PathTopology {
                anchor_count: 3,
                segment_count: 3,
                open_subpath_count: 0,
                closed_subpath_count: 1,
            }
        );
        assert_eq!(
            path.segments().expect("segments"),
            vec![
                PathSegment::Line {
                    start: point(0.0, 0.0),
                    end: point(10.0, 0.0),
                },
                PathSegment::Line {
                    start: point(10.0, 0.0),
                    end: point(10.0, 10.0),
                },
                PathSegment::Line {
                    start: point(10.0, 10.0),
                    end: point(0.0, 0.0),
                },
            ]
        );
    }

    #[test]
    fn split_open_line_segment_inserts_handle_free_anchor() {
        let path = PathGeometry::new(vec![anchor(0.0, 0.0), anchor(10.0, 0.0)], false)
            .expect("valid path");
        let original = path.clone();

        let (split, inserted_index) = path.split_segment(0, 0.5).expect("split path");

        assert_eq!(inserted_index, 1);
        assert_eq!(
            split.topology(),
            PathTopology {
                anchor_count: 3,
                segment_count: 2,
                open_subpath_count: 1,
                closed_subpath_count: 0,
            }
        );
        assert_eq!(
            split.anchors(),
            &[anchor(0.0, 0.0), anchor(5.0, 0.0), anchor(10.0, 0.0)]
        );
        assert_eq!(
            split.segments().expect("segments"),
            vec![
                PathSegment::Line {
                    start: point(0.0, 0.0),
                    end: point(5.0, 0.0),
                },
                PathSegment::Line {
                    start: point(5.0, 0.0),
                    end: point(10.0, 0.0),
                },
            ]
        );
        assert_eq!(path, original);
    }

    #[test]
    fn split_cubic_segment_preserves_exact_split_handles() {
        let curve = CubicBezier::new_unchecked(
            point(0.0, 0.0),
            point(0.0, 12.0),
            point(10.0, 12.0),
            point(10.0, 0.0),
        );
        let start = PathAnchor::new(curve.p0, Some(point(-2.0, 0.0)), Some(curve.p1))
            .expect("valid anchor");
        let end = PathAnchor::new(curve.p3, Some(curve.p2), Some(point(12.0, 0.0)))
            .expect("valid anchor");
        let path = PathGeometry::new(vec![start, end], false).expect("valid path");
        let original = path.clone();
        let (left, right) = curve.split(0.25).expect("split curve");

        let (split, inserted_index) = path.split_segment(0, 0.25).expect("split path");

        assert_eq!(inserted_index, 1);
        assert_eq!(
            split.anchors(),
            &[
                PathAnchor {
                    point: curve.p0,
                    in_handle: Some(point(-2.0, 0.0)),
                    out_handle: Some(left.p1),
                },
                PathAnchor {
                    point: left.p3,
                    in_handle: Some(left.p2),
                    out_handle: Some(right.p1),
                },
                PathAnchor {
                    point: curve.p3,
                    in_handle: Some(right.p2),
                    out_handle: Some(point(12.0, 0.0)),
                },
            ]
        );
        assert_eq!(
            split.segments().expect("segments"),
            vec![
                PathSegment::Cubic { curve: left },
                PathSegment::Cubic { curve: right },
            ]
        );
        assert_eq!(path, original);
    }

    #[test]
    fn split_closed_closing_edge_appends_anchor_and_keeps_path_closed() {
        let path = PathGeometry::new(
            vec![anchor(0.0, 0.0), anchor(10.0, 0.0), anchor(10.0, 10.0)],
            true,
        )
        .expect("valid path");
        let original = path.clone();

        let (split, inserted_index) = path.split_segment(2, 0.5).expect("split path");

        assert_eq!(inserted_index, 3);
        assert!(split.closed());
        assert_eq!(
            split.topology(),
            PathTopology {
                anchor_count: 4,
                segment_count: 4,
                open_subpath_count: 0,
                closed_subpath_count: 1,
            }
        );
        assert_eq!(
            split.anchors(),
            &[
                anchor(0.0, 0.0),
                anchor(10.0, 0.0),
                anchor(10.0, 10.0),
                anchor(5.0, 5.0),
            ]
        );
        assert_eq!(
            split.segments().expect("segments"),
            vec![
                PathSegment::Line {
                    start: point(0.0, 0.0),
                    end: point(10.0, 0.0),
                },
                PathSegment::Line {
                    start: point(10.0, 0.0),
                    end: point(10.0, 10.0),
                },
                PathSegment::Line {
                    start: point(10.0, 10.0),
                    end: point(5.0, 5.0),
                },
                PathSegment::Line {
                    start: point(5.0, 5.0),
                    end: point(0.0, 0.0),
                },
            ]
        );
        assert_eq!(path, original);
    }

    #[test]
    fn split_segment_rejects_invalid_index_and_parameters() {
        let path = PathGeometry::new(vec![anchor(0.0, 0.0), anchor(10.0, 0.0)], false)
            .expect("valid path");

        assert_eq!(
            path.split_segment(1, 0.5),
            Err(GeometryError::CountOutOfRange)
        );
        assert_eq!(
            path.split_segment(0, f64::NAN),
            Err(GeometryError::NonFiniteParameter)
        );
        assert_eq!(
            path.split_segment(0, -0.1),
            Err(GeometryError::ParameterOutOfRange)
        );
        assert_eq!(
            path.split_segment(0, 1.1),
            Err(GeometryError::ParameterOutOfRange)
        );
    }

    #[test]
    fn transform_applies_to_anchors_and_handles() {
        let source = PathGeometry::new(
            vec![
                PathAnchor::new(
                    point(1.0, 2.0),
                    Some(point(0.0, 2.0)),
                    Some(point(3.0, 2.0)),
                )
                .expect("valid anchor"),
            ],
            false,
        )
        .expect("valid path");
        let transform = AffineTransform::translation(10.0, -4.0).expect("valid transform");

        let transformed = source.transform(transform).expect("transformed path");

        assert_eq!(
            transformed.anchors(),
            &[PathAnchor {
                point: point(11.0, -2.0),
                in_handle: Some(point(10.0, -2.0)),
                out_handle: Some(point(13.0, -2.0)),
            }]
        );
    }

    #[test]
    fn flatten_mixed_line_and_cubic_segments_without_duplicate_starts() {
        let cubic_start =
            PathAnchor::new(point(10.0, 0.0), None, Some(point(15.0, 10.0))).expect("valid anchor");
        let cubic_end = PathAnchor::new(point(20.0, 0.0), Some(point(15.0, -10.0)), None)
            .expect("valid anchor");
        let path =
            PathGeometry::new(vec![anchor(0.0, 0.0), cubic_start, cubic_end], false).expect("path");

        let flattened = path.flatten(0.5).expect("flattened path");

        assert_eq!(flattened.first(), Some(&point(0.0, 0.0)));
        assert_eq!(flattened.get(1), Some(&point(10.0, 0.0)));
        assert_eq!(flattened.last(), Some(&point(20.0, 0.0)));
        assert!(flattened.len() > 3);
        for adjacent in flattened.windows(2) {
            let [start, end] = adjacent else {
                continue;
            };
            assert_ne!(*start, *end);
        }
    }

    #[test]
    fn project_returns_none_for_empty_and_one_anchor_paths() {
        let empty = PathGeometry::new(Vec::new(), false).expect("valid path");
        assert_eq!(empty.project(point(1.0, 2.0), 0.1), Ok(None));

        let one_anchor = PathGeometry::new(vec![anchor(0.0, 0.0)], false).expect("valid path");
        assert_eq!(one_anchor.project(point(1.0, 2.0), 0.1), Ok(None));
    }

    #[test]
    fn project_finds_nearest_segment_in_mixed_open_path() {
        let cubic_start =
            PathAnchor::new(point(10.0, 0.0), None, Some(point(10.0, 0.0))).expect("valid anchor");
        let cubic_end =
            PathAnchor::new(point(20.0, 0.0), Some(point(20.0, 0.0)), None).expect("valid anchor");
        let path =
            PathGeometry::new(vec![anchor(0.0, 0.0), cubic_start, cubic_end], false).expect("path");

        assert_eq!(
            path.project(point(15.0, 2.0), 0.1),
            Ok(Some(PathProjection {
                point: point(15.0, 0.0),
                segment_index: 1,
                segment_t: 0.5,
                distance_squared: 4.0,
            }))
        );
    }

    #[test]
    fn project_considers_closed_path_closing_segment() {
        let path = PathGeometry::new(
            vec![anchor(0.0, 0.0), anchor(10.0, 0.0), anchor(10.0, 10.0)],
            true,
        )
        .expect("valid path");

        assert_eq!(
            path.project(point(2.0, 7.0), 0.1),
            Ok(Some(PathProjection {
                point: point(4.5, 4.5),
                segment_index: 2,
                segment_t: 0.55,
                distance_squared: 12.5,
            }))
        );
    }

    #[test]
    fn project_keeps_earliest_segment_on_tie() {
        let path = PathGeometry::new(
            vec![anchor(0.0, 0.0), anchor(2.0, 0.0), anchor(2.0, 2.0)],
            false,
        )
        .expect("valid path");

        assert_eq!(
            path.project(point(1.0, 1.0), 0.1),
            Ok(Some(PathProjection {
                point: point(1.0, 0.0),
                segment_index: 0,
                segment_t: 0.5,
                distance_squared: 1.0,
            }))
        );
    }

    #[test]
    fn project_handles_repeated_anchor_zero_length_line() {
        let path = PathGeometry::new(
            vec![anchor(0.0, 0.0), anchor(0.0, 0.0), anchor(4.0, 0.0)],
            false,
        )
        .expect("valid path");

        assert_eq!(
            path.project(point(0.0, 2.0), 0.1),
            Ok(Some(PathProjection {
                point: point(0.0, 0.0),
                segment_index: 0,
                segment_t: 0.0,
                distance_squared: 4.0,
            }))
        );
    }

    #[test]
    fn project_rejects_invalid_query_point_and_tolerance() {
        let path =
            PathGeometry::new(vec![anchor(0.0, 0.0), anchor(1.0, 0.0)], false).expect("valid path");

        assert_eq!(
            path.project(point(f64::NAN, 0.0), 0.1),
            Err(GeometryError::NonFinitePoint)
        );
        assert_eq!(
            path.project(point(0.0, 0.0), f64::NAN),
            Err(GeometryError::NonFiniteTolerance)
        );
        assert_eq!(
            path.project(point(0.0, 0.0), 0.0),
            Err(GeometryError::NonPositiveTolerance)
        );
    }

    #[test]
    fn bounds_handles_empty_and_closed_lines() {
        assert_eq!(
            PathGeometry::new(Vec::new(), false)
                .expect("valid path")
                .bounds(),
            Ok(None)
        );

        let closed = PathGeometry::new(
            vec![anchor(0.0, 0.0), anchor(10.0, -4.0), anchor(-2.0, 6.0)],
            true,
        )
        .expect("valid path");

        assert_eq!(
            closed.bounds(),
            Ok(Some(RectBounds {
                min_x: -2.0,
                min_y: -4.0,
                max_x: 10.0,
                max_y: 6.0,
            }))
        );
    }

    #[test]
    fn bounds_uses_cubic_extrema_not_control_point_box() {
        let curve = CubicBezier::new_unchecked(
            point(0.0, 0.0),
            point(0.0, 10.0),
            point(10.0, 10.0),
            point(10.0, 0.0),
        );
        let start = PathAnchor::new(curve.p0, None, Some(curve.p1)).expect("valid anchor");
        let end = PathAnchor::new(curve.p3, Some(curve.p2), None).expect("valid anchor");
        let path = PathGeometry::new(vec![start, end], false).expect("valid path");

        assert_eq!(
            path.bounds(),
            Ok(Some(RectBounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 10.0,
                max_y: 7.5,
            }))
        );
    }

    #[test]
    fn project_cubic_segment_matches_cubic_projection() {
        let curve = CubicBezier::new_unchecked(
            point(0.0, 0.0),
            point(0.0, 10.0),
            point(10.0, 10.0),
            point(10.0, 0.0),
        );
        let start = PathAnchor::new(curve.p0, None, Some(curve.p1)).expect("valid anchor");
        let end = PathAnchor::new(curve.p3, Some(curve.p2), None).expect("valid anchor");
        let path = PathGeometry::new(vec![start, end], false).expect("valid path");
        let query = point(6.0, 8.0);
        let tolerance = 0.05;
        let cubic_projection =
            project_onto_cubic_bezier(query, curve, tolerance).expect("valid projection");

        assert_eq!(
            path.project(query, tolerance),
            Ok(Some(PathProjection {
                point: cubic_projection.point,
                segment_index: 0,
                segment_t: cubic_projection.t,
                distance_squared: cubic_projection.distance_squared,
            }))
        );
    }

    #[test]
    fn join_vectors_report_alignment_balance_and_zero_lengths() {
        let smooth = PathAnchor::new(
            point(0.0, 0.0),
            Some(point(-2.0, 0.0)),
            Some(point(4.0, 0.0)),
        )
        .expect("valid anchor");
        let join = smooth.join_vectors().expect("join vectors");

        assert_close(join.opposing_tangent_alignment(), 1.0);
        assert_close(join.handle_length_balance(), 0.5);
        assert_eq!(smooth.complete_handle_count(), 2);

        let same_direction = PathAnchor::new(
            point(0.0, 0.0),
            Some(point(2.0, 0.0)),
            Some(point(4.0, 0.0)),
        )
        .expect("valid anchor");
        assert_close(
            same_direction
                .join_vectors()
                .expect("join vectors")
                .opposing_tangent_alignment(),
            0.0,
        );

        let zero = PathJoinVectors {
            in_vector: point(0.0, 0.0),
            out_vector: point(0.0, 0.0),
            in_length: 0.0,
            out_length: 0.0,
        };
        assert_close(zero.opposing_tangent_alignment(), 0.0);
        assert_close(zero.handle_length_balance(), 0.0);
    }

    fn anchor(x: f64, y: f64) -> PathAnchor {
        PathAnchor::new(point(x, y), None, None).expect("valid anchor")
    }

    fn point(x: f64, y: f64) -> Point2 {
        Point2::new_unchecked(x, y)
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= EPSILON,
            "expected {actual} to be within {EPSILON} of {expected}"
        );
    }
}
