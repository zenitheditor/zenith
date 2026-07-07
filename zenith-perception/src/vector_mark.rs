use crate::{
    PathCollisionInput, PathCollisionReport, PerceptionDiagnostic, VectorPathPerceptionInput,
    VectorPathPerceptionReport, analyze_vector_path, path_collision,
};
use zenith_core::PathAnchor;

#[derive(Debug, Clone, Copy)]
pub struct VectorMarkPathInput<'a> {
    pub id: &'a str,
    pub anchors: &'a [PathAnchor],
    pub closed: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct VectorMarkPerceptionInput<'a> {
    pub paths: &'a [VectorMarkPathInput<'a>],
    pub collision_tolerance: f64,
    pub required_clearance: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VectorMarkPerceptionReport {
    pub path_count: usize,
    pub collision_pair_count: usize,
    pub path_reports: Vec<VectorPathPerceptionReport>,
    pub collision_reports: Vec<VectorMarkCollisionReport>,
    pub total_intersection_count: usize,
    pub minimum_clearance: Option<f64>,
    pub collision_score_mean: Option<f32>,
    pub diagnostics: Vec<PerceptionDiagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VectorMarkCollisionReport {
    pub first_path_index: usize,
    pub first_path_id: String,
    pub second_path_index: usize,
    pub second_path_id: String,
    pub collision: PathCollisionReport,
}

pub fn analyze_vector_mark(input: VectorMarkPerceptionInput<'_>) -> VectorMarkPerceptionReport {
    let mut diagnostics = Vec::new();
    let path_reports = path_reports(input.paths, &mut diagnostics);
    let collision_reports = collision_reports(input, &mut diagnostics);
    let total_intersection_count = collision_reports
        .iter()
        .map(|report| report.collision.intersection_count)
        .sum();
    let minimum_clearance = minimum_clearance(&collision_reports);
    let collision_score_mean = collision_score_mean(&collision_reports);

    VectorMarkPerceptionReport {
        path_count: input.paths.len(),
        collision_pair_count: collision_reports.len(),
        path_reports,
        collision_reports,
        total_intersection_count,
        minimum_clearance,
        collision_score_mean,
        diagnostics,
    }
}

fn path_reports(
    paths: &[VectorMarkPathInput<'_>],
    diagnostics: &mut Vec<PerceptionDiagnostic>,
) -> Vec<VectorPathPerceptionReport> {
    let mut reports = Vec::with_capacity(paths.len());
    for path in paths {
        let report = analyze_vector_path(VectorPathPerceptionInput {
            anchors: path.anchors,
            closed: path.closed,
        });
        diagnostics.extend(report.diagnostics.iter().cloned());
        reports.push(report);
    }
    reports
}

fn collision_reports(
    input: VectorMarkPerceptionInput<'_>,
    diagnostics: &mut Vec<PerceptionDiagnostic>,
) -> Vec<VectorMarkCollisionReport> {
    let pair_count = input
        .paths
        .len()
        .saturating_mul(input.paths.len().saturating_sub(1))
        / 2;
    let mut reports = Vec::with_capacity(pair_count);

    for first_index in 0..input.paths.len() {
        for second_index in first_index + 1..input.paths.len() {
            let Some(first) = input.paths.get(first_index) else {
                continue;
            };
            let Some(second) = input.paths.get(second_index) else {
                continue;
            };
            let collision = path_collision(PathCollisionInput {
                first_anchors: first.anchors,
                first_closed: first.closed,
                second_anchors: second.anchors,
                second_closed: second.closed,
                tolerance: input.collision_tolerance,
                required_clearance: input.required_clearance,
            });
            diagnostics.extend(collision.diagnostics.iter().cloned());
            reports.push(VectorMarkCollisionReport {
                first_path_index: first_index,
                first_path_id: first.id.to_owned(),
                second_path_index: second_index,
                second_path_id: second.id.to_owned(),
                collision,
            });
        }
    }

    reports
}

fn minimum_clearance(reports: &[VectorMarkCollisionReport]) -> Option<f64> {
    reports
        .iter()
        .filter_map(|report| {
            report
                .collision
                .nearest
                .map(|_| report.collision.minimum_distance)
        })
        .min_by(|left, right| left.total_cmp(right))
}

fn collision_score_mean(reports: &[VectorMarkCollisionReport]) -> Option<f32> {
    if reports.is_empty() {
        return None;
    }

    let total: f32 = reports.iter().map(|report| report.collision.score).sum();
    Some(total / reports.len() as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{Dimension, Unit};

    #[test]
    fn mark_report_combines_path_reports_and_pairwise_collisions() {
        let horizontal = [anchor(0.0, 0.0), anchor(10.0, 0.0)];
        let vertical = [anchor(5.0, -5.0), anchor(5.0, 5.0)];
        let paths = [
            VectorMarkPathInput {
                id: "horizontal",
                anchors: &horizontal,
                closed: false,
            },
            VectorMarkPathInput {
                id: "vertical",
                anchors: &vertical,
                closed: false,
            },
        ];

        let report = analyze_vector_mark(VectorMarkPerceptionInput {
            paths: &paths,
            collision_tolerance: 0.1,
            required_clearance: 2.0,
        });

        assert_eq!(report.path_count, 2);
        assert_eq!(report.path_reports.len(), 2);
        assert_eq!(report.collision_pair_count, 1);
        assert_eq!(report.collision_reports.len(), 1);
        assert_eq!(report.total_intersection_count, 1);
        assert_eq!(report.minimum_clearance, Some(0.0));
        assert_eq!(report.collision_score_mean, Some(0.0));
        assert_eq!(report.collision_reports[0].first_path_id, "horizontal");
        assert_eq!(report.collision_reports[0].second_path_id, "vertical");
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "path_collision.intersection"),
            "expected collision diagnostic; got {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn single_path_mark_has_no_collision_pairs() {
        let line = [anchor(0.0, 0.0), anchor(10.0, 0.0)];
        let paths = [VectorMarkPathInput {
            id: "line",
            anchors: &line,
            closed: false,
        }];

        let report = analyze_vector_mark(VectorMarkPerceptionInput {
            paths: &paths,
            collision_tolerance: 0.1,
            required_clearance: 2.0,
        });

        assert_eq!(report.path_count, 1);
        assert_eq!(report.collision_pair_count, 0);
        assert!(report.collision_reports.is_empty());
        assert_eq!(report.minimum_clearance, None);
        assert_eq!(report.collision_score_mean, None);
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
