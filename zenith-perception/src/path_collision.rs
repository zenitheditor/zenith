use crate::{
    PerceptionDiagnostic, PerceptionSeverity, VectorPathContourInput, path_geometry::geometry_path,
};
use zenith_core::PathAnchor;
use zenith_geometry::{
    CompoundPathGeometry, CompoundPathGeometryNearestPoints, PathGeometryNearestPoints, Point2,
    collect_compound_path_geometry_intersections, collect_path_geometry_intersections,
    nearest_compound_path_geometry_points, nearest_path_geometry_points,
};

#[derive(Debug, Clone, Copy)]
pub struct PathCollisionInput<'a> {
    pub first_anchors: &'a [PathAnchor],
    pub first_closed: bool,
    pub second_anchors: &'a [PathAnchor],
    pub second_closed: bool,
    pub tolerance: f64,
    pub required_clearance: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct CompoundPathCollisionInput<'a> {
    pub first_contours: &'a [VectorPathContourInput<'a>],
    pub second_contours: &'a [VectorPathContourInput<'a>],
    pub tolerance: f64,
    pub required_clearance: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PathCollisionReport {
    pub first_anchor_count: usize,
    pub second_anchor_count: usize,
    pub intersection_count: usize,
    pub nearest: Option<PathCollisionNearestPoints>,
    pub minimum_distance: f64,
    pub required_clearance: f64,
    pub clearance_ratio: f32,
    pub score: f32,
    pub diagnostics: Vec<PerceptionDiagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompoundPathCollisionReport {
    pub first_contour_count: usize,
    pub second_contour_count: usize,
    pub first_anchor_count: usize,
    pub second_anchor_count: usize,
    pub intersection_count: usize,
    pub nearest: Option<CompoundPathCollisionNearestPoints>,
    pub minimum_distance: f64,
    pub required_clearance: f64,
    pub clearance_ratio: f32,
    pub score: f32,
    pub diagnostics: Vec<PerceptionDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathCollisionNearestPoints {
    pub first_point: Point2,
    pub second_point: Point2,
    pub first_segment_index: usize,
    pub second_segment_index: usize,
    pub first_segment_t: f64,
    pub second_segment_t: f64,
    pub distance: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompoundPathCollisionNearestPoints {
    pub first_contour_index: usize,
    pub second_contour_index: usize,
    pub points: PathCollisionNearestPoints,
}

pub fn path_collision(input: PathCollisionInput<'_>) -> PathCollisionReport {
    let first_anchor_count = input.first_anchors.len();
    let second_anchor_count = input.second_anchors.len();
    let mut diagnostics = Vec::new();

    let first = geometry_path(input.first_anchors, input.first_closed);
    let second = geometry_path(input.second_anchors, input.second_closed);
    let required_clearance = valid_required_clearance(input.required_clearance, &mut diagnostics);

    let (intersection_count, nearest) = match (first, second) {
        (Ok(first), Ok(second)) => {
            measure_paths(&first, &second, input.tolerance, &mut diagnostics)
        }
        (Err(()), Ok(_)) => {
            diagnostics.push(invalid_path_diagnostic(
                "path_collision.invalid_first_geometry",
            ));
            (0, None)
        }
        (Ok(_), Err(())) => {
            diagnostics.push(invalid_path_diagnostic(
                "path_collision.invalid_second_geometry",
            ));
            (0, None)
        }
        (Err(()), Err(())) => {
            diagnostics.push(invalid_path_diagnostic(
                "path_collision.invalid_first_geometry",
            ));
            diagnostics.push(invalid_path_diagnostic(
                "path_collision.invalid_second_geometry",
            ));
            (0, None)
        }
    };

    if first_anchor_count == 0 || second_anchor_count == 0 {
        diagnostics.push(PerceptionDiagnostic::new(
            "path_collision.empty_path",
            PerceptionSeverity::Info,
            "path collision requires two non-empty paths",
        ));
    }

    let minimum_distance = nearest.map_or(0.0, |nearest| nearest.distance);
    let has_warning = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == PerceptionSeverity::Warning);
    let has_measurement = nearest.is_some();
    let clearance_ratio = clearance_ratio(minimum_distance, required_clearance);
    let score = collision_score(has_warning, has_measurement, clearance_ratio);

    if !has_warning && has_measurement && intersection_count > 0 {
        diagnostics.push(PerceptionDiagnostic::new(
            "path_collision.intersection",
            PerceptionSeverity::Info,
            "paths intersect or overlap after deterministic flattening",
        ));
    } else if !has_warning && has_measurement && clearance_ratio < 1.0 {
        diagnostics.push(PerceptionDiagnostic::new(
            "path_collision.insufficient_clearance",
            PerceptionSeverity::Info,
            "minimum path clearance is below the required distance",
        ));
    }

    PathCollisionReport {
        first_anchor_count,
        second_anchor_count,
        intersection_count,
        nearest,
        minimum_distance,
        required_clearance,
        clearance_ratio,
        score,
        diagnostics,
    }
}

pub fn compound_path_collision(
    input: CompoundPathCollisionInput<'_>,
) -> CompoundPathCollisionReport {
    let first_anchor_count = contour_anchor_count(input.first_contours);
    let second_anchor_count = contour_anchor_count(input.second_contours);
    let mut diagnostics = Vec::new();

    let first = compound_geometry(
        input.first_contours,
        "path_collision.invalid_first_geometry",
    );
    let second = compound_geometry(
        input.second_contours,
        "path_collision.invalid_second_geometry",
    );
    let required_clearance = valid_required_clearance(input.required_clearance, &mut diagnostics);

    let (intersection_count, nearest) = match (first, second) {
        (Ok(first), Ok(second)) => {
            measure_compound_paths(&first, &second, input.tolerance, &mut diagnostics)
        }
        (Err(diagnostic), Ok(_)) => {
            diagnostics.push(diagnostic);
            (0, None)
        }
        (Ok(_), Err(diagnostic)) => {
            diagnostics.push(diagnostic);
            (0, None)
        }
        (Err(first_diagnostic), Err(second_diagnostic)) => {
            diagnostics.push(first_diagnostic);
            diagnostics.push(second_diagnostic);
            (0, None)
        }
    };

    if first_anchor_count == 0 || second_anchor_count == 0 {
        diagnostics.push(PerceptionDiagnostic::new(
            "path_collision.empty_path",
            PerceptionSeverity::Info,
            "path collision requires two non-empty paths",
        ));
    }

    let minimum_distance = nearest.map_or(0.0, |nearest| nearest.points.distance);
    let has_warning = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == PerceptionSeverity::Warning);
    let has_measurement = nearest.is_some();
    let clearance_ratio = clearance_ratio(minimum_distance, required_clearance);
    let score = collision_score(has_warning, has_measurement, clearance_ratio);

    if !has_warning && has_measurement && intersection_count > 0 {
        diagnostics.push(PerceptionDiagnostic::new(
            "path_collision.intersection",
            PerceptionSeverity::Info,
            "paths intersect or overlap after deterministic flattening",
        ));
    } else if !has_warning && has_measurement && clearance_ratio < 1.0 {
        diagnostics.push(PerceptionDiagnostic::new(
            "path_collision.insufficient_clearance",
            PerceptionSeverity::Info,
            "minimum path clearance is below the required distance",
        ));
    }

    CompoundPathCollisionReport {
        first_contour_count: input.first_contours.len(),
        second_contour_count: input.second_contours.len(),
        first_anchor_count,
        second_anchor_count,
        intersection_count,
        nearest,
        minimum_distance,
        required_clearance,
        clearance_ratio,
        score,
        diagnostics,
    }
}

fn measure_paths(
    first: &zenith_geometry::PathGeometry,
    second: &zenith_geometry::PathGeometry,
    tolerance: f64,
    diagnostics: &mut Vec<PerceptionDiagnostic>,
) -> (usize, Option<PathCollisionNearestPoints>) {
    let intersection_count = match collect_path_geometry_intersections(first, second, tolerance) {
        Ok(report) => report.intersections.len(),
        Err(_) => {
            diagnostics.push(invalid_tolerance_diagnostic());
            return (0, None);
        }
    };

    match nearest_path_geometry_points(first, second, tolerance) {
        Ok(nearest) => (
            intersection_count,
            nearest.map(PathCollisionNearestPoints::from),
        ),
        Err(_) => {
            diagnostics.push(invalid_tolerance_diagnostic());
            (intersection_count, None)
        }
    }
}

fn measure_compound_paths(
    first: &CompoundPathGeometry,
    second: &CompoundPathGeometry,
    tolerance: f64,
    diagnostics: &mut Vec<PerceptionDiagnostic>,
) -> (usize, Option<CompoundPathCollisionNearestPoints>) {
    let intersection_count =
        match collect_compound_path_geometry_intersections(first, second, tolerance) {
            Ok(report) => report.intersections.len(),
            Err(_) => {
                diagnostics.push(invalid_tolerance_diagnostic());
                return (0, None);
            }
        };

    match nearest_compound_path_geometry_points(first, second, tolerance) {
        Ok(nearest) => (
            intersection_count,
            nearest.map(CompoundPathCollisionNearestPoints::from),
        ),
        Err(_) => {
            diagnostics.push(invalid_tolerance_diagnostic());
            (intersection_count, None)
        }
    }
}

fn compound_geometry(
    contours: &[VectorPathContourInput<'_>],
    invalid_code: &'static str,
) -> Result<CompoundPathGeometry, PerceptionDiagnostic> {
    let mut geometry_contours = Vec::with_capacity(contours.len());
    for contour in contours {
        match geometry_path(contour.anchors, contour.closed) {
            Ok(geometry) => geometry_contours.push(geometry),
            Err(()) => return Err(invalid_path_diagnostic(invalid_code)),
        }
    }

    Ok(CompoundPathGeometry::new(geometry_contours))
}

fn contour_anchor_count(contours: &[VectorPathContourInput<'_>]) -> usize {
    contours.iter().map(|contour| contour.anchors.len()).sum()
}

fn valid_required_clearance(
    required_clearance: f64,
    diagnostics: &mut Vec<PerceptionDiagnostic>,
) -> f64 {
    if required_clearance.is_finite() && required_clearance > 0.0 {
        required_clearance
    } else {
        diagnostics.push(PerceptionDiagnostic::new(
            "path_collision.invalid_required_clearance",
            PerceptionSeverity::Warning,
            "required path clearance must be a positive finite distance",
        ));
        0.0
    }
}

fn clearance_ratio(minimum_distance: f64, required_clearance: f64) -> f32 {
    if required_clearance <= 0.0 {
        0.0
    } else {
        (minimum_distance / required_clearance).clamp(0.0, 1.0) as f32
    }
}

fn collision_score(has_warning: bool, has_measurement: bool, clearance_ratio: f32) -> f32 {
    if has_warning || !has_measurement {
        0.0
    } else {
        clearance_ratio
    }
}

fn invalid_path_diagnostic(code: &'static str) -> PerceptionDiagnostic {
    PerceptionDiagnostic::new(
        code,
        PerceptionSeverity::Warning,
        "path collision requires complete finite px anchor and handle coordinates",
    )
}

fn invalid_tolerance_diagnostic() -> PerceptionDiagnostic {
    PerceptionDiagnostic::new(
        "path_collision.invalid_tolerance",
        PerceptionSeverity::Warning,
        "path collision tolerance must be a positive finite distance",
    )
}

impl From<PathGeometryNearestPoints> for PathCollisionNearestPoints {
    fn from(nearest: PathGeometryNearestPoints) -> Self {
        Self {
            first_point: nearest.first_point,
            second_point: nearest.second_point,
            first_segment_index: nearest.first_segment_index,
            second_segment_index: nearest.second_segment_index,
            first_segment_t: nearest.first_segment_t,
            second_segment_t: nearest.second_segment_t,
            distance: nearest.distance_squared.sqrt(),
        }
    }
}

impl From<CompoundPathGeometryNearestPoints> for CompoundPathCollisionNearestPoints {
    fn from(nearest: CompoundPathGeometryNearestPoints) -> Self {
        Self {
            first_contour_index: nearest.first_contour_index,
            second_contour_index: nearest.second_contour_index,
            points: PathCollisionNearestPoints::from(nearest.nearest),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{Dimension, Unit};

    #[test]
    fn intersecting_paths_report_zero_clearance() {
        let horizontal = [anchor(0.0, 0.0), anchor(10.0, 0.0)];
        let vertical = [anchor(5.0, -5.0), anchor(5.0, 5.0)];

        let report = path_collision(PathCollisionInput {
            first_anchors: &horizontal,
            first_closed: false,
            second_anchors: &vertical,
            second_closed: false,
            tolerance: 0.1,
            required_clearance: 2.0,
        });

        assert_eq!(report.first_anchor_count, 2);
        assert_eq!(report.second_anchor_count, 2);
        assert_eq!(report.intersection_count, 1);
        assert_eq!(report.minimum_distance, 0.0);
        assert_eq!(report.clearance_ratio, 0.0);
        assert_eq!(report.score, 0.0);
        assert_eq!(
            report.diagnostics,
            vec![PerceptionDiagnostic::new(
                "path_collision.intersection",
                PerceptionSeverity::Info,
                "paths intersect or overlap after deterministic flattening",
            )]
        );
    }

    #[test]
    fn separated_paths_score_by_required_clearance() {
        let first = [anchor(0.0, 0.0), anchor(4.0, 0.0)];
        let second = [anchor(6.0, 3.0), anchor(10.0, 3.0)];

        let report = path_collision(PathCollisionInput {
            first_anchors: &first,
            first_closed: false,
            second_anchors: &second,
            second_closed: false,
            tolerance: 0.1,
            required_clearance: 5.0,
        });

        assert_eq!(report.intersection_count, 0);
        assert_eq!(report.minimum_distance, 13.0_f64.sqrt());
        assert_eq!(report.clearance_ratio, (13.0_f64.sqrt() / 5.0) as f32);
        assert_eq!(report.score, report.clearance_ratio);
        assert_eq!(
            report.diagnostics,
            vec![PerceptionDiagnostic::new(
                "path_collision.insufficient_clearance",
                PerceptionSeverity::Info,
                "minimum path clearance is below the required distance",
            )]
        );
    }

    #[test]
    fn sufficient_clearance_scores_one_without_diagnostics() {
        let first = [anchor(0.0, 0.0), anchor(4.0, 0.0)];
        let second = [anchor(6.0, 3.0), anchor(10.0, 3.0)];

        let report = path_collision(PathCollisionInput {
            first_anchors: &first,
            first_closed: false,
            second_anchors: &second,
            second_closed: false,
            tolerance: 0.1,
            required_clearance: 2.0,
        });

        assert_eq!(report.intersection_count, 0);
        assert_eq!(report.clearance_ratio, 1.0);
        assert_eq!(report.score, 1.0);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn empty_path_reports_no_measurement() {
        let second = [anchor(0.0, 0.0), anchor(10.0, 0.0)];

        let report = path_collision(PathCollisionInput {
            first_anchors: &[],
            first_closed: false,
            second_anchors: &second,
            second_closed: false,
            tolerance: 0.1,
            required_clearance: 2.0,
        });

        assert_eq!(report.intersection_count, 0);
        assert_eq!(report.nearest, None);
        assert_eq!(report.score, 0.0);
        assert_eq!(
            report.diagnostics,
            vec![PerceptionDiagnostic::new(
                "path_collision.empty_path",
                PerceptionSeverity::Info,
                "path collision requires two non-empty paths",
            )]
        );
    }

    #[test]
    fn invalid_geometry_and_tolerance_are_warnings() {
        let first = [PathAnchor {
            x: Some(px(0.0)),
            y: Some(px(0.0)),
            kind: None,
            in_x: None,
            in_y: None,
            out_x: Some(px(1.0)),
            out_y: None,
        }];
        let second = [anchor(0.0, 0.0), anchor(10.0, 0.0)];

        let report = path_collision(PathCollisionInput {
            first_anchors: &first,
            first_closed: false,
            second_anchors: &second,
            second_closed: false,
            tolerance: 0.0,
            required_clearance: -1.0,
        });

        assert_eq!(report.nearest, None);
        assert_eq!(report.score, 0.0);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "path_collision.invalid_first_geometry"),
            "expected invalid first geometry diagnostic; got {:?}",
            report.diagnostics
        );
        assert!(
            report.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == "path_collision.invalid_required_clearance"
            }),
            "expected invalid required clearance diagnostic; got {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn invalid_tolerance_is_a_warning() {
        let first = [anchor(0.0, 0.0), anchor(10.0, 0.0)];
        let second = [anchor(5.0, -5.0), anchor(5.0, 5.0)];

        let report = path_collision(PathCollisionInput {
            first_anchors: &first,
            first_closed: false,
            second_anchors: &second,
            second_closed: false,
            tolerance: 0.0,
            required_clearance: 1.0,
        });

        assert_eq!(report.nearest, None);
        assert_eq!(report.score, 0.0);
        assert_eq!(
            report.diagnostics,
            vec![PerceptionDiagnostic::new(
                "path_collision.invalid_tolerance",
                PerceptionSeverity::Warning,
                "path collision tolerance must be a positive finite distance",
            )]
        );
    }

    #[test]
    fn compound_collision_reports_contour_indices() {
        let first_a = [anchor(0.0, 10.0), anchor(10.0, 10.0)];
        let first_b = [anchor(0.0, 0.0), anchor(10.0, 0.0)];
        let second_a = [anchor(20.0, 20.0), anchor(30.0, 20.0)];
        let second_b = [anchor(5.0, -5.0), anchor(5.0, 5.0)];
        let first_contours = [
            VectorPathContourInput {
                anchors: &first_a,
                closed: false,
            },
            VectorPathContourInput {
                anchors: &first_b,
                closed: false,
            },
        ];
        let second_contours = [
            VectorPathContourInput {
                anchors: &second_a,
                closed: false,
            },
            VectorPathContourInput {
                anchors: &second_b,
                closed: false,
            },
        ];

        let report = compound_path_collision(CompoundPathCollisionInput {
            first_contours: &first_contours,
            second_contours: &second_contours,
            tolerance: 0.1,
            required_clearance: 2.0,
        });

        assert_eq!(report.first_contour_count, 2);
        assert_eq!(report.second_contour_count, 2);
        assert_eq!(report.first_anchor_count, 4);
        assert_eq!(report.second_anchor_count, 4);
        assert_eq!(report.intersection_count, 1);
        let nearest = report.nearest.expect("nearest");
        assert_eq!(nearest.first_contour_index, 1);
        assert_eq!(nearest.second_contour_index, 1);
        assert_eq!(nearest.points.distance, 0.0);
        assert_eq!(report.score, 0.0);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "path_collision.intersection")
        );
    }

    #[test]
    fn compound_collision_scores_clearance_between_nearest_contours() {
        let first_a = [anchor(0.0, 0.0), anchor(4.0, 0.0)];
        let first_b = [anchor(20.0, 20.0), anchor(24.0, 20.0)];
        let second = [anchor(6.0, 3.0), anchor(10.0, 3.0)];
        let first_contours = [
            VectorPathContourInput {
                anchors: &first_a,
                closed: false,
            },
            VectorPathContourInput {
                anchors: &first_b,
                closed: false,
            },
        ];
        let second_contours = [VectorPathContourInput {
            anchors: &second,
            closed: false,
        }];

        let report = compound_path_collision(CompoundPathCollisionInput {
            first_contours: &first_contours,
            second_contours: &second_contours,
            tolerance: 0.1,
            required_clearance: 5.0,
        });

        assert_eq!(report.intersection_count, 0);
        assert_eq!(report.minimum_distance, 13.0_f64.sqrt());
        assert_eq!(report.nearest.expect("nearest").first_contour_index, 0);
        assert_eq!(report.clearance_ratio, (13.0_f64.sqrt() / 5.0) as f32);
        assert_eq!(report.score, report.clearance_ratio);
    }

    #[test]
    fn compound_collision_reports_invalid_geometry() {
        let invalid = [PathAnchor {
            x: Some(px(0.0)),
            y: None,
            kind: None,
            in_x: None,
            in_y: None,
            out_x: None,
            out_y: None,
        }];
        let valid = [anchor(0.0, 0.0), anchor(10.0, 0.0)];
        let first_contours = [VectorPathContourInput {
            anchors: &invalid,
            closed: false,
        }];
        let second_contours = [VectorPathContourInput {
            anchors: &valid,
            closed: false,
        }];

        let report = compound_path_collision(CompoundPathCollisionInput {
            first_contours: &first_contours,
            second_contours: &second_contours,
            tolerance: 0.1,
            required_clearance: 2.0,
        });

        assert_eq!(report.nearest, None);
        assert_eq!(report.score, 0.0);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "path_collision.invalid_first_geometry")
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
