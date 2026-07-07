use crate::{PerceptionDiagnostic, PerceptionSeverity, path_geometry::geometry_path};
use zenith_core::PathAnchor;
use zenith_geometry::{
    ClosedPolylineOutlinePolicy, OpenPolylineOutlinePolicy, PathGeometry, PathOutline, Point2,
    RectBounds, outline_path_geometry,
};

#[derive(Debug, Clone, Copy)]
pub struct PathOutlinePerceptionInput<'a> {
    pub anchors: &'a [PathAnchor],
    pub closed: bool,
    pub tolerance: f64,
    pub stroke_width: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathOutlineKind {
    Open,
    Closed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PathOutlinePerceptionReport {
    pub anchor_count: usize,
    pub segment_count: usize,
    pub closed: bool,
    pub outline_kind: Option<PathOutlineKind>,
    pub outline_point_count: Option<usize>,
    pub left_ring_point_count: Option<usize>,
    pub right_ring_point_count: Option<usize>,
    pub bounds: Option<RectBounds>,
    pub signed_area: Option<f64>,
    pub complexity_score: f32,
    pub diagnostics: Vec<PerceptionDiagnostic>,
}

pub fn path_outline(input: PathOutlinePerceptionInput<'_>) -> PathOutlinePerceptionReport {
    let anchor_count = input.anchors.len();
    let topology = PathGeometry::topology_for(anchor_count, input.closed);
    let mut diagnostics = Vec::new();

    let outline = if !input.stroke_width.is_finite() || input.stroke_width <= 0.0 {
        diagnostics.push(PerceptionDiagnostic::new(
            "path_outline.invalid_stroke_width",
            PerceptionSeverity::Info,
            "path outline perception requires a positive finite stroke width",
        ));
        None
    } else {
        match geometry_path(input.anchors, input.closed) {
            Ok(path) => match outline_path_geometry(
                &path,
                input.tolerance,
                input.stroke_width,
                OpenPolylineOutlinePolicy::default(),
                ClosedPolylineOutlinePolicy::default(),
            ) {
                Ok(outline) => outline,
                Err(_) => {
                    diagnostics.push(PerceptionDiagnostic::new(
                        "path_outline.invalid_outline_input",
                        PerceptionSeverity::Warning,
                        "path outline perception requires valid tolerance and outline geometry",
                    ));
                    None
                }
            },
            Err(()) => {
                diagnostics.push(PerceptionDiagnostic::new(
                    "path_outline.invalid_geometry",
                    PerceptionSeverity::Warning,
                    "path outline perception requires complete finite px anchor and handle coordinates",
                ));
                None
            }
        }
    };

    let summary = outline.as_ref().map(outline_summary);
    PathOutlinePerceptionReport {
        anchor_count,
        segment_count: topology.segment_count,
        closed: input.closed,
        outline_kind: summary.map(|summary| summary.kind),
        outline_point_count: summary.map(|summary| summary.outline_point_count),
        left_ring_point_count: summary.and_then(|summary| summary.left_ring_point_count),
        right_ring_point_count: summary.and_then(|summary| summary.right_ring_point_count),
        bounds: summary.and_then(|summary| summary.bounds),
        signed_area: summary.map(|summary| summary.signed_area),
        complexity_score: complexity_score(summary.map(|summary| summary.outline_point_count)),
        diagnostics,
    }
}

#[derive(Debug, Clone, Copy)]
struct OutlineSummary {
    kind: PathOutlineKind,
    outline_point_count: usize,
    left_ring_point_count: Option<usize>,
    right_ring_point_count: Option<usize>,
    bounds: Option<RectBounds>,
    signed_area: f64,
}

fn outline_summary(outline: &PathOutline) -> OutlineSummary {
    match outline {
        PathOutline::Open(open) => OutlineSummary {
            kind: PathOutlineKind::Open,
            outline_point_count: open.points.len(),
            left_ring_point_count: None,
            right_ring_point_count: None,
            bounds: bounds_for_points(open.points.iter().copied()),
            signed_area: signed_area(open.points.iter().copied()),
        },
        PathOutline::Closed(closed) => {
            let outline_point_count = closed.left_ring.len() + closed.right_ring.len();
            OutlineSummary {
                kind: PathOutlineKind::Closed,
                outline_point_count,
                left_ring_point_count: Some(closed.left_ring.len()),
                right_ring_point_count: Some(closed.right_ring.len()),
                bounds: bounds_for_points(
                    closed
                        .left_ring
                        .iter()
                        .copied()
                        .chain(closed.right_ring.iter().copied()),
                ),
                signed_area: signed_area(closed.left_ring.iter().copied())
                    + signed_area(closed.right_ring.iter().copied()),
            }
        }
    }
}

fn complexity_score(point_count: Option<usize>) -> f32 {
    let Some(point_count) = point_count else {
        return 0.0;
    };
    if point_count == 0 {
        0.0
    } else {
        (1.0 / (point_count as f32 / 8.0).max(1.0)).clamp(0.0, 1.0)
    }
}

fn bounds_for_points(points: impl IntoIterator<Item = Point2>) -> Option<RectBounds> {
    let mut points = points.into_iter();
    let first = points.next()?;
    let mut bounds = RectBounds::from_point(first);
    for point in points {
        bounds = bounds.include_point(point);
    }
    if bounds.is_valid() {
        Some(bounds)
    } else {
        None
    }
}

fn signed_area(points: impl IntoIterator<Item = Point2>) -> f64 {
    let mut points = points.into_iter();
    let Some(first) = points.next() else {
        return 0.0;
    };
    let Some(second) = points.next() else {
        return 0.0;
    };
    let mut area = 0.0;
    let mut previous = first;
    let mut current = second;
    loop {
        area += previous.x * current.y - current.x * previous.y;
        previous = current;
        match points.next() {
            Some(next) => current = next,
            None => break,
        }
    }
    area += previous.x * first.y - first.x * previous.y;
    area * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{Dimension, Unit};

    #[test]
    fn open_path_reports_outline_summary() {
        let anchors = [anchor(0.0, 0.0), anchor(10.0, 0.0)];

        let report = path_outline(PathOutlinePerceptionInput {
            anchors: &anchors,
            closed: false,
            tolerance: 0.25,
            stroke_width: 4.0,
        });

        assert_eq!(report.anchor_count, 2);
        assert_eq!(report.segment_count, 1);
        assert_eq!(report.outline_kind, Some(PathOutlineKind::Open));
        assert_eq!(report.outline_point_count, Some(4));
        assert_eq!(report.left_ring_point_count, None);
        assert_eq!(report.right_ring_point_count, None);
        assert_eq!(report.signed_area, Some(-40.0));
        assert_eq!(report.complexity_score, 1.0);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn closed_path_reports_ring_counts() {
        let anchors = [
            anchor(0.0, 0.0),
            anchor(10.0, 0.0),
            anchor(10.0, 10.0),
            anchor(0.0, 10.0),
        ];

        let report = path_outline(PathOutlinePerceptionInput {
            anchors: &anchors,
            closed: true,
            tolerance: 0.25,
            stroke_width: 4.0,
        });

        assert_eq!(report.outline_kind, Some(PathOutlineKind::Closed));
        assert_eq!(report.outline_point_count, Some(12));
        assert_eq!(report.left_ring_point_count, Some(4));
        assert_eq!(report.right_ring_point_count, Some(8));
        assert_eq!(
            report.bounds,
            Some(RectBounds {
                min_x: -2.0,
                min_y: -2.0,
                max_x: 12.0,
                max_y: 12.0,
            })
        );
        assert_eq!(report.complexity_score, 2.0 / 3.0);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn zero_stroke_reports_no_measurement() {
        let anchors = [anchor(0.0, 0.0), anchor(10.0, 0.0)];

        let report = path_outline(PathOutlinePerceptionInput {
            anchors: &anchors,
            closed: false,
            tolerance: 0.25,
            stroke_width: 0.0,
        });

        assert_eq!(report.outline_kind, None);
        assert_eq!(report.outline_point_count, None);
        assert_eq!(report.complexity_score, 0.0);
        assert_eq!(
            report.diagnostics,
            vec![PerceptionDiagnostic::new(
                "path_outline.invalid_stroke_width",
                PerceptionSeverity::Info,
                "path outline perception requires a positive finite stroke width",
            )]
        );
    }

    #[test]
    fn invalid_anchor_geometry_reports_warning() {
        let anchors = [PathAnchor {
            x: Some(px(0.0)),
            y: None,
            kind: None,
            in_x: None,
            in_y: None,
            out_x: None,
            out_y: None,
        }];

        let report = path_outline(PathOutlinePerceptionInput {
            anchors: &anchors,
            closed: false,
            tolerance: 0.25,
            stroke_width: 4.0,
        });

        assert_eq!(report.outline_kind, None);
        assert_eq!(report.complexity_score, 0.0);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "path_outline.invalid_geometry")
        );
    }

    #[test]
    fn invalid_tolerance_reports_warning() {
        let anchors = [anchor(0.0, 0.0), anchor(10.0, 0.0)];

        let report = path_outline(PathOutlinePerceptionInput {
            anchors: &anchors,
            closed: false,
            tolerance: 0.0,
            stroke_width: 4.0,
        });

        assert_eq!(report.outline_kind, None);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "path_outline.invalid_outline_input")
        );
    }

    fn anchor(x: f64, y: f64) -> PathAnchor {
        PathAnchor {
            x: Some(px(x)),
            y: Some(px(y)),
            kind: None,
            in_x: None,
            in_y: None,
            out_x: None,
            out_y: None,
        }
    }

    fn px(value: f64) -> Dimension {
        Dimension {
            value,
            unit: Unit::Px,
        }
    }
}
