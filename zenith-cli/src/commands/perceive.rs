//! Pure logic for `zenith perceive`.

use serde::Serialize;
use zenith_core::{KdlAdapter, KdlSource, Node, PathNode};
use zenith_perception::{
    CompoundFillRule, CompoundVectorPathPerceptionInput, CompoundVectorPathPerceptionReport,
    PerceptionDiagnostic, PerceptionSeverity, VectorPathContourInput, analyze_compound_vector_path,
};

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

pub fn vector(src: &str, json: bool) -> Result<PerceiveOutcome, PerceiveCmdErr> {
    let doc = KdlAdapter
        .parse(src.as_bytes())
        .map_err(|e| PerceiveCmdErr {
            message: format!("error[parse.error]: {}", e.message),
            exit_code: 2,
        })?;

    let mut paths = Vec::new();
    for page in &doc.body.pages {
        collect_paths(&page.children, &mut paths);
    }

    let output_paths = paths
        .iter()
        .map(|path| analyze_path(path))
        .collect::<Vec<_>>();
    let warning_count = output_paths
        .iter()
        .flat_map(|path| path.diagnostics.iter())
        .filter(|diagnostic| diagnostic.severity == "warning")
        .count();
    let info_count = output_paths
        .iter()
        .flat_map(|path| path.diagnostics.iter())
        .filter(|diagnostic| diagnostic.severity == "info")
        .count();
    let output = VectorDocumentOutput {
        schema: "zenith-perceive-vector-v1",
        path_count: output_paths.len(),
        warning_count,
        info_count,
        paths: output_paths,
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

fn analyze_path(path: &PathNode) -> VectorPathOutput {
    let contours = VectorPathContourInput::from_path_node(path);
    let report = analyze_compound_vector_path(CompoundVectorPathPerceptionInput {
        contours: &contours,
        fill_rule: fill_rule(path.fill_rule.as_deref()),
    });

    vector_path_output(&path.id, report)
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
        let outcome = vector(DOC, true).expect("perception should run");

        assert_eq!(outcome.exit_code, 0, "stdout: {}", outcome.stdout);
        assert!(
            outcome
                .stdout
                .contains("\"schema\": \"zenith-perceive-vector-v1\"")
        );
        assert!(outcome.stdout.contains("\"path_count\": 1"));
        assert!(outcome.stdout.contains("\"id\": \"mark\""));
        assert!(outcome.stdout.contains("\"anchor_count\": 4"));
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

        let outcome = vector(doc, false).expect("perception should run");

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, "vector perception: no path nodes");
    }
}
