use zenith_core::{Dimension, PathAnchor, Unit};
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
