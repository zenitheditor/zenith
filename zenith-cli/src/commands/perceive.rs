//! Pure logic for `zenith perceive`.

use std::collections::BTreeSet;

use serde::Serialize;
use zenith_core::{KdlAdapter, KdlSource, Node, PathNode};
use zenith_perception::{
    CompoundFillRule, CompoundPathCollisionInput, CompoundPathCollisionReport,
    CompoundVectorPathPerceptionInput, CompoundVectorPathPerceptionReport, PerceptionDiagnostic,
    PerceptionSeverity, VectorPathContourInput, analyze_compound_vector_path,
    compound_path_collision,
};

const COLLISION_TOLERANCE: f64 = 0.1;
const REQUIRED_CLEARANCE: f64 = 2.0;

use crate::commands::serialize_pretty;

#[derive(Debug)]
pub struct PerceiveCmdErr {
    pub message: String,
    pub exit_code: u8,
}

#[derive(Debug)]
pub struct PerceiveOutcome {
    pub stdout: String,
    pub exit_code: u8,
}

#[derive(Debug, Serialize)]
struct VectorDocumentOutput {
    schema: &'static str,
    path_count: usize,
    warning_count: usize,
    info_count: usize,
    mark: VectorMarkOutput,
    paths: Vec<VectorPathOutput>,
}

#[derive(Debug, Serialize)]
struct VectorPathOutput {
    id: String,
    contour_count: usize,
    anchor_count: usize,
    segment_count: usize,
    open_subpath_count: usize,
    closed_subpath_count: usize,
    bounds: Option<BoundsOutput>,
    anchor_economy_score: f32,
    tangent_quality_score_mean: Option<f32>,
    small_legibility_score: f32,
    diagnostics: Vec<PerceptionDiagnosticOutput>,
}

#[derive(Debug, Serialize)]
struct VectorMarkOutput {
    path_count: usize,
    contour_count: usize,
    anchor_count: usize,
    segment_count: usize,
    collision_pair_count: usize,
    total_intersection_count: usize,
    minimum_clearance: Option<f64>,
    collision_score_mean: Option<f32>,
    aggregate_anchor_economy_score: f32,
    aggregate_tangent_quality_score_mean: Option<f32>,
    aggregate_small_legibility_score: f32,
    diagnostics: Vec<PerceptionDiagnosticOutput>,
    collision_reports: Vec<VectorCollisionOutput>,
}

#[derive(Debug, Serialize)]
struct VectorCollisionOutput {
    first_path_id: String,
    second_path_id: String,
    first_contour_count: usize,
    second_contour_count: usize,
    intersection_count: usize,
    minimum_distance: f64,
    required_clearance: f64,
    clearance_ratio: f32,
    score: f32,
    diagnostics: Vec<PerceptionDiagnosticOutput>,
}

#[derive(Debug, Serialize)]
struct BoundsOutput {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

#[derive(Debug, Serialize)]
struct PerceptionDiagnosticOutput {
    code: &'static str,
    severity: &'static str,
    message: &'static str,
}

pub fn vector(
    src: &str,
    json: bool,
    selected_nodes: &[String],
) -> Result<PerceiveOutcome, PerceiveCmdErr> {
    let doc = KdlAdapter
        .parse(src.as_bytes())
        .map_err(|e| PerceiveCmdErr {
            message: crate::report::parse_error_line(src, &e),
            exit_code: 2,
        })?;

    let mut all_paths = Vec::new();
    for page in &doc.body.pages {
        collect_paths(&page.children, &mut all_paths);
    }
    let paths = selected_paths(&all_paths, selected_nodes)?;

    let analyses = paths
        .iter()
        .map(|path| analyze_path(path))
        .collect::<Vec<_>>();
    let mark = analyze_mark(&analyses);
    let warning_count = analyses
        .iter()
        .flat_map(|analysis| analysis.output.diagnostics.iter())
        .chain(mark.diagnostics.iter())
        .filter(|diagnostic| diagnostic.severity == "warning")
        .count();
    let info_count = analyses
        .iter()
        .flat_map(|analysis| analysis.output.diagnostics.iter())
        .chain(mark.diagnostics.iter())
        .filter(|diagnostic| diagnostic.severity == "info")
        .count();
    let output = VectorDocumentOutput {
        schema: "zenith-perceive-vector-v1",
        path_count: analyses.len(),
        warning_count,
        info_count,
        mark,
        paths: analyses
            .into_iter()
            .map(|analysis| analysis.output)
            .collect(),
    };

    let stdout = if json {
        serialize_pretty(&output)
    } else {
        format_vector_human(&output)
    };

    Ok(PerceiveOutcome {
        stdout,
        exit_code: if warning_count == 0 { 0 } else { 1 },
    })
}

fn collect_paths<'a>(nodes: &'a [Node], paths: &mut Vec<&'a PathNode>) {
    for node in nodes {
        match node {
            Node::Path(path) => paths.push(path),
            Node::Frame(frame) => collect_paths(&frame.children, paths),
            Node::Group(group) => collect_paths(&group.children, paths),
            Node::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        collect_paths(&cell.children, paths);
                    }
                }
            }
            Node::Unknown(unknown) => collect_paths(&unknown.children, paths),
            Node::Rect(_)
            | Node::Ellipse(_)
            | Node::Line(_)
            | Node::Text(_)
            | Node::Code(_)
            | Node::Image(_)
            | Node::Polygon(_)
            | Node::Polyline(_)
            | Node::Instance(_)
            | Node::Field(_)
            | Node::Footnote(_)
            | Node::Toc(_)
            | Node::Shape(_)
            | Node::Connector(_)
            | Node::Pattern(_)
            | Node::Chart(_)
            | Node::Light(_)
            | Node::Mesh(_) => {}
        }
    }
}

fn selected_paths<'a>(
    paths: &[&'a PathNode],
    selected_nodes: &[String],
) -> Result<Vec<&'a PathNode>, PerceiveCmdErr> {
    if selected_nodes.is_empty() {
        return Ok(paths.to_vec());
    }

    let requested = selected_nodes
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut found = BTreeSet::new();
    let selected = paths
        .iter()
        .copied()
        .filter(|path| {
            if requested.contains(path.id.as_str()) {
                found.insert(path.id.as_str());
                true
            } else {
                false
            }
        })
        .collect::<Vec<_>>();

    if found.len() == requested.len() {
        return Ok(selected);
    }

    let missing = requested
        .difference(&found)
        .copied()
        .collect::<Vec<_>>()
        .join(", ");
    Err(PerceiveCmdErr {
        message: format!("error[perceive.node_not_found]: path node not found: {missing}"),
        exit_code: 2,
    })
}

struct PathAnalysis<'a> {
    id: &'a str,
    contours: Vec<VectorPathContourInput<'a>>,
    output: VectorPathOutput,
}

fn analyze_path(path: &PathNode) -> PathAnalysis<'_> {
    let contours = VectorPathContourInput::from_path_node(path);
    let report = analyze_compound_vector_path(CompoundVectorPathPerceptionInput {
        contours: &contours,
        fill_rule: fill_rule(path.fill_rule.as_deref()),
    });

    PathAnalysis {
        id: &path.id,
        output: vector_path_output(&path.id, report),
        contours,
    }
}

fn fill_rule(value: Option<&str>) -> Option<CompoundFillRule> {
    match value {
        Some("evenodd") => Some(CompoundFillRule::EvenOdd),
        Some("nonzero") | None => Some(CompoundFillRule::NonZero),
        Some(_) => None,
    }
}

fn vector_path_output(id: &str, report: CompoundVectorPathPerceptionReport) -> VectorPathOutput {
    VectorPathOutput {
        id: id.to_owned(),
        contour_count: report.contour_count,
        anchor_count: report.anchor_count,
        segment_count: report.segment_count,
        open_subpath_count: report.open_subpath_count,
        closed_subpath_count: report.closed_subpath_count,
        bounds: report.bounds.map(|bounds| BoundsOutput {
            min_x: bounds.min_x,
            min_y: bounds.min_y,
            max_x: bounds.max_x,
            max_y: bounds.max_y,
        }),
        anchor_economy_score: report.anchor_economy.economy_score,
        tangent_quality_score_mean: report.tangent_quality_score_mean,
        small_legibility_score: report.small_legibility.score,
        diagnostics: report.diagnostics.iter().map(diagnostic_output).collect(),
    }
}

fn analyze_mark(paths: &[PathAnalysis<'_>]) -> VectorMarkOutput {
    let mut all_contours = Vec::new();
    for path in paths {
        all_contours.extend(path.contours.iter().copied());
    }
    let aggregate = analyze_compound_vector_path(CompoundVectorPathPerceptionInput {
        contours: &all_contours,
        fill_rule: None,
    });

    let mut raw_collision_reports = Vec::new();
    let mut diagnostics = aggregate
        .diagnostics
        .iter()
        .map(diagnostic_output)
        .collect::<Vec<_>>();
    for (first_index, first) in paths.iter().enumerate() {
        for second in paths.iter().skip(first_index + 1) {
            let report = compound_path_collision(CompoundPathCollisionInput {
                first_contours: &first.contours,
                second_contours: &second.contours,
                tolerance: COLLISION_TOLERANCE,
                required_clearance: REQUIRED_CLEARANCE,
            });
            diagnostics.extend(report.diagnostics.iter().map(diagnostic_output));
            raw_collision_reports.push((first.id, second.id, report));
        }
    }

    let total_intersection_count = raw_collision_reports
        .iter()
        .map(|(_, _, report)| report.intersection_count)
        .sum();
    let minimum_clearance = raw_collision_reports
        .iter()
        .filter(|(_, _, report)| report.nearest.is_some())
        .map(|(_, _, report)| report.minimum_distance)
        .min_by(|left, right| left.total_cmp(right));
    let collision_score_mean = mean_score(
        raw_collision_reports
            .iter()
            .map(|(_, _, report)| report.score),
    );
    let collision_reports = raw_collision_reports
        .into_iter()
        .map(|(first_id, second_id, report)| collision_output(first_id, second_id, report))
        .collect::<Vec<_>>();

    VectorMarkOutput {
        path_count: paths.len(),
        contour_count: aggregate.contour_count,
        anchor_count: aggregate.anchor_count,
        segment_count: aggregate.segment_count,
        collision_pair_count: collision_reports.len(),
        total_intersection_count,
        minimum_clearance,
        collision_score_mean,
        aggregate_anchor_economy_score: aggregate.anchor_economy.economy_score,
        aggregate_tangent_quality_score_mean: aggregate.tangent_quality_score_mean,
        aggregate_small_legibility_score: aggregate.small_legibility.score,
        diagnostics,
        collision_reports,
    }
}

fn collision_output(
    first_path_id: &str,
    second_path_id: &str,
    report: CompoundPathCollisionReport,
) -> VectorCollisionOutput {
    VectorCollisionOutput {
        first_path_id: first_path_id.to_owned(),
        second_path_id: second_path_id.to_owned(),
        first_contour_count: report.first_contour_count,
        second_contour_count: report.second_contour_count,
        intersection_count: report.intersection_count,
        minimum_distance: report.minimum_distance,
        required_clearance: report.required_clearance,
        clearance_ratio: report.clearance_ratio,
        score: report.score,
        diagnostics: report.diagnostics.iter().map(diagnostic_output).collect(),
    }
}

fn mean_score(scores: impl Iterator<Item = f32>) -> Option<f32> {
    let mut count = 0;
    let mut total = 0.0;
    for score in scores {
        count += 1;
        total += score;
    }
    if count == 0 {
        None
    } else {
        Some(total / count as f32)
    }
}

fn diagnostic_output(diagnostic: &PerceptionDiagnostic) -> PerceptionDiagnosticOutput {
    PerceptionDiagnosticOutput {
        code: diagnostic.code,
        severity: severity_str(diagnostic.severity),
        message: diagnostic.message,
    }
}

fn severity_str(severity: PerceptionSeverity) -> &'static str {
    match severity {
        PerceptionSeverity::Info => "info",
        PerceptionSeverity::Warning => "warning",
    }
}

fn format_vector_human(output: &VectorDocumentOutput) -> String {
    if output.path_count == 0 {
        return "vector perception: no path nodes".to_owned();
    }

    let mut lines = Vec::new();
    lines.push(format!(
        "vector perception: {} path(s), {} warning(s), {} info",
        output.path_count, output.warning_count, output.info_count
    ));
    lines.push(format!(
        "mark: contours={} anchors={} segments={} pairs={} intersections={} clearance={} collision={}",
        output.mark.contour_count,
        output.mark.anchor_count,
        output.mark.segment_count,
        output.mark.collision_pair_count,
        output.mark.total_intersection_count,
        format_optional_f64(output.mark.minimum_clearance),
        format_optional_score(output.mark.collision_score_mean)
    ));
    for path in &output.paths {
        lines.push(format!(
            "{}: contours={} anchors={} segments={} economy={:.3} tangent={} small={:.3}",
            path.id,
            path.contour_count,
            path.anchor_count,
            path.segment_count,
            path.anchor_economy_score,
            format_optional_score(path.tangent_quality_score_mean),
            path.small_legibility_score
        ));
        for diagnostic in &path.diagnostics {
            lines.push(format!(
                "  {}[{}]: {}",
                diagnostic.severity, diagnostic.code, diagnostic.message
            ));
        }
    }
    lines.join("\n")
}

fn format_optional_f64(value: Option<f64>) -> String {
    match value {
        Some(value) => format!("{value:.3}"),
        None => "n/a".to_owned(),
    }
}

fn format_optional_score(score: Option<f32>) -> String {
    match score {
        Some(score) => format!("{score:.3}"),
        None => "n/a".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = r##"zenith version=1 {
  project id="proj" name="Perceive"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc" title="Perceive" {
    page id="pg" w=(px)200 h=(px)120 {
      path id="mark" closed=#true {
        anchor x=(px)0 y=(px)0
        anchor x=(px)40 y=(px)0
        anchor x=(px)40 y=(px)40
        anchor x=(px)0 y=(px)40
      }
    }
  }
}"##;

    #[test]
    fn vector_json_reports_path_metrics() {
        let outcome = vector(DOC, true, &[]).expect("perception should run");

        assert_eq!(outcome.exit_code, 0, "stdout: {}", outcome.stdout);
        assert!(
            outcome
                .stdout
                .contains("\"schema\": \"zenith-perceive-vector-v1\"")
        );
        assert!(outcome.stdout.contains("\"path_count\": 1"));
        assert!(outcome.stdout.contains("\"id\": \"mark\""));
        assert!(outcome.stdout.contains("\"anchor_count\": 4"));
        assert!(outcome.stdout.contains("\"collision_pair_count\": 0"));
    }

    #[test]
    fn vector_json_reports_mark_collisions() {
        let doc = r##"zenith version=1 {
  project id="proj" name="Perceive"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc" title="Perceive" {
    page id="pg" w=(px)200 h=(px)120 {
      path id="horizontal" {
        anchor x=(px)0 y=(px)0
        anchor x=(px)10 y=(px)0
      }
      path id="vertical" {
        anchor x=(px)5 y=(px)-5
        anchor x=(px)5 y=(px)5
      }
    }
  }
}"##;
        let outcome = vector(doc, true, &[]).expect("perception should run");

        assert_eq!(outcome.exit_code, 0, "stdout: {}", outcome.stdout);
        assert!(outcome.stdout.contains("\"collision_pair_count\": 1"));
        assert!(outcome.stdout.contains("\"total_intersection_count\": 1"));
        assert!(outcome.stdout.contains("\"first_path_id\": \"horizontal\""));
        assert!(outcome.stdout.contains("\"second_path_id\": \"vertical\""));
    }

    #[test]
    fn vector_human_reports_no_paths() {
        let doc = r##"zenith version=1 {
  project id="proj" name="Empty"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc" title="Empty" {
    page id="pg" w=(px)200 h=(px)120 { }
  }
}"##;

        let outcome = vector(doc, false, &[]).expect("perception should run");

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, "vector perception: no path nodes");
    }

    #[test]
    fn vector_filters_selected_nodes() {
        let doc = r##"zenith version=1 {
  project id="proj" name="Perceive"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc" title="Perceive" {
    page id="pg" w=(px)200 h=(px)120 {
      path id="one" closed=#true {
        anchor x=(px)0 y=(px)0
        anchor x=(px)10 y=(px)0
        anchor x=(px)10 y=(px)10
      }
      path id="two" closed=#true {
        anchor x=(px)20 y=(px)0
        anchor x=(px)30 y=(px)0
        anchor x=(px)30 y=(px)10
      }
    }
  }
}"##;
        let outcome = vector(doc, true, &["two".to_owned()]).expect("perception should run");

        assert_eq!(outcome.exit_code, 0, "stdout: {}", outcome.stdout);
        assert!(outcome.stdout.contains("\"path_count\": 1"));
        assert!(!outcome.stdout.contains("\"id\": \"one\""));
        assert!(outcome.stdout.contains("\"id\": \"two\""));
    }

    #[test]
    fn vector_rejects_missing_selected_node() {
        let err = vector(DOC, true, &["missing".to_owned()]).expect_err("missing node rejects");

        assert_eq!(err.exit_code, 2);
        assert!(err.message.contains("perceive.node_not_found"));
        assert!(err.message.contains("missing"));
    }
}
