use crate::{
    PerceptionDiagnostic, PerceptionSeverity, VectorPathContourInput,
    path_geometry::compound_geometry,
};
use zenith_geometry::{
    CompoundFillRule, CompoundFillTopology, FilledContourBoundaryRole, FlattenedPathContour,
    Point2, RectBounds, classify_compound_path_fill_topology,
};

#[derive(Debug, Clone, Copy)]
pub struct SmallLegibilityInput<'a> {
    pub contours: &'a [VectorPathContourInput<'a>],
    pub fill_rule: Option<CompoundFillRule>,
    pub target_size_px: f64,
    pub minimum_feature_px: f64,
    pub minimum_gap_px: f64,
    pub flatten_tolerance_px: f64,
    pub maximum_detail_density: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SmallLegibilityReport {
    pub original_bounds: Option<RectBounds>,
    pub thumbnail_scale: Option<f64>,
    pub measured_contour_count: usize,
    pub flattened_point_count: usize,
    pub detail_density: Option<f64>,
    pub minimum_scaled_contour_dimension: Option<f64>,
    pub minimum_scaled_contour_gap: Option<f64>,
    pub fill_topology: Option<SmallLegibilityFillTopologyReport>,
    pub score: f32,
    pub diagnostics: Vec<PerceptionDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmallLegibilityFillTopologyReport {
    pub rule: CompoundFillRule,
    pub contour_count: usize,
    pub paint_contour_count: usize,
    pub hole_contour_count: usize,
    pub no_fill_change_contour_count: usize,
    pub topology: CompoundFillTopology,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SmallLegibilityDefaults {
    pub target_size_px: f64,
    pub minimum_feature_px: f64,
    pub minimum_gap_px: f64,
    pub flatten_tolerance_px: f64,
    pub maximum_detail_density: f64,
}

impl SmallLegibilityDefaults {
    pub(crate) const fn conservative() -> Self {
        Self {
            target_size_px: 24.0,
            minimum_feature_px: 1.5,
            minimum_gap_px: 1.0,
            flatten_tolerance_px: 0.25,
            maximum_detail_density: 0.5,
        }
    }
}

pub fn small_legibility(input: SmallLegibilityInput<'_>) -> SmallLegibilityReport {
    let mut diagnostics = Vec::new();
    let target_size_px = positive_parameter(
        input.target_size_px,
        "small_legibility.invalid_target_size",
        "small legibility target size must be a positive finite px value",
        &mut diagnostics,
    );
    let minimum_feature_px = positive_parameter(
        input.minimum_feature_px,
        "small_legibility.invalid_minimum_feature",
        "small legibility minimum feature must be a positive finite px value",
        &mut diagnostics,
    );
    let minimum_gap_px = positive_parameter(
        input.minimum_gap_px,
        "small_legibility.invalid_minimum_gap",
        "small legibility minimum gap must be a positive finite px value",
        &mut diagnostics,
    );
    let flatten_tolerance_px = positive_parameter(
        input.flatten_tolerance_px,
        "small_legibility.invalid_flatten_tolerance",
        "small legibility flatten tolerance must be a positive finite px value",
        &mut diagnostics,
    );
    let maximum_detail_density = positive_parameter(
        input.maximum_detail_density,
        "small_legibility.invalid_maximum_detail_density",
        "small legibility maximum detail density must be a positive finite value",
        &mut diagnostics,
    );

    let geometry = match compound_geometry(input.contours) {
        Ok(geometry) => geometry,
        Err(()) => {
            diagnostics.push(invalid_geometry_diagnostic());
            return empty_report(diagnostics);
        }
    };
    let original_bounds = match geometry.bounds() {
        Ok(Some(bounds)) => Some(bounds),
        Ok(None) => {
            diagnostics.push(empty_geometry_diagnostic());
            None
        }
        Err(_) => {
            diagnostics.push(invalid_geometry_diagnostic());
            None
        }
    };
    let thumbnail_scale =
        original_bounds.and_then(|bounds| thumbnail_scale(bounds, target_size_px));
    let flatten_tolerance_source_units =
        flatten_tolerance_source_units(flatten_tolerance_px, thumbnail_scale);
    let flattened = match flatten_tolerance_source_units {
        Some(flatten_tolerance) => match geometry.flatten_contours(flatten_tolerance) {
            Ok(flattened) => flattened,
            Err(_) => {
                diagnostics.push(invalid_flattening_diagnostic());
                Vec::new()
            }
        },
        None => Vec::new(),
    };
    let flattened_point_count = flattened.iter().map(|contour| contour.points.len()).sum();
    let measured_contour_count = flattened
        .iter()
        .filter(|contour| contour.points.len() >= 2)
        .count();
    let detail_density = detail_density(flattened_point_count, original_bounds, thumbnail_scale);
    let minimum_scaled_contour_dimension =
        minimum_scaled_contour_dimension(&flattened, thumbnail_scale);
    let minimum_scaled_contour_gap = minimum_scaled_contour_gap(&flattened, thumbnail_scale);
    let fill_topology = fill_topology_report(
        input.fill_rule,
        input.contours,
        &geometry,
        flatten_tolerance_source_units,
        flatten_tolerance_px,
        &mut diagnostics,
    );

    if measured_contour_count == 0
        && !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "small_legibility.empty_geometry")
    {
        diagnostics.push(empty_geometry_diagnostic());
    }
    if minimum_scaled_contour_dimension.is_some_and(|dimension| dimension < minimum_feature_px) {
        diagnostics.push(PerceptionDiagnostic::new(
            "small_legibility.thin_feature",
            PerceptionSeverity::Info,
            "scaled contour feature is below the configured small-size threshold",
        ));
    }
    if minimum_scaled_contour_gap.is_some_and(|gap| gap < minimum_gap_px) {
        diagnostics.push(PerceptionDiagnostic::new(
            "small_legibility.tight_gap",
            PerceptionSeverity::Info,
            "scaled contour gap is below the configured small-size threshold",
        ));
    }
    if detail_density.is_some_and(|density| density > maximum_detail_density) {
        diagnostics.push(PerceptionDiagnostic::new(
            "small_legibility.high_detail_density",
            PerceptionSeverity::Info,
            "scaled contour detail density is above the configured small-size threshold",
        ));
    }

    SmallLegibilityReport {
        original_bounds,
        thumbnail_scale,
        measured_contour_count,
        flattened_point_count,
        detail_density,
        minimum_scaled_contour_dimension,
        minimum_scaled_contour_gap,
        fill_topology,
        score: legibility_score(LegibilityScoreInput {
            minimum_scaled_contour_dimension,
            minimum_feature_px,
            minimum_scaled_contour_gap,
            minimum_gap_px,
            detail_density,
            maximum_detail_density,
            has_measurement: measured_contour_count > 0 && thumbnail_scale.is_some(),
            has_warning: diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == PerceptionSeverity::Warning),
        }),
        diagnostics,
    }
}

pub(crate) fn small_legibility_with_defaults(
    contours: &[VectorPathContourInput<'_>],
    fill_rule: Option<CompoundFillRule>,
    defaults: SmallLegibilityDefaults,
) -> SmallLegibilityReport {
    small_legibility(SmallLegibilityInput {
        contours,
        fill_rule,
        target_size_px: defaults.target_size_px,
        minimum_feature_px: defaults.minimum_feature_px,
        minimum_gap_px: defaults.minimum_gap_px,
        flatten_tolerance_px: defaults.flatten_tolerance_px,
        maximum_detail_density: defaults.maximum_detail_density,
    })
}

fn fill_topology_report(
    fill_rule: Option<CompoundFillRule>,
    contours: &[VectorPathContourInput<'_>],
    geometry: &zenith_geometry::CompoundPathGeometry,
    flatten_tolerance_source_units: Option<f64>,
    flatten_tolerance_px: f64,
    diagnostics: &mut Vec<PerceptionDiagnostic>,
) -> Option<SmallLegibilityFillTopologyReport> {
    let rule = fill_rule?;
    if contours.iter().any(|contour| !contour.closed) {
        diagnostics.push(PerceptionDiagnostic::new(
            "small_legibility.fill_topology_open_contour",
            PerceptionSeverity::Info,
            "compound fill topology requires closed contours",
        ));
        return None;
    }

    let flatten_tolerance = flatten_tolerance_source_units.unwrap_or(flatten_tolerance_px);
    let topology = match classify_compound_path_fill_topology(geometry, rule, flatten_tolerance) {
        Ok(topology) => topology,
        Err(_) => {
            diagnostics.push(PerceptionDiagnostic::new(
                "small_legibility.fill_topology_unavailable",
                PerceptionSeverity::Info,
                "compound fill topology is unavailable for this contour arrangement",
            ));
            return None;
        }
    };

    let mut paint_contour_count = 0usize;
    let mut hole_contour_count = 0usize;
    let mut no_fill_change_contour_count = 0usize;
    for contour in &topology.contours {
        match contour.role {
            FilledContourBoundaryRole::Paint => {
                paint_contour_count = paint_contour_count.saturating_add(1);
            }
            FilledContourBoundaryRole::Hole => {
                hole_contour_count = hole_contour_count.saturating_add(1);
            }
            FilledContourBoundaryRole::NoFillChange => {
                no_fill_change_contour_count = no_fill_change_contour_count.saturating_add(1);
            }
        }
    }

    Some(SmallLegibilityFillTopologyReport {
        rule,
        contour_count: topology.contours.len(),
        paint_contour_count,
        hole_contour_count,
        no_fill_change_contour_count,
        topology,
    })
}

fn positive_parameter(
    value: f64,
    code: &'static str,
    message: &'static str,
    diagnostics: &mut Vec<PerceptionDiagnostic>,
) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        diagnostics.push(PerceptionDiagnostic::new(
            code,
            PerceptionSeverity::Warning,
            message,
        ));
        1.0
    }
}

fn thumbnail_scale(bounds: RectBounds, target_size_px: f64) -> Option<f64> {
    let long_edge = bounds.width().max(bounds.height());
    (long_edge.is_finite() && long_edge > 0.0).then_some(target_size_px / long_edge)
}

fn flatten_tolerance_source_units(
    flatten_tolerance_px: f64,
    thumbnail_scale: Option<f64>,
) -> Option<f64> {
    let scale = thumbnail_scale?;
    (scale.is_finite() && scale > 0.0).then_some(flatten_tolerance_px / scale)
}

fn detail_density(
    flattened_point_count: usize,
    original_bounds: Option<RectBounds>,
    thumbnail_scale: Option<f64>,
) -> Option<f64> {
    let bounds = original_bounds?;
    let scale = thumbnail_scale?;
    let perimeter = (bounds.width() + bounds.height()) * 2.0 * scale;
    (perimeter.is_finite() && perimeter > 0.0).then_some(flattened_point_count as f64 / perimeter)
}

fn minimum_scaled_contour_dimension(
    contours: &[FlattenedPathContour],
    thumbnail_scale: Option<f64>,
) -> Option<f64> {
    let scale = thumbnail_scale?;
    contours
        .iter()
        .filter_map(|contour| bounds_for_points(&contour.points))
        .map(|bounds| bounds.width().min(bounds.height()) * scale)
        .filter(|dimension| dimension.is_finite())
        .min_by(|left, right| left.total_cmp(right))
}

fn minimum_scaled_contour_gap(
    contours: &[FlattenedPathContour],
    thumbnail_scale: Option<f64>,
) -> Option<f64> {
    let scale = thumbnail_scale?;
    let mut minimum_gap_squared: Option<f64> = None;
    for (first_index, first) in contours.iter().enumerate() {
        for second in contours.iter().skip(first_index + 1) {
            let Some(distance_squared) = contour_distance_squared(first, second) else {
                continue;
            };
            minimum_gap_squared = Some(match minimum_gap_squared {
                Some(current) => current.min(distance_squared),
                None => distance_squared,
            });
        }
    }

    minimum_gap_squared.map(|gap_squared| gap_squared.sqrt() * scale)
}

fn contour_distance_squared(
    first: &FlattenedPathContour,
    second: &FlattenedPathContour,
) -> Option<f64> {
    let mut minimum: Option<f64> = None;
    for first_segment in segments(first) {
        for second_segment in segments(second) {
            let distance_squared = segment_distance_squared(first_segment, second_segment);
            minimum = Some(match minimum {
                Some(current) => current.min(distance_squared),
                None => distance_squared,
            });
        }
    }

    minimum
}

fn segments(contour: &FlattenedPathContour) -> Vec<(Point2, Point2)> {
    let mut segments: Vec<_> = contour
        .points
        .windows(2)
        .filter_map(|pair| match pair {
            [start, end] => Some((*start, *end)),
            [] | [_] | [_, _, ..] => None,
        })
        .collect();

    if contour.closed && contour.points.len() > 2 {
        if let (Some(first), Some(last)) = (contour.points.first(), contour.points.last()) {
            segments.push((*last, *first));
        }
    }

    segments
}

fn segment_distance_squared(first: (Point2, Point2), second: (Point2, Point2)) -> f64 {
    let (first_start, first_end) = first;
    let (second_start, second_end) = second;
    if segments_intersect(first_start, first_end, second_start, second_end) {
        return 0.0;
    }

    first_start
        .distance_squared_to_segment(second_start, second_end)
        .min(first_end.distance_squared_to_segment(second_start, second_end))
        .min(second_start.distance_squared_to_segment(first_start, first_end))
        .min(second_end.distance_squared_to_segment(first_start, first_end))
}

fn segments_intersect(
    first_start: Point2,
    first_end: Point2,
    second_start: Point2,
    second_end: Point2,
) -> bool {
    let first_side_start = orientation(first_start, first_end, second_start);
    let first_side_end = orientation(first_start, first_end, second_end);
    let second_side_start = orientation(second_start, second_end, first_start);
    let second_side_end = orientation(second_start, second_end, first_end);

    if first_side_start == 0.0 && point_on_segment(second_start, first_start, first_end) {
        return true;
    }
    if first_side_end == 0.0 && point_on_segment(second_end, first_start, first_end) {
        return true;
    }
    if second_side_start == 0.0 && point_on_segment(first_start, second_start, second_end) {
        return true;
    }
    if second_side_end == 0.0 && point_on_segment(first_end, second_start, second_end) {
        return true;
    }

    first_side_start.signum() != first_side_end.signum()
        && second_side_start.signum() != second_side_end.signum()
}

fn orientation(start: Point2, end: Point2, point: Point2) -> f64 {
    (end.x - start.x) * (point.y - start.y) - (end.y - start.y) * (point.x - start.x)
}

fn point_on_segment(point: Point2, start: Point2, end: Point2) -> bool {
    point.x >= start.x.min(end.x)
        && point.x <= start.x.max(end.x)
        && point.y >= start.y.min(end.y)
        && point.y <= start.y.max(end.y)
}

fn bounds_for_points(points: &[Point2]) -> Option<RectBounds> {
    let first = points.first()?;
    let mut bounds = RectBounds::from_point(*first);
    for point in points.iter().skip(1) {
        bounds = bounds.include_point(*point);
    }
    Some(bounds)
}

#[derive(Debug, Clone, Copy)]
struct LegibilityScoreInput {
    minimum_scaled_contour_dimension: Option<f64>,
    minimum_feature_px: f64,
    minimum_scaled_contour_gap: Option<f64>,
    minimum_gap_px: f64,
    detail_density: Option<f64>,
    maximum_detail_density: f64,
    has_measurement: bool,
    has_warning: bool,
}

fn legibility_score(input: LegibilityScoreInput) -> f32 {
    if !input.has_measurement || input.has_warning {
        return 0.0;
    }

    let feature_score = input
        .minimum_scaled_contour_dimension
        .map(|dimension| dimension / input.minimum_feature_px)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let gap_score = input
        .minimum_scaled_contour_gap
        .map(|gap| gap / input.minimum_gap_px)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    let detail_score = input
        .detail_density
        .map(|density| input.maximum_detail_density / density)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);

    feature_score.min(gap_score).min(detail_score) as f32
}

fn empty_report(diagnostics: Vec<PerceptionDiagnostic>) -> SmallLegibilityReport {
    SmallLegibilityReport {
        original_bounds: None,
        thumbnail_scale: None,
        measured_contour_count: 0,
        flattened_point_count: 0,
        detail_density: None,
        minimum_scaled_contour_dimension: None,
        minimum_scaled_contour_gap: None,
        fill_topology: None,
        score: 0.0,
        diagnostics,
    }
}

fn invalid_geometry_diagnostic() -> PerceptionDiagnostic {
    PerceptionDiagnostic::new(
        "small_legibility.invalid_geometry",
        PerceptionSeverity::Warning,
        "small legibility requires complete finite px contour geometry",
    )
}

fn invalid_flattening_diagnostic() -> PerceptionDiagnostic {
    PerceptionDiagnostic::new(
        "small_legibility.invalid_flattening",
        PerceptionSeverity::Warning,
        "small legibility requires a valid positive flatten tolerance",
    )
}

fn empty_geometry_diagnostic() -> PerceptionDiagnostic {
    PerceptionDiagnostic::new(
        "small_legibility.empty_geometry",
        PerceptionSeverity::Info,
        "small legibility requires at least one measurable contour",
    )
}
