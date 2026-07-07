use crate::{AffineTransform, CubicBezier, GeometryError, Point2, validation::validate_tolerance};

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

        let in_length = in_vector.x.hypot(in_vector.y);
        let out_length = out_vector.x.hypot(out_vector.y);
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

impl PathJoinVectors {
    #[must_use]
    pub fn opposing_tangent_alignment(self) -> f64 {
        if self.in_length <= ZERO_LENGTH_EPSILON || self.out_length <= ZERO_LENGTH_EPSILON {
            return 0.0;
        }

        let normalized_in = Point2::new_unchecked(
            self.in_vector.x / self.in_length,
            self.in_vector.y / self.in_length,
        );
        let normalized_out = Point2::new_unchecked(
            self.out_vector.x / self.out_length,
            self.out_vector.y / self.out_length,
        );
        let dot = normalized_in
            .x
            .mul_add(normalized_out.x, normalized_in.y * normalized_out.y);

        (-dot).max(0.0).clamp(0.0, 1.0)
    }

    #[must_use]
    pub fn handle_length_balance(self) -> f64 {
        let shorter = self.in_length.min(self.out_length);
        let longer = self.in_length.max(self.out_length);
        if longer <= ZERO_LENGTH_EPSILON {
            0.0
        } else {
            (shorter / longer).clamp(0.0, 1.0)
        }
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
    let end_index = if index + 1 < anchors.len() {
        index + 1
    } else if closed {
        0
    } else {
        return None;
    };
    let end = anchors.get(end_index).copied()?;
    Some((start, end))
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
