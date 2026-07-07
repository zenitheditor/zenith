use crate::{
    PerceptionDiagnostic, PerceptionSeverity,
    path_geometry::{geometry_anchor, geometry_path},
};
use zenith_core::PathAnchor;
use zenith_geometry::{ConstructionGuide, Point2, RectBounds};

#[derive(Debug, Clone, Copy)]
pub struct GridConformanceInput<'a> {
    pub anchors: &'a [PathAnchor],
    pub closed: bool,
    pub guides: &'a [ConstructionGuide],
}

#[derive(Debug, Clone, PartialEq)]
pub struct GridConformanceReport {
    pub guide_count: usize,
    pub evaluated_key_point_count: usize,
    pub maximum_guide_distance: f64,
    pub normalized_distance: f32,
    pub score: f32,
    pub diagnostics: Vec<PerceptionDiagnostic>,
}

pub fn grid_conformance(input: GridConformanceInput<'_>) -> GridConformanceReport {
    let mut diagnostics = Vec::new();
    let key_points = key_points(input.anchors, input.closed, &mut diagnostics);
    let valid_guides = valid_guides(input.guides, &mut diagnostics);
    let has_measurement = !key_points.is_empty() && !valid_guides.is_empty();

    if input.guides.is_empty() {
        diagnostics.push(PerceptionDiagnostic::new(
            "grid_conformance.no_guides",
            PerceptionSeverity::Info,
            "grid conformance requires at least one construction guide",
        ));
    }

    let has_warnings = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == PerceptionSeverity::Warning);
    let maximum_guide_distance = if has_measurement {
        maximum_nearest_distance(&key_points, &valid_guides).unwrap_or(0.0)
    } else {
        0.0
    };
    let normalized_distance = if has_warnings || !has_measurement {
        1.0
    } else {
        normalize_distance(maximum_guide_distance, &key_points)
    };
    let score = if has_warnings || !has_measurement {
        0.0
    } else {
        (1.0 - normalized_distance).clamp(0.0, 1.0)
    };

    if !has_warnings && has_measurement && score < 1.0 {
        diagnostics.push(PerceptionDiagnostic::new(
            "grid_conformance.low_conformance",
            PerceptionSeverity::Info,
            "path key points do not land exactly on the supplied construction guides",
        ));
    }

    GridConformanceReport {
        guide_count: input.guides.len(),
        evaluated_key_point_count: key_points.len(),
        maximum_guide_distance,
        normalized_distance,
        score,
        diagnostics,
    }
}

fn key_points(
    anchors: &[PathAnchor],
    closed: bool,
    diagnostics: &mut Vec<PerceptionDiagnostic>,
) -> Vec<Point2> {
    let mut points = Vec::with_capacity(anchors.len() + 5);
    for anchor in anchors {
        if let Some(anchor) = geometry_anchor(anchor) {
            points.push(anchor.point);
        }
    }

    match geometry_path(anchors, closed) {
        Ok(path) => match path.bounds() {
            Ok(Some(bounds)) => {
                points.push(Point2::new_unchecked(bounds.min_x, bounds.min_y));
                points.push(Point2::new_unchecked(bounds.max_x, bounds.min_y));
                points.push(Point2::new_unchecked(bounds.max_x, bounds.max_y));
                points.push(Point2::new_unchecked(bounds.min_x, bounds.max_y));
                points.push(Point2::new_unchecked(bounds.center_x(), bounds.center_y()));
            }
            Ok(None) => {}
            Err(_) => diagnostics.push(invalid_path_diagnostic()),
        },
        Err(()) => diagnostics.push(invalid_path_diagnostic()),
    }

    points
}

fn valid_guides(
    guides: &[ConstructionGuide],
    diagnostics: &mut Vec<PerceptionDiagnostic>,
) -> Vec<ConstructionGuide> {
    let mut valid = Vec::with_capacity(guides.len());
    for guide in guides {
        match guide.bounds() {
            Ok(_) => valid.push(*guide),
            Err(_) => diagnostics.push(PerceptionDiagnostic::new(
                "grid_conformance.invalid_guide",
                PerceptionSeverity::Warning,
                "construction guides must contain finite non-degenerate geometry",
            )),
        }
    }
    valid
}

fn maximum_nearest_distance(points: &[Point2], guides: &[ConstructionGuide]) -> Option<f64> {
    points
        .iter()
        .filter_map(|point| {
            guides
                .iter()
                .map(|guide| distance_to_guide(*point, *guide))
                .min_by(|left, right| left.total_cmp(right))
        })
        .max_by(|left, right| left.total_cmp(right))
}

fn distance_to_guide(point: Point2, guide: ConstructionGuide) -> f64 {
    match guide {
        ConstructionGuide::Segment { start, end } => {
            point.distance_squared_to_segment(start, end).sqrt()
        }
        ConstructionGuide::Circle { center, radius } => {
            (point.distance_squared(center).sqrt() - radius).abs()
        }
    }
}

fn normalize_distance(distance: f64, points: &[Point2]) -> f32 {
    if distance <= 0.0 {
        return 0.0;
    }

    let scale = key_point_scale(points);
    if scale <= 0.0 {
        1.0
    } else {
        (distance / scale).clamp(0.0, 1.0) as f32
    }
}

fn key_point_scale(points: &[Point2]) -> f64 {
    let mut bounds: Option<RectBounds> = None;
    for point in points {
        bounds = Some(match bounds {
            Some(bounds) => bounds.include_point(*point),
            None => RectBounds::from_point(*point),
        });
    }

    match bounds {
        Some(bounds) => bounds.width().hypot(bounds.height()),
        None => 0.0,
    }
}

fn invalid_path_diagnostic() -> PerceptionDiagnostic {
    PerceptionDiagnostic::new(
        "grid_conformance.invalid_path_geometry",
        PerceptionSeverity::Warning,
        "grid conformance requires complete finite px anchor and handle coordinates",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{Dimension, Unit};

    #[test]
    fn exact_segment_alignment_scores_one() {
        let anchors = [anchor(0.0, 0.0), anchor(10.0, 0.0)];
        let guide =
            ConstructionGuide::segment(point(0.0, 0.0), point(10.0, 0.0)).expect("valid guide");

        let report = grid_conformance(GridConformanceInput {
            anchors: &anchors,
            closed: false,
            guides: &[guide],
        });

        assert_eq!(report.guide_count, 1);
        assert_eq!(report.evaluated_key_point_count, 7);
        assert_eq!(report.maximum_guide_distance, 0.0);
        assert_eq!(report.normalized_distance, 0.0);
        assert_eq!(report.score, 1.0);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn circle_alignment_scores_one() {
        let anchors = [anchor(10.0, 0.0)];
        let guide = ConstructionGuide::circle(point(0.0, 0.0), 10.0).expect("valid guide");

        let report = grid_conformance(GridConformanceInput {
            anchors: &anchors,
            closed: false,
            guides: &[guide],
        });

        assert_eq!(report.maximum_guide_distance, 0.0);
        assert_eq!(report.score, 1.0);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn off_grid_path_lowers_score() {
        let anchors = [anchor(1.0, 1.0), anchor(11.0, 1.0)];
        let guide =
            ConstructionGuide::segment(point(0.0, 0.0), point(10.0, 0.0)).expect("valid guide");

        let report = grid_conformance(GridConformanceInput {
            anchors: &anchors,
            closed: false,
            guides: &[guide],
        });

        assert!((report.maximum_guide_distance - 2.0_f64.sqrt()).abs() < 0.000_001);
        assert!(report.normalized_distance > 0.0);
        assert!(report.score < 1.0);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "grid_conformance.low_conformance")
        );
    }

    #[test]
    fn invalid_path_geometry_scores_zero() {
        let anchors = [PathAnchor {
            x: Some(px(0.0)),
            y: None,
            kind: None,
            in_x: None,
            in_y: None,
            out_x: None,
            out_y: None,
        }];
        let guide =
            ConstructionGuide::segment(point(0.0, 0.0), point(10.0, 0.0)).expect("valid guide");

        let report = grid_conformance(GridConformanceInput {
            anchors: &anchors,
            closed: false,
            guides: &[guide],
        });

        assert_eq!(report.score, 0.0);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "grid_conformance.invalid_path_geometry")
        );
    }

    #[test]
    fn invalid_guides_score_zero() {
        let anchors = [anchor(0.0, 0.0)];
        let guide = ConstructionGuide::Circle {
            center: point(0.0, 0.0),
            radius: 0.0,
        };

        let report = grid_conformance(GridConformanceInput {
            anchors: &anchors,
            closed: false,
            guides: &[guide],
        });

        assert_eq!(report.score, 0.0);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "grid_conformance.invalid_guide")
        );
    }

    #[test]
    fn empty_guides_emit_info_and_score_zero() {
        let anchors = [anchor(0.0, 0.0)];

        let report = grid_conformance(GridConformanceInput {
            anchors: &anchors,
            closed: false,
            guides: &[],
        });

        assert_eq!(report.guide_count, 0);
        assert_eq!(report.maximum_guide_distance, 0.0);
        assert_eq!(report.normalized_distance, 1.0);
        assert_eq!(report.score, 0.0);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "grid_conformance.no_guides")
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

    fn point(x: f64, y: f64) -> Point2 {
        Point2::new(x, y).expect("valid point")
    }

    fn px(value: f64) -> Dimension {
        Dimension {
            value,
            unit: Unit::Px,
        }
    }

    #[test]
    fn normalizes_by_path_key_point_extent() {
        let points = [
            point(0.0, 0.0),
            point(3.0, 0.0),
            point(3.0, 4.0),
            point(0.0, 4.0),
        ];

        assert_eq!(key_point_scale(&points), 5.0);
    }

    #[test]
    fn empty_scale_normalizes_to_one() {
        assert_eq!(normalize_distance(2.0, &[]), 1.0);
    }
}
