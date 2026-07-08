use crate::{
    AnchorEconomyInput, AnchorEconomyReport, PathTangentQualityInput, PathTangentQualityReport,
    PerceptionDiagnostic, PerceptionSeverity, SmallLegibilityReport, anchor_economy,
    path_geometry::{complete_handle_count, compound_geometry, geometry_path},
    path_tangent_quality,
    small_legibility::{SmallLegibilityDefaults, small_legibility_with_defaults},
};
use zenith_core::{PathAnchor, PathNode};
use zenith_geometry::{PathGeometry, PathTopology, RectBounds};

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

#[derive(Debug, Clone, Copy)]
pub struct VectorPathContourInput<'a> {
    pub anchors: &'a [PathAnchor],
    pub closed: bool,
}

impl<'a> VectorPathContourInput<'a> {
    pub fn from_path_node(path: &'a PathNode) -> Vec<Self> {
        path.effective_subpaths()
            .map(|subpath| Self {
                anchors: subpath.anchors,
                closed: subpath.closed == Some(true),
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CompoundVectorPathPerceptionInput<'a> {
    pub contours: &'a [VectorPathContourInput<'a>],
}

/// Aggregated deterministic perception metrics for one editable path.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorPathPerceptionReport {
    pub anchor_count: usize,
    pub segment_count: usize,
    pub closed: bool,
    pub bounds: Option<RectBounds>,
    pub anchor_economy: AnchorEconomyReport,
    pub tangent_quality: PathTangentQualityReport,
    pub small_legibility: SmallLegibilityReport,
    pub diagnostics: Vec<PerceptionDiagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompoundVectorPathPerceptionReport {
    pub contour_count: usize,
    pub anchor_count: usize,
    pub segment_count: usize,
    pub open_subpath_count: usize,
    pub closed_subpath_count: usize,
    pub bounds: Option<RectBounds>,
    pub anchor_economy: AnchorEconomyReport,
    pub tangent_quality_reports: Vec<PathTangentQualityReport>,
    pub tangent_quality_score_mean: Option<f32>,
    pub small_legibility: SmallLegibilityReport,
    pub diagnostics: Vec<PerceptionDiagnostic>,
}

/// Analyze one editable vector path.
///
/// Low-level metrics remain public and composable. This aggregate derives
/// topology once, combines the current path metrics, and provides a stable
/// path-level entry point for later vector/logo perception modules.
pub fn analyze_vector_path(input: VectorPathPerceptionInput<'_>) -> VectorPathPerceptionReport {
    let anchor_count = input.anchors.len();
    let topology = PathGeometry::topology_for(anchor_count, input.closed);
    let anchor_economy = anchor_economy(anchor_economy_input(input, anchor_count, topology));
    let tangent_quality = path_tangent_quality(PathTangentQualityInput {
        anchors: input.anchors,
        closed: input.closed,
    });
    let (bounds, bounds_diagnostic) = path_bounds(input);
    let contours = [VectorPathContourInput {
        anchors: input.anchors,
        closed: input.closed,
    }];
    let small_legibility =
        small_legibility_with_defaults(&contours, SmallLegibilityDefaults::conservative());

    let mut diagnostics = anchor_economy.diagnostics.clone();
    diagnostics.extend(tangent_quality.diagnostics.iter().cloned());
    diagnostics.extend(small_legibility.diagnostics.iter().cloned());
    if let Some(diagnostic) = bounds_diagnostic {
        diagnostics.push(diagnostic);
    }

    VectorPathPerceptionReport {
        anchor_count,
        segment_count: topology.segment_count,
        closed: input.closed,
        bounds,
        anchor_economy,
        tangent_quality,
        small_legibility,
        diagnostics,
    }
}

pub fn analyze_compound_vector_path(
    input: CompoundVectorPathPerceptionInput<'_>,
) -> CompoundVectorPathPerceptionReport {
    let topology = compound_topology(input.contours);
    let anchor_economy = anchor_economy(AnchorEconomyInput {
        anchor_count: topology.anchor_count,
        segment_count: topology.segment_count,
        handle_count: compound_handle_count(input.contours),
        open_subpath_count: topology.open_subpath_count,
        closed_subpath_count: topology.closed_subpath_count,
    });
    let tangent_quality_reports = compound_tangent_quality(input.contours);
    let (bounds, bounds_diagnostic) = compound_path_bounds(input.contours);
    let small_legibility =
        small_legibility_with_defaults(input.contours, SmallLegibilityDefaults::conservative());

    let mut diagnostics = anchor_economy.diagnostics.clone();
    for report in &tangent_quality_reports {
        diagnostics.extend(report.diagnostics.iter().cloned());
    }
    diagnostics.extend(small_legibility.diagnostics.iter().cloned());
    if let Some(diagnostic) = bounds_diagnostic {
        diagnostics.push(diagnostic);
    }

    CompoundVectorPathPerceptionReport {
        contour_count: input.contours.len(),
        anchor_count: topology.anchor_count,
        segment_count: topology.segment_count,
        open_subpath_count: topology.open_subpath_count,
        closed_subpath_count: topology.closed_subpath_count,
        bounds,
        anchor_economy,
        tangent_quality_score_mean: tangent_quality_score_mean(&tangent_quality_reports),
        tangent_quality_reports,
        small_legibility,
        diagnostics,
    }
}

fn anchor_economy_input(
    input: VectorPathPerceptionInput<'_>,
    anchor_count: usize,
    topology: PathTopology,
) -> AnchorEconomyInput {
    AnchorEconomyInput {
        anchor_count,
        segment_count: topology.segment_count,
        handle_count: input.anchors.iter().map(complete_handle_count).sum(),
        open_subpath_count: topology.open_subpath_count,
        closed_subpath_count: topology.closed_subpath_count,
    }
}

fn path_bounds(
    input: VectorPathPerceptionInput<'_>,
) -> (Option<RectBounds>, Option<PerceptionDiagnostic>) {
    match geometry_path(input.anchors, input.closed) {
        Ok(geometry) => match geometry.bounds() {
            Ok(bounds) => (bounds, None),
            Err(_) => (None, Some(invalid_geometry_diagnostic())),
        },
        Err(()) => (None, Some(invalid_geometry_diagnostic())),
    }
}

fn compound_topology(contours: &[VectorPathContourInput<'_>]) -> PathTopology {
    contours.iter().fold(
        PathTopology {
            anchor_count: 0,
            segment_count: 0,
            open_subpath_count: 0,
            closed_subpath_count: 0,
        },
        |mut topology, contour| {
            let contour_topology =
                PathGeometry::topology_for(contour.anchors.len(), contour.closed);
            topology.anchor_count = topology
                .anchor_count
                .saturating_add(contour_topology.anchor_count);
            topology.segment_count = topology
                .segment_count
                .saturating_add(contour_topology.segment_count);
            topology.open_subpath_count = topology
                .open_subpath_count
                .saturating_add(contour_topology.open_subpath_count);
            topology.closed_subpath_count = topology
                .closed_subpath_count
                .saturating_add(contour_topology.closed_subpath_count);
            topology
        },
    )
}

fn compound_handle_count(contours: &[VectorPathContourInput<'_>]) -> usize {
    contours
        .iter()
        .flat_map(|contour| contour.anchors.iter())
        .map(complete_handle_count)
        .sum()
}

fn compound_tangent_quality(
    contours: &[VectorPathContourInput<'_>],
) -> Vec<PathTangentQualityReport> {
    contours
        .iter()
        .map(|contour| {
            path_tangent_quality(PathTangentQualityInput {
                anchors: contour.anchors,
                closed: contour.closed,
            })
        })
        .collect()
}

fn compound_path_bounds(
    contours: &[VectorPathContourInput<'_>],
) -> (Option<RectBounds>, Option<PerceptionDiagnostic>) {
    let geometry = match compound_geometry(contours) {
        Ok(geometry) => geometry,
        Err(()) => return (None, Some(invalid_geometry_diagnostic())),
    };

    match geometry.bounds() {
        Ok(bounds) => (bounds, None),
        Err(_) => (None, Some(invalid_geometry_diagnostic())),
    }
}

fn tangent_quality_score_mean(reports: &[PathTangentQualityReport]) -> Option<f32> {
    if reports.is_empty() {
        return None;
    }

    let total: f32 = reports
        .iter()
        .map(|report| report.craftsmanship_score)
        .sum();
    Some(total / reports.len() as f32)
}

fn invalid_geometry_diagnostic() -> PerceptionDiagnostic {
    PerceptionDiagnostic::new(
        "vector_path.invalid_geometry",
        PerceptionSeverity::Info,
        "path bounds require complete finite px anchor and handle coordinates",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PerceptionSeverity;
    use zenith_core::{Dimension, PathSubpath, Unit};

    #[test]
    fn open_path_derives_anchor_economy_counts() {
        let anchors = [
            anchor(0.0, 0.0, -10.0, 0.0, 10.0, 0.0),
            anchor(10.0, 0.0, 0.0, 0.0, 20.0, 0.0),
            PathAnchor {
                x: Some(px(20.0)),
                y: Some(px(0.0)),
                kind: None,
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
        assert_eq!(report.small_legibility.measured_contour_count, 1);
        assert!(report.small_legibility.thumbnail_scale.is_some());
        assert!(report.anchor_economy.diagnostics.is_empty());
    }

    #[test]
    fn vector_path_report_carries_exact_bounds() {
        let anchors = [
            PathAnchor {
                x: Some(px(0.0)),
                y: Some(px(0.0)),
                kind: None,
                in_x: None,
                in_y: None,
                out_x: Some(px(0.0)),
                out_y: Some(px(10.0)),
            },
            PathAnchor {
                x: Some(px(10.0)),
                y: Some(px(0.0)),
                kind: None,
                in_x: Some(px(10.0)),
                in_y: Some(px(10.0)),
                out_x: None,
                out_y: None,
            },
        ];

        let report = analyze_vector_path(VectorPathPerceptionInput {
            anchors: &anchors,
            closed: false,
        });

        assert_eq!(
            report.bounds,
            Some(RectBounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 10.0,
                max_y: 7.5,
            })
        );
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
                kind: None,
                in_x: None,
                in_y: None,
                out_x: None,
                out_y: None,
            },
            PathAnchor {
                x: Some(px(100.0)),
                y: Some(px(0.0)),
                kind: None,
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
        assert!(
            report.diagnostics.iter().any(|diagnostic| diagnostic
                == &PerceptionDiagnostic::new(
                    "path_tangent_quality.low_tangent_alignment",
                    PerceptionSeverity::Info,
                    "mean tangent alignment is low across evaluated path joins",
                )),
            "expected tangent quality diagnostic; got {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn vector_path_report_diagnoses_invalid_bounds_geometry() {
        let anchors = [PathAnchor {
            x: Some(px(0.0)),
            y: Some(px(0.0)),
            kind: None,
            in_x: None,
            in_y: None,
            out_x: Some(px(1.0)),
            out_y: None,
        }];

        let report = analyze_vector_path(VectorPathPerceptionInput {
            anchors: &anchors,
            closed: false,
        });

        assert_eq!(report.bounds, None);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "vector_path.invalid_geometry"),
            "expected invalid bounds geometry diagnostic; got {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn compound_vector_path_aggregates_topology_and_bounds() {
        let open = [line_anchor(0.0, 0.0), line_anchor(20.0, 0.0)];
        let closed = [
            line_anchor(40.0, 10.0),
            line_anchor(60.0, 10.0),
            line_anchor(60.0, 30.0),
        ];
        let contours = [
            VectorPathContourInput {
                anchors: &open,
                closed: false,
            },
            VectorPathContourInput {
                anchors: &closed,
                closed: true,
            },
        ];

        let report = analyze_compound_vector_path(CompoundVectorPathPerceptionInput {
            contours: &contours,
        });

        assert_eq!(report.contour_count, 2);
        assert_eq!(report.anchor_count, 5);
        assert_eq!(report.segment_count, 4);
        assert_eq!(report.open_subpath_count, 1);
        assert_eq!(report.closed_subpath_count, 1);
        assert_eq!(report.anchor_economy.handle_count, 0);
        assert_eq!(report.tangent_quality_reports.len(), 2);
        assert!(report.tangent_quality_score_mean.is_some());
        assert_eq!(report.small_legibility.measured_contour_count, 2);
        assert_eq!(
            report.bounds,
            Some(RectBounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 60.0,
                max_y: 30.0,
            })
        );
    }

    #[test]
    fn compound_vector_path_reports_invalid_bounds_geometry() {
        let valid = [anchor(0.0, 0.0, -10.0, 0.0, 10.0, 0.0)];
        let invalid = [PathAnchor {
            x: Some(px(20.0)),
            y: Some(px(20.0)),
            kind: None,
            in_x: Some(px(10.0)),
            in_y: None,
            out_x: None,
            out_y: None,
        }];
        let contours = [
            VectorPathContourInput {
                anchors: &valid,
                closed: false,
            },
            VectorPathContourInput {
                anchors: &invalid,
                closed: false,
            },
        ];

        let report = analyze_compound_vector_path(CompoundVectorPathPerceptionInput {
            contours: &contours,
        });

        assert_eq!(report.bounds, None);
        assert_eq!(report.anchor_count, 2);
        assert_eq!(report.open_subpath_count, 2);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "vector_path.invalid_geometry"),
            "expected invalid compound bounds diagnostic; got {:?}",
            report.diagnostics
        );
    }

    #[test]
    fn contour_input_adapts_legacy_path_node() {
        let anchors = [line_anchor(0.0, 0.0), line_anchor(10.0, 0.0)];
        let path = path_node("legacy", Some(true), anchors.to_vec(), Vec::new());

        let contours = VectorPathContourInput::from_path_node(&path);

        assert_eq!(contours.len(), 1);
        assert_eq!(contours[0].anchors, &anchors);
        assert!(contours[0].closed);
    }

    #[test]
    fn contour_input_adapts_compound_path_node() {
        let first = [line_anchor(0.0, 0.0), line_anchor(10.0, 0.0)];
        let second = [
            line_anchor(20.0, 0.0),
            line_anchor(30.0, 0.0),
            line_anchor(30.0, 10.0),
        ];
        let path = path_node(
            "compound",
            None,
            Vec::new(),
            vec![
                PathSubpath {
                    closed: None,
                    anchors: first.to_vec(),
                },
                PathSubpath {
                    closed: Some(true),
                    anchors: second.to_vec(),
                },
            ],
        );

        let contours = VectorPathContourInput::from_path_node(&path);

        assert_eq!(contours.len(), 2);
        assert_eq!(contours[0].anchors, &first);
        assert!(!contours[0].closed);
        assert_eq!(contours[1].anchors, &second);
        assert!(contours[1].closed);
    }

    fn anchor(x: f64, y: f64, in_x: f64, in_y: f64, out_x: f64, out_y: f64) -> PathAnchor {
        PathAnchor {
            x: Some(px(x)),
            y: Some(px(y)),
            kind: None,
            in_x: Some(px(in_x)),
            in_y: Some(px(in_y)),
            out_x: Some(px(out_x)),
            out_y: Some(px(out_y)),
        }
    }

    fn line_anchor(x: f64, y: f64) -> PathAnchor {
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

    fn path_node(
        id: &str,
        closed: Option<bool>,
        anchors: Vec<PathAnchor>,
        subpaths: Vec<PathSubpath>,
    ) -> PathNode {
        PathNode {
            id: id.to_owned(),
            name: None,
            role: None,
            closed,
            fill: None,
            stroke: None,
            stroke_width: None,
            stroke_alignment: None,
            stroke_linejoin: None,
            stroke_miter_limit: None,
            fill_rule: None,
            opacity: None,
            visible: None,
            locked: None,
            rotate: None,
            style: None,
            anchors,
            subpaths,
            source_span: None,
            unknown_props: Default::default(),
        }
    }

    fn px(value: f64) -> Dimension {
        Dimension {
            value,
            unit: Unit::Px,
        }
    }
}
