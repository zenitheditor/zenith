use crate::{
    AnchorEconomyInput, AnchorEconomyReport, PathTangentQualityInput, PathTangentQualityReport,
    PerceptionDiagnostic, anchor_economy, path_tangent_quality,
};
use zenith_core::PathAnchor;

/// Input for path-level vector perception.
///
/// This is separate from raster `analyze(surface)` because vector/logo metrics
/// inspect editable structure, not pixels. Future grid, balance, outline, and
/// legibility metrics can extend the path/document vector report surface
/// without changing the raster report contract.
#[derive(Debug, Clone, Copy)]
pub struct VectorPathPerceptionInput<'a> {
    pub anchors: &'a [PathAnchor],
    pub closed: bool,
}

/// Aggregated deterministic perception metrics for one editable path.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorPathPerceptionReport {
    pub anchor_count: usize,
    pub segment_count: usize,
    pub closed: bool,
    pub anchor_economy: AnchorEconomyReport,
    pub tangent_quality: PathTangentQualityReport,
    pub diagnostics: Vec<PerceptionDiagnostic>,
}

/// Analyze one editable vector path.
///
/// Low-level metrics remain public and composable. This aggregate derives
/// topology once, combines the current path metrics, and provides a stable
/// path-level entry point for later vector/logo perception modules.
pub fn analyze_vector_path(input: VectorPathPerceptionInput<'_>) -> VectorPathPerceptionReport {
    let anchor_count = input.anchors.len();
    let segment_count = segment_count(anchor_count, input.closed);
    let anchor_economy = anchor_economy(anchor_economy_input(input, anchor_count, segment_count));
    let tangent_quality = path_tangent_quality(PathTangentQualityInput {
        anchors: input.anchors,
        closed: input.closed,
    });

    let mut diagnostics = anchor_economy.diagnostics.clone();
    diagnostics.extend(tangent_quality.diagnostics.iter().cloned());

    VectorPathPerceptionReport {
        anchor_count,
        segment_count,
        closed: input.closed,
        anchor_economy,
        tangent_quality,
        diagnostics,
    }
}

fn anchor_economy_input(
    input: VectorPathPerceptionInput<'_>,
    anchor_count: usize,
    segment_count: usize,
) -> AnchorEconomyInput {
    AnchorEconomyInput {
        anchor_count,
        segment_count,
        handle_count: input.anchors.iter().map(complete_handle_count).sum(),
        open_subpath_count: usize::from(anchor_count > 0 && !input.closed),
        closed_subpath_count: closed_subpath_count(anchor_count, input.closed),
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

fn closed_subpath_count(anchor_count: usize, closed: bool) -> usize {
    usize::from(closed && anchor_count >= 3)
}

fn complete_handle_count(anchor: &PathAnchor) -> usize {
    usize::from(anchor.in_x.is_some() && anchor.in_y.is_some())
        + usize::from(anchor.out_x.is_some() && anchor.out_y.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PerceptionSeverity;
    use zenith_core::{Dimension, Unit};

    #[test]
    fn open_path_derives_anchor_economy_counts() {
        let anchors = [
            anchor(0.0, 0.0, -10.0, 0.0, 10.0, 0.0),
            anchor(10.0, 0.0, 0.0, 0.0, 20.0, 0.0),
            PathAnchor {
                x: Some(px(20.0)),
                y: Some(px(0.0)),
                in_x: Some(px(10.0)),
                in_y: Some(px(0.0)),
                out_x: None,
                out_y: None,
            },
        ];

        let report = analyze_vector_path(VectorPathPerceptionInput {
            anchors: &anchors,
            closed: false,
        });

        assert_eq!(report.anchor_economy.anchor_count, 3);
        assert_eq!(report.anchor_count, 3);
        assert_eq!(report.anchor_economy.segment_count, 2);
        assert_eq!(report.segment_count, 2);
        assert!(!report.closed);
        assert_eq!(report.anchor_economy.handle_count, 5);
        assert_eq!(report.anchor_economy.open_subpath_count, 1);
        assert_eq!(report.anchor_economy.closed_subpath_count, 0);
        assert_eq!(report.anchor_economy.minimum_anchor_count, 3);
        assert!(report.anchor_economy.diagnostics.is_empty());
    }

    #[test]
    fn closed_path_derives_anchor_economy_counts() {
        let anchors = [
            anchor(0.0, 0.0, -10.0, 0.0, 10.0, 0.0),
            anchor(10.0, 0.0, 0.0, 0.0, 20.0, 0.0),
            anchor(20.0, 0.0, 10.0, 0.0, 30.0, 0.0),
        ];

        let report = analyze_vector_path(VectorPathPerceptionInput {
            anchors: &anchors,
            closed: true,
        });

        assert_eq!(report.anchor_economy.anchor_count, 3);
        assert_eq!(report.anchor_count, 3);
        assert_eq!(report.anchor_economy.segment_count, 3);
        assert_eq!(report.segment_count, 3);
        assert!(report.closed);
        assert_eq!(report.anchor_economy.handle_count, 6);
        assert_eq!(report.anchor_economy.open_subpath_count, 0);
        assert_eq!(report.anchor_economy.closed_subpath_count, 1);
        assert_eq!(report.anchor_economy.minimum_anchor_count, 3);
        assert!(report.anchor_economy.diagnostics.is_empty());
    }

    #[test]
    fn short_closed_path_reports_invalid_topology() {
        let anchors = [
            anchor(0.0, 0.0, -10.0, 0.0, 10.0, 0.0),
            anchor(10.0, 0.0, 0.0, 0.0, 20.0, 0.0),
        ];

        let report = analyze_vector_path(VectorPathPerceptionInput {
            anchors: &anchors,
            closed: true,
        });

        assert_eq!(report.anchor_count, 2);
        assert_eq!(report.segment_count, 2);
        assert!(report.closed);
        assert_eq!(report.anchor_economy.closed_subpath_count, 0);
        assert_eq!(report.anchor_economy.economy_score, 0.0);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "anchor_economy.invalid_missing_topology"),
            "expected invalid topology diagnostic; got {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn vector_path_report_carries_tangent_quality_and_diagnostics() {
        let anchors = [
            anchor(0.0, 0.0, 10.0, 0.0, 20.0, 0.0),
            PathAnchor {
                x: Some(px(50.0)),
                y: Some(px(0.0)),
                in_x: None,
                in_y: None,
                out_x: None,
                out_y: None,
            },
            PathAnchor {
                x: Some(px(100.0)),
                y: Some(px(0.0)),
                in_x: None,
                in_y: None,
                out_x: None,
                out_y: None,
            },
        ];

        let report = analyze_vector_path(VectorPathPerceptionInput {
            anchors: &anchors,
            closed: true,
        });

        assert_eq!(report.tangent_quality.evaluated_join_count, 1);
        assert_eq!(report.tangent_quality.sharp_turn_count, 1);
        assert_eq!(report.tangent_quality.smooth_join_count, 0);
        assert_eq!(
            report.diagnostics,
            vec![PerceptionDiagnostic::new(
                "path_tangent_quality.low_tangent_alignment",
                PerceptionSeverity::Info,
                "mean tangent alignment is low across evaluated path joins",
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
}
