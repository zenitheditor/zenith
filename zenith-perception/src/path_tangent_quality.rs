use crate::{PerceptionDiagnostic, PerceptionSeverity};
use zenith_core::{Dimension, PathAnchor, Unit};
use zenith_geometry::{PathAnchor as GeometryPathAnchor, Point2};

const SMOOTH_ALIGNMENT_THRESHOLD: f64 = 0.95;
const SHARP_ALIGNMENT_THRESHOLD: f64 = 0.35;
const LOW_ALIGNMENT_THRESHOLD: f64 = 0.75;
const LOW_BALANCE_THRESHOLD: f64 = 0.5;
const TANGENT_ALIGNMENT_WEIGHT: f64 = 0.6;
const HANDLE_BALANCE_WEIGHT: f64 = 0.4;
const ZERO_LENGTH_EPSILON: f64 = 0.0;

/// Input for local cubic-handle tangent craftsmanship analysis.
///
/// The metric intentionally evaluates only complete `px` anchor/handle
/// coordinates. Missing, non-pixel, or non-finite coordinates are skipped so
/// perception remains advisory and validation stays in `zenith-core`.
#[derive(Debug, Clone, Copy)]
pub struct PathTangentQualityInput<'a> {
    pub anchors: &'a [PathAnchor],
    pub closed: bool,
}

/// Deterministic local tangent quality summary for a single path anchor list.
#[derive(Debug, Clone, PartialEq)]
pub struct PathTangentQualityReport {
    pub anchor_count: usize,
    pub closed: bool,
    pub handle_count: usize,
    pub evaluated_join_count: usize,
    pub smooth_join_count: usize,
    pub sharp_turn_count: usize,
    pub degenerate_handle_count: usize,
    pub handle_balance_mean: f32,
    pub tangent_alignment_mean: f32,
    pub craftsmanship_score: f32,
    pub diagnostics: Vec<PerceptionDiagnostic>,
}

/// Analyze local path handle alignment and balance.
///
/// A join is evaluated when an anchor has complete `px` anchor coordinates plus
/// complete `px` in/out handle pairs. The score is not aesthetic judgment; it is
/// a deterministic signal for whether handles that appear to claim smooth cubic
/// continuity are aligned and proportioned like smooth handles.
pub fn path_tangent_quality(input: PathTangentQualityInput<'_>) -> PathTangentQualityReport {
    let anchor_count = input.anchors.len();
    let handle_count = input.anchors.iter().map(complete_handle_count).sum();

    let mut evaluated_join_count = 0;
    let mut smooth_join_count = 0;
    let mut sharp_turn_count = 0;
    let mut degenerate_handle_count = 0;
    let mut handle_balance_total = 0.0;
    let mut tangent_alignment_total = 0.0;

    for anchor in evaluable_anchors(input.anchors, input.closed) {
        let Some(join) =
            evaluable_geometry_anchor(anchor).and_then(GeometryPathAnchor::join_vectors)
        else {
            continue;
        };

        evaluated_join_count += 1;

        if join.in_length <= ZERO_LENGTH_EPSILON || join.out_length <= ZERO_LENGTH_EPSILON {
            degenerate_handle_count += 1;
            continue;
        }

        let tangent_alignment = join.opposing_tangent_alignment();
        let handle_balance = join.handle_length_balance();

        if tangent_alignment >= SMOOTH_ALIGNMENT_THRESHOLD {
            smooth_join_count += 1;
        }

        if tangent_alignment <= SHARP_ALIGNMENT_THRESHOLD {
            sharp_turn_count += 1;
        }

        tangent_alignment_total += tangent_alignment;
        handle_balance_total += handle_balance;
    }

    let tangent_alignment_mean = mean(tangent_alignment_total, evaluated_join_count);
    let handle_balance_mean = mean(handle_balance_total, evaluated_join_count);
    let craftsmanship_score = craftsmanship_score(
        evaluated_join_count,
        tangent_alignment_mean,
        handle_balance_mean,
    );
    let diagnostics = diagnostics(
        anchor_count,
        evaluated_join_count,
        degenerate_handle_count,
        tangent_alignment_mean,
        handle_balance_mean,
    );

    PathTangentQualityReport {
        anchor_count,
        closed: input.closed,
        handle_count,
        evaluated_join_count,
        smooth_join_count,
        sharp_turn_count,
        degenerate_handle_count,
        handle_balance_mean,
        tangent_alignment_mean,
        craftsmanship_score,
        diagnostics,
    }
}

fn complete_handle_count(anchor: &PathAnchor) -> usize {
    usize::from(anchor.in_x.is_some() && anchor.in_y.is_some())
        + usize::from(anchor.out_x.is_some() && anchor.out_y.is_some())
}

fn evaluable_geometry_anchor(anchor: &PathAnchor) -> Option<GeometryPathAnchor> {
    let anchor_point = point_from_px_pair(anchor.x.as_ref(), anchor.y.as_ref())?;
    let in_handle = point_from_px_pair(anchor.in_x.as_ref(), anchor.in_y.as_ref())?;
    let out_handle = point_from_px_pair(anchor.out_x.as_ref(), anchor.out_y.as_ref())?;

    GeometryPathAnchor::new(anchor_point, Some(in_handle), Some(out_handle)).ok()
}

fn evaluable_anchors(anchors: &[PathAnchor], closed: bool) -> &[PathAnchor] {
    if closed {
        anchors
    } else if anchors.len() > 2 {
        &anchors[1..anchors.len() - 1]
    } else {
        &[]
    }
}

fn px_value(dimension: Option<&Dimension>) -> Option<f64> {
    let dimension = dimension?;
    match dimension.unit {
        Unit::Px if dimension.value.is_finite() => Some(dimension.value),
        Unit::Px => None,
        Unit::Pt | Unit::Pct | Unit::Deg | Unit::Unknown(_) => None,
    }
}

fn point_from_px_pair(x: Option<&Dimension>, y: Option<&Dimension>) -> Option<Point2> {
    Point2::new(px_value(x)?, px_value(y)?).ok()
}

fn mean(total: f64, count: usize) -> f32 {
    if count == 0 {
        0.0
    } else {
        ((total / count as f64).clamp(0.0, 1.0)) as f32
    }
}

fn craftsmanship_score(
    evaluated_join_count: usize,
    tangent_alignment_mean: f32,
    handle_balance_mean: f32,
) -> f32 {
    if evaluated_join_count == 0 {
        0.0
    } else {
        (TANGENT_ALIGNMENT_WEIGHT * f64::from(tangent_alignment_mean)
            + HANDLE_BALANCE_WEIGHT * f64::from(handle_balance_mean))
        .clamp(0.0, 1.0) as f32
    }
}

fn diagnostics(
    anchor_count: usize,
    evaluated_join_count: usize,
    degenerate_handle_count: usize,
    tangent_alignment_mean: f32,
    handle_balance_mean: f32,
) -> Vec<PerceptionDiagnostic> {
    let mut diagnostics = Vec::new();

    if degenerate_handle_count > 0 {
        diagnostics.push(PerceptionDiagnostic::new(
            "path_tangent_quality.degenerate_handle",
            PerceptionSeverity::Info,
            "one or more path joins have zero-length handles",
        ));
    }

    if evaluated_join_count > 0 && f64::from(tangent_alignment_mean) < LOW_ALIGNMENT_THRESHOLD {
        diagnostics.push(PerceptionDiagnostic::new(
            "path_tangent_quality.low_tangent_alignment",
            PerceptionSeverity::Info,
            "mean tangent alignment is low across evaluated path joins",
        ));
    }

    if evaluated_join_count > 0 && f64::from(handle_balance_mean) < LOW_BALANCE_THRESHOLD {
        diagnostics.push(PerceptionDiagnostic::new(
            "path_tangent_quality.unbalanced_handle_lengths",
            PerceptionSeverity::Info,
            "mean handle length balance is low across evaluated path joins",
        ));
    }

    if anchor_count > 0 && evaluated_join_count == 0 {
        diagnostics.push(PerceptionDiagnostic::new(
            "path_tangent_quality.no_evaluable_joins",
            PerceptionSeverity::Info,
            "path anchors do not contain complete px anchor and handle coordinates",
        ));
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_returns_zeroes_without_diagnostics() {
        let report = path_tangent_quality(PathTangentQualityInput {
            anchors: &[],
            closed: false,
        });

        assert_eq!(report.anchor_count, 0);
        assert!(!report.closed);
        assert_eq!(report.handle_count, 0);
        assert_eq!(report.evaluated_join_count, 0);
        assert_eq!(report.smooth_join_count, 0);
        assert_eq!(report.sharp_turn_count, 0);
        assert_eq!(report.degenerate_handle_count, 0);
        assert_near(report.handle_balance_mean, 0.0);
        assert_near(report.tangent_alignment_mean, 0.0);
        assert_near(report.craftsmanship_score, 0.0);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn opposite_collinear_handles_score_perfectly() {
        let anchors = [anchor(0.0, 0.0, -10.0, 0.0, 10.0, 0.0)];

        let report = path_tangent_quality(PathTangentQualityInput {
            anchors: &anchors,
            closed: true,
        });

        assert_eq!(report.anchor_count, 1);
        assert!(report.closed);
        assert_eq!(report.handle_count, 2);
        assert_eq!(report.evaluated_join_count, 1);
        assert_eq!(report.smooth_join_count, 1);
        assert_eq!(report.sharp_turn_count, 0);
        assert_eq!(report.degenerate_handle_count, 0);
        assert_near(report.handle_balance_mean, 1.0);
        assert_near(report.tangent_alignment_mean, 1.0);
        assert_near(report.craftsmanship_score, 1.0);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn same_direction_handles_emit_low_alignment_diagnostic() {
        let anchors = [anchor(0.0, 0.0, 10.0, 0.0, 20.0, 0.0)];

        let report = path_tangent_quality(PathTangentQualityInput {
            anchors: &anchors,
            closed: true,
        });

        assert_eq!(report.evaluated_join_count, 1);
        assert_eq!(report.smooth_join_count, 0);
        assert_eq!(report.sharp_turn_count, 1);
        assert_near(report.tangent_alignment_mean, 0.0);
        assert_eq!(
            report.diagnostics,
            vec![PerceptionDiagnostic::new(
                "path_tangent_quality.low_tangent_alignment",
                PerceptionSeverity::Info,
                "mean tangent alignment is low across evaluated path joins",
            )]
        );
    }

    #[test]
    fn unbalanced_opposite_handles_lower_balance_and_score() {
        let anchors = [anchor(0.0, 0.0, -1.0, 0.0, 10.0, 0.0)];

        let report = path_tangent_quality(PathTangentQualityInput {
            anchors: &anchors,
            closed: true,
        });

        assert_eq!(report.evaluated_join_count, 1);
        assert_eq!(report.smooth_join_count, 1);
        assert_eq!(report.sharp_turn_count, 0);
        assert_near(report.tangent_alignment_mean, 1.0);
        assert_near(report.handle_balance_mean, 0.1);
        assert_near(report.craftsmanship_score, 0.64);
        assert_eq!(
            report.diagnostics,
            vec![PerceptionDiagnostic::new(
                "path_tangent_quality.unbalanced_handle_lengths",
                PerceptionSeverity::Info,
                "mean handle length balance is low across evaluated path joins",
            )]
        );
    }

    #[test]
    fn degenerate_handle_counts_and_emits_diagnostic() {
        let anchors = [anchor(0.0, 0.0, 0.0, 0.0, 10.0, 0.0)];

        let report = path_tangent_quality(PathTangentQualityInput {
            anchors: &anchors,
            closed: true,
        });

        assert_eq!(report.evaluated_join_count, 1);
        assert_eq!(report.degenerate_handle_count, 1);
        assert_eq!(report.smooth_join_count, 0);
        assert_eq!(report.sharp_turn_count, 0);
        assert_near(report.tangent_alignment_mean, 0.0);
        assert_near(report.handle_balance_mean, 0.0);
        assert_near(report.craftsmanship_score, 0.0);
        assert_eq!(
            report.diagnostics,
            vec![
                PerceptionDiagnostic::new(
                    "path_tangent_quality.degenerate_handle",
                    PerceptionSeverity::Info,
                    "one or more path joins have zero-length handles",
                ),
                PerceptionDiagnostic::new(
                    "path_tangent_quality.low_tangent_alignment",
                    PerceptionSeverity::Info,
                    "mean tangent alignment is low across evaluated path joins",
                ),
                PerceptionDiagnostic::new(
                    "path_tangent_quality.unbalanced_handle_lengths",
                    PerceptionSeverity::Info,
                    "mean handle length balance is low across evaluated path joins",
                ),
            ]
        );
    }

    #[test]
    fn missing_or_non_px_handles_are_skipped() {
        let anchors = [
            PathAnchor {
                x: Some(px(0.0)),
                y: Some(px(0.0)),
                in_x: Some(px(-1.0)),
                in_y: None,
                out_x: Some(px(1.0)),
                out_y: Some(px(0.0)),
            },
            PathAnchor {
                x: Some(px(0.0)),
                y: Some(px(0.0)),
                in_x: Some(px(-1.0)),
                in_y: Some(px(0.0)),
                out_x: Some(pt(1.0)),
                out_y: Some(px(0.0)),
            },
        ];

        let report = path_tangent_quality(PathTangentQualityInput {
            anchors: &anchors,
            closed: false,
        });

        assert_eq!(report.anchor_count, 2);
        assert_eq!(report.handle_count, 3);
        assert_eq!(report.evaluated_join_count, 0);
        assert_eq!(report.degenerate_handle_count, 0);
        assert_near(report.craftsmanship_score, 0.0);
        assert_eq!(
            report.diagnostics,
            vec![PerceptionDiagnostic::new(
                "path_tangent_quality.no_evaluable_joins",
                PerceptionSeverity::Info,
                "path anchors do not contain complete px anchor and handle coordinates",
            )]
        );
    }

    #[test]
    fn open_path_endpoints_are_not_evaluated_as_joins() {
        let anchors = [
            anchor(0.0, 0.0, 10.0, 0.0, 20.0, 0.0),
            anchor(50.0, 0.0, 40.0, 0.0, 60.0, 0.0),
            anchor(100.0, 0.0, 80.0, 0.0, 90.0, 0.0),
        ];

        let report = path_tangent_quality(PathTangentQualityInput {
            anchors: &anchors,
            closed: false,
        });

        assert_eq!(report.evaluated_join_count, 1);
        assert_eq!(report.smooth_join_count, 1);
        assert_eq!(report.sharp_turn_count, 0);
        assert_near(report.craftsmanship_score, 1.0);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn short_open_path_endpoints_are_not_evaluated_as_joins() {
        let anchors = [
            anchor(0.0, 0.0, 10.0, 0.0, 20.0, 0.0),
            anchor(100.0, 0.0, 80.0, 0.0, 90.0, 0.0),
        ];

        let report = path_tangent_quality(PathTangentQualityInput {
            anchors: &anchors,
            closed: false,
        });

        assert_eq!(report.anchor_count, 2);
        assert_eq!(report.evaluated_join_count, 0);
        assert_near(report.craftsmanship_score, 0.0);
        assert_eq!(
            report.diagnostics,
            vec![PerceptionDiagnostic::new(
                "path_tangent_quality.no_evaluable_joins",
                PerceptionSeverity::Info,
                "path anchors do not contain complete px anchor and handle coordinates",
            )]
        );
    }

    #[test]
    fn closed_path_evaluates_endpoint_anchors_as_joins() {
        let anchors = [
            anchor(0.0, 0.0, -10.0, 0.0, 10.0, 0.0),
            anchor(50.0, 0.0, 40.0, 0.0, 60.0, 0.0),
            anchor(100.0, 0.0, 90.0, 0.0, 110.0, 0.0),
        ];

        let report = path_tangent_quality(PathTangentQualityInput {
            anchors: &anchors,
            closed: true,
        });

        assert_eq!(report.evaluated_join_count, 3);
        assert_eq!(report.smooth_join_count, 3);
        assert_near(report.craftsmanship_score, 1.0);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn non_finite_coordinates_are_skipped() {
        let anchors = [PathAnchor {
            x: Some(px(f64::INFINITY)),
            y: Some(px(0.0)),
            in_x: Some(px(-1.0)),
            in_y: Some(px(0.0)),
            out_x: Some(px(1.0)),
            out_y: Some(px(0.0)),
        }];

        let report = path_tangent_quality(PathTangentQualityInput {
            anchors: &anchors,
            closed: true,
        });

        assert_eq!(report.anchor_count, 1);
        assert!(report.closed);
        assert_eq!(report.evaluated_join_count, 0);
        assert_near(report.craftsmanship_score, 0.0);
        assert_eq!(
            report.diagnostics,
            vec![PerceptionDiagnostic::new(
                "path_tangent_quality.no_evaluable_joins",
                PerceptionSeverity::Info,
                "path anchors do not contain complete px anchor and handle coordinates",
            )]
        );
    }

    #[test]
    fn huge_finite_coordinates_do_not_create_nan_scores() {
        let huge = 1.0e308;
        let anchors = [anchor(0.0, 0.0, -huge, 0.0, huge, 0.0)];

        let report = path_tangent_quality(PathTangentQualityInput {
            anchors: &anchors,
            closed: true,
        });

        assert_eq!(report.evaluated_join_count, 1);
        assert!(report.tangent_alignment_mean.is_finite());
        assert!(report.handle_balance_mean.is_finite());
        assert!(report.craftsmanship_score.is_finite());
        assert_near(report.tangent_alignment_mean, 1.0);
    }

    #[test]
    fn derived_infinite_vectors_are_skipped() {
        let anchors = [anchor(-f64::MAX, 0.0, f64::MAX, 0.0, 0.0, 0.0)];

        let report = path_tangent_quality(PathTangentQualityInput {
            anchors: &anchors,
            closed: true,
        });

        assert_eq!(report.evaluated_join_count, 0);
        assert!(report.tangent_alignment_mean.is_finite());
        assert!(report.handle_balance_mean.is_finite());
        assert!(report.craftsmanship_score.is_finite());
        assert_eq!(
            report.diagnostics,
            vec![PerceptionDiagnostic::new(
                "path_tangent_quality.no_evaluable_joins",
                PerceptionSeverity::Info,
                "path anchors do not contain complete px anchor and handle coordinates",
            )]
        );
    }

    fn anchor(x: f64, y: f64, in_x: f64, in_y: f64, out_x: f64, out_y: f64) -> PathAnchor {
        PathAnchor {
            x: Some(px(x)),
            y: Some(px(y)),
            in_x: Some(px(in_x)),
            in_y: Some(px(in_y)),
            out_x: Some(px(out_x)),
            out_y: Some(px(out_y)),
        }
    }

    fn px(value: f64) -> Dimension {
        Dimension {
            value,
            unit: Unit::Px,
        }
    }

    fn pt(value: f64) -> Dimension {
        Dimension {
            value,
            unit: Unit::Pt,
        }
    }

    fn assert_near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= f32::EPSILON,
            "{actual} is not near {expected}"
        );
    }
}
