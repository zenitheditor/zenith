use zenith_core::{Dimension, PathAnchor, Unit};
use zenith_geometry::{CompoundFillRule, CompoundFillTopology, FilledContourBoundaryRole};
use zenith_perception::{SmallLegibilityInput, VectorPathContourInput, small_legibility};

#[test]
fn scaled_closed_contour_reports_feature_facts() {
    let square = [
        anchor(0.0, 0.0),
        anchor(100.0, 0.0),
        anchor(100.0, 50.0),
        anchor(0.0, 50.0),
    ];
    let contours = [VectorPathContourInput {
        anchors: &square,
        closed: true,
    }];

    let report = small_legibility(SmallLegibilityInput {
        contours: &contours,
        fill_rule: None,
        target_size_px: 24.0,
        minimum_feature_px: 1.5,
        minimum_gap_px: 1.0,
        flatten_tolerance_px: 0.25,
        maximum_detail_density: 0.5,
    });

    assert_eq!(report.measured_contour_count, 1);
    assert_eq!(report.flattened_point_count, 5);
    assert_eq!(report.thumbnail_scale, Some(0.24));
    assert_eq!(report.minimum_scaled_contour_dimension, Some(12.0));
    assert_eq!(report.minimum_scaled_contour_gap, None);
    assert_eq!(report.score, 1.0);
    assert!(report.diagnostics.is_empty());
}

#[test]
fn tight_compound_gap_lowers_score_and_emits_diagnostic() {
    let first = [
        anchor(0.0, 0.0),
        anchor(40.0, 0.0),
        anchor(40.0, 40.0),
        anchor(0.0, 40.0),
    ];
    let second = [
        anchor(42.0, 0.0),
        anchor(82.0, 0.0),
        anchor(82.0, 40.0),
        anchor(42.0, 40.0),
    ];
    let contours = [
        VectorPathContourInput {
            anchors: &first,
            closed: true,
        },
        VectorPathContourInput {
            anchors: &second,
            closed: true,
        },
    ];

    let report = small_legibility(SmallLegibilityInput {
        contours: &contours,
        fill_rule: None,
        target_size_px: 24.0,
        minimum_feature_px: 1.5,
        minimum_gap_px: 1.0,
        flatten_tolerance_px: 0.25,
        maximum_detail_density: 0.5,
    });

    assert_eq!(report.measured_contour_count, 2);
    assert_eq!(report.minimum_scaled_contour_gap, Some(24.0 / 41.0));
    assert!(report.score < 1.0);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "small_legibility.tight_gap"),
        "expected tight gap diagnostic; got {:?}",
        report.diagnostics
    );
}

#[test]
fn invalid_contour_geometry_scores_zero() {
    let invalid = [PathAnchor {
        x: Some(px(0.0)),
        y: Some(px(0.0)),
        kind: None,
        in_x: Some(px(1.0)),
        in_y: None,
        out_x: None,
        out_y: None,
    }];
    let contours = [VectorPathContourInput {
        anchors: &invalid,
        closed: false,
    }];

    let report = small_legibility(SmallLegibilityInput {
        contours: &contours,
        fill_rule: None,
        target_size_px: 24.0,
        minimum_feature_px: 1.5,
        minimum_gap_px: 1.0,
        flatten_tolerance_px: 0.25,
        maximum_detail_density: 0.5,
    });

    assert_eq!(report.score, 0.0);
    assert_eq!(report.measured_contour_count, 0);
    assert_eq!(report.original_bounds, None);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "small_legibility.invalid_geometry"),
        "expected invalid geometry diagnostic; got {:?}",
        report.diagnostics
    );
}

#[test]
fn high_detail_density_lowers_score_and_emits_diagnostic() {
    let detailed = [
        anchor(0.0, 0.0),
        anchor(10.0, 0.0),
        anchor(10.0, 10.0),
        anchor(0.0, 10.0),
        anchor(1.0, 1.0),
        anchor(9.0, 1.0),
        anchor(9.0, 9.0),
        anchor(1.0, 9.0),
        anchor(2.0, 2.0),
        anchor(8.0, 2.0),
        anchor(8.0, 8.0),
        anchor(2.0, 8.0),
    ];
    let contours = [VectorPathContourInput {
        anchors: &detailed,
        closed: true,
    }];

    let report = small_legibility(SmallLegibilityInput {
        contours: &contours,
        fill_rule: None,
        target_size_px: 24.0,
        minimum_feature_px: 1.5,
        minimum_gap_px: 1.0,
        flatten_tolerance_px: 0.25,
        maximum_detail_density: 0.1,
    });

    assert!(report.detail_density.is_some_and(|density| density > 0.1));
    assert!(report.score < 1.0);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "small_legibility.high_detail_density"),
        "expected high detail density diagnostic; got {:?}",
        report.diagnostics
    );
}

#[test]
fn even_odd_nested_fill_topology_counts_paint_hole_paint() {
    let outer = square(0.0, 0.0, 30.0);
    let middle = reversed_square(5.0, 5.0, 20.0);
    let inner = square(10.0, 10.0, 10.0);
    let contours = closed_contours([&outer, &middle, &inner]);

    let report = small_legibility(default_input(&contours, Some(CompoundFillRule::EvenOdd)));
    let fill_topology = report.fill_topology.expect("fill topology");

    assert_eq!(fill_topology.rule, CompoundFillRule::EvenOdd);
    assert_eq!(fill_topology.contour_count, 3);
    assert_eq!(fill_topology.paint_contour_count, 2);
    assert_eq!(fill_topology.hole_contour_count, 1);
    assert_eq!(fill_topology.no_fill_change_contour_count, 0);
    assert_eq!(
        roles(&fill_topology.topology),
        vec![
            FilledContourBoundaryRole::Paint,
            FilledContourBoundaryRole::Hole,
            FilledContourBoundaryRole::Paint,
        ]
    );
}

#[test]
fn nonzero_opposite_winding_child_counts_hole() {
    let outer = square(0.0, 0.0, 20.0);
    let inner = reversed_square(5.0, 5.0, 10.0);
    let contours = closed_contours([&outer, &inner]);

    let report = small_legibility(default_input(&contours, Some(CompoundFillRule::NonZero)));
    let fill_topology = report.fill_topology.expect("fill topology");

    assert_eq!(fill_topology.rule, CompoundFillRule::NonZero);
    assert_eq!(fill_topology.contour_count, 2);
    assert_eq!(fill_topology.paint_contour_count, 1);
    assert_eq!(fill_topology.hole_contour_count, 1);
    assert_eq!(fill_topology.no_fill_change_contour_count, 0);
    assert_eq!(
        roles(&fill_topology.topology),
        vec![
            FilledContourBoundaryRole::Paint,
            FilledContourBoundaryRole::Hole,
        ]
    );
}

#[test]
fn nonzero_same_winding_child_counts_no_fill_change() {
    let outer = square(0.0, 0.0, 20.0);
    let inner = square(5.0, 5.0, 10.0);
    let contours = closed_contours([&outer, &inner]);

    let report = small_legibility(default_input(&contours, Some(CompoundFillRule::NonZero)));
    let fill_topology = report.fill_topology.expect("fill topology");

    assert_eq!(fill_topology.rule, CompoundFillRule::NonZero);
    assert_eq!(fill_topology.contour_count, 2);
    assert_eq!(fill_topology.paint_contour_count, 1);
    assert_eq!(fill_topology.hole_contour_count, 0);
    assert_eq!(fill_topology.no_fill_change_contour_count, 1);
    assert_eq!(
        roles(&fill_topology.topology),
        vec![
            FilledContourBoundaryRole::Paint,
            FilledContourBoundaryRole::NoFillChange,
        ]
    );
}

#[test]
fn absent_fill_rule_leaves_topology_absent() {
    let outer = square(0.0, 0.0, 20.0);
    let inner = reversed_square(5.0, 5.0, 10.0);
    let contours = closed_contours([&outer, &inner]);

    let report = small_legibility(default_input(&contours, None));

    assert_eq!(report.fill_topology, None);
    assert!(!report.diagnostics.iter().any(|diagnostic| diagnostic.code
        == "small_legibility.fill_topology_open_contour"
        || diagnostic.code == "small_legibility.fill_topology_unavailable"));
}

#[test]
fn fill_rule_with_open_contour_emits_info_without_zeroing_score() {
    let open = [anchor(0.0, 0.0), anchor(20.0, 0.0), anchor(20.0, 20.0)];
    let contours = [VectorPathContourInput {
        anchors: &open,
        closed: false,
    }];

    let report = small_legibility(default_input(&contours, Some(CompoundFillRule::EvenOdd)));

    assert_eq!(report.fill_topology, None);
    assert!(report.score > 0.0);
    assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.code
        == "small_legibility.fill_topology_open_contour"
        && diagnostic.severity == zenith_perception::PerceptionSeverity::Info));
}

#[test]
fn intersecting_closed_fill_topology_emits_info_without_topology() {
    let first = square(0.0, 0.0, 10.0);
    let second = square(5.0, 5.0, 10.0);
    let contours = closed_contours([&first, &second]);

    let report = small_legibility(default_input(&contours, Some(CompoundFillRule::EvenOdd)));

    assert_eq!(report.fill_topology, None);
    assert!(report.diagnostics.iter().any(|diagnostic| diagnostic.code
        == "small_legibility.fill_topology_unavailable"
        && diagnostic.severity == zenith_perception::PerceptionSeverity::Info));
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

fn square(x: f64, y: f64, size: f64) -> [PathAnchor; 4] {
    [
        anchor(x, y),
        anchor(x + size, y),
        anchor(x + size, y + size),
        anchor(x, y + size),
    ]
}

fn reversed_square(x: f64, y: f64, size: f64) -> [PathAnchor; 4] {
    [
        anchor(x, y),
        anchor(x, y + size),
        anchor(x + size, y + size),
        anchor(x + size, y),
    ]
}

fn closed_contours<'a, const N: usize>(
    anchors: [&'a [PathAnchor]; N],
) -> [VectorPathContourInput<'a>; N] {
    anchors.map(|anchors| VectorPathContourInput {
        anchors,
        closed: true,
    })
}

fn default_input<'a>(
    contours: &'a [VectorPathContourInput<'a>],
    fill_rule: Option<CompoundFillRule>,
) -> SmallLegibilityInput<'a> {
    SmallLegibilityInput {
        contours,
        fill_rule,
        target_size_px: 24.0,
        minimum_feature_px: 1.5,
        minimum_gap_px: 1.0,
        flatten_tolerance_px: 0.25,
        maximum_detail_density: 0.5,
    }
}

fn roles(topology: &CompoundFillTopology) -> Vec<FilledContourBoundaryRole> {
    topology
        .contours
        .iter()
        .map(|contour| contour.role)
        .collect()
}

fn px(value: f64) -> Dimension {
    Dimension {
        value,
        unit: Unit::Px,
    }
}
