//! `zenith inspect path` — topology, bounds, and optional craft for one path node.
//!
//! Coordinates are local path units (authored anchor space), not page-absolute.
//! Fill bounds use closed contours only via
//! [`zenith_geometry::compound_path_fill_bounds`] at
//! [`zenith_geometry::PATH_CONSUMPTION_TOLERANCE`].

use zenith_core::{Dimension, KdlAdapter, KdlSource, Node, Page, PathAnchor, PathNode, Unit};
use zenith_geometry::{
    CompoundFillRule, CompoundPathGeometry, PATH_CONSUMPTION_TOLERANCE,
    PathAnchor as GeomPathAnchor, PathGeometry, Point2, RectBounds, compound_path_fill_bounds,
};
use zenith_perception::{
    CompoundVectorPathPerceptionInput, PerceptionDiagnostic, PerceptionSeverity,
    VectorPathContourInput, analyze_compound_vector_path,
};

use crate::commands::serialize_pretty;

use super::document::InspectCmdErr;

const SCHEMA: &str = "zenith-inspect-path-v1";

// ── Output DTOs ───────────────────────────────────────────────────────────────

/// Machine-readable envelope for `zenith inspect path`.
#[derive(Debug, serde::Serialize)]
pub struct PathInspectOutput {
    pub schema: &'static str,
    pub id: String,
    pub kind: &'static str,
    pub fill_rule: &'static str,
    pub has_fill: bool,
    pub has_stroke: bool,
    pub subpath_count: usize,
    pub closed_subpath_count: usize,
    pub open_subpath_count: usize,
    pub anchor_count: usize,
    pub segment_count: usize,
    pub geometry_ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<BoundsOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_bounds: Option<BoundsOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub craft: Option<CraftOutput>,
}

/// Axis-aligned extrema bounds in local path coordinates.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct BoundsOutput {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

/// Optional perception craft summary (only when `--craft` is set).
#[derive(Debug, serde::Serialize)]
pub struct CraftOutput {
    pub anchor_economy_score: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tangent_quality_score_mean: Option<f32>,
    pub small_legibility_score: f32,
    pub diagnostics: Vec<CraftDiagnosticOutput>,
}

/// One craft diagnostic code from perception.
#[derive(Debug, serde::Serialize)]
pub struct CraftDiagnosticOutput {
    pub code: &'static str,
    pub severity: &'static str,
    pub message: &'static str,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Run `zenith inspect path` over in-memory source.
///
/// - `node_id` — id of a `path` node.
/// - `json`    — emit [`PathInspectOutput`] JSON.
/// - `craft`   — attach perception craft scores + diagnostic codes.
///
/// Missing id or wrong kind → exit code 2.
pub fn run(src: &str, node_id: &str, json: bool, craft: bool) -> Result<String, InspectCmdErr> {
    let doc = KdlAdapter
        .parse(src.as_bytes())
        .map_err(|e| InspectCmdErr {
            message: crate::report::parse_error_line(src, &e),
            exit_code: 2,
        })?;

    let path = find_path_node(&doc.body.pages, node_id)?;
    let output = inspect_path(path, craft);

    if json {
        Ok(serialize_pretty(&output))
    } else {
        Ok(render_human(&output))
    }
}

// ── Inspection core ───────────────────────────────────────────────────────────

fn inspect_path(path: &PathNode, include_craft: bool) -> PathInspectOutput {
    let fill_rule = compound_fill_rule(path.fill_rule.as_deref());
    let contours = VectorPathContourInput::from_path_node(path);
    let report = analyze_compound_vector_path(CompoundVectorPathPerceptionInput {
        contours: &contours,
        fill_rule: Some(fill_rule),
    });

    let (geometry_ok, bounds, fill_bounds) = local_geometry_summary(&contours);

    let craft = if include_craft {
        Some(CraftOutput {
            anchor_economy_score: report.anchor_economy.economy_score,
            tangent_quality_score_mean: report.tangent_quality_score_mean,
            small_legibility_score: report.small_legibility.score,
            diagnostics: report.diagnostics.iter().map(craft_diagnostic).collect(),
        })
    } else {
        None
    };

    PathInspectOutput {
        schema: SCHEMA,
        id: path.id.clone(),
        kind: "path",
        fill_rule: fill_rule_label(fill_rule),
        has_fill: path.fill.is_some(),
        has_stroke: path.stroke.is_some(),
        subpath_count: report.contour_count,
        closed_subpath_count: report.closed_subpath_count,
        open_subpath_count: report.open_subpath_count,
        anchor_count: report.anchor_count,
        segment_count: report.segment_count,
        geometry_ok,
        bounds: bounds.map(bounds_output),
        fill_bounds: fill_bounds.map(bounds_output),
        craft,
    }
}

/// Build local (non-page) compound geometry: extrema bounds + closed fill bounds.
///
/// `geometry_ok` is false when any contour fails to convert (mixed partial
/// handles, non-`px` units, non-finite coordinates).
fn local_geometry_summary(
    contours: &[VectorPathContourInput<'_>],
) -> (bool, Option<RectBounds>, Option<RectBounds>) {
    let mut geometry_contours = Vec::with_capacity(contours.len());
    for contour in contours {
        let mut anchors = Vec::with_capacity(contour.anchors.len());
        for anchor in contour.anchors {
            let Some(geom) = local_geometry_anchor(anchor) else {
                return (false, None, None);
            };
            anchors.push(geom);
        }
        let Ok(geometry_contour) = PathGeometry::new(anchors, contour.closed) else {
            return (false, None, None);
        };
        geometry_contours.push(geometry_contour);
    }

    let geometry = CompoundPathGeometry::new(geometry_contours);
    let bounds = match geometry.bounds() {
        Ok(b) => b,
        Err(_) => return (false, None, None),
    };
    let fill_bounds = match compound_path_fill_bounds(&geometry, PATH_CONSUMPTION_TOLERANCE) {
        Ok(b) => b,
        Err(_) => return (false, bounds, None),
    };
    (true, bounds, fill_bounds)
}

fn local_geometry_anchor(anchor: &PathAnchor) -> Option<GeomPathAnchor> {
    GeomPathAnchor::new(
        point_from_px_pair(anchor.x.as_ref(), anchor.y.as_ref())?,
        optional_point_from_px_pair(anchor.in_x.as_ref(), anchor.in_y.as_ref())?,
        optional_point_from_px_pair(anchor.out_x.as_ref(), anchor.out_y.as_ref())?,
    )
    .ok()
}

fn optional_point_from_px_pair(
    x: Option<&Dimension>,
    y: Option<&Dimension>,
) -> Option<Option<Point2>> {
    match (x, y) {
        (None, None) => Some(None),
        (Some(x), Some(y)) => point_from_px_pair(Some(x), Some(y)).map(Some),
        (Some(_), None) | (None, Some(_)) => None,
    }
}

fn point_from_px_pair(x: Option<&Dimension>, y: Option<&Dimension>) -> Option<Point2> {
    Point2::new(px_value(x)?, px_value(y)?).ok()
}

fn px_value(dimension: Option<&Dimension>) -> Option<f64> {
    let dimension = dimension?;
    match dimension.unit {
        Unit::Px if dimension.value.is_finite() => Some(dimension.value),
        Unit::Px | Unit::Pt | Unit::Pct | Unit::Deg | Unit::Unknown(_) => None,
    }
}

fn compound_fill_rule(value: Option<&str>) -> CompoundFillRule {
    match value {
        Some("evenodd") => CompoundFillRule::EvenOdd,
        Some(_) | None => CompoundFillRule::NonZero,
    }
}

fn fill_rule_label(rule: CompoundFillRule) -> &'static str {
    match rule {
        CompoundFillRule::EvenOdd => "evenodd",
        CompoundFillRule::NonZero => "nonzero",
    }
}

fn bounds_output(bounds: RectBounds) -> BoundsOutput {
    BoundsOutput {
        min_x: bounds.min_x,
        min_y: bounds.min_y,
        max_x: bounds.max_x,
        max_y: bounds.max_y,
    }
}

fn craft_diagnostic(diagnostic: &PerceptionDiagnostic) -> CraftDiagnosticOutput {
    CraftDiagnosticOutput {
        code: diagnostic.code,
        severity: match diagnostic.severity {
            PerceptionSeverity::Info => "info",
            PerceptionSeverity::Warning => "warning",
        },
        message: diagnostic.message,
    }
}

// ── Node lookup ───────────────────────────────────────────────────────────────

fn find_path_node<'a>(pages: &'a [Page], id: &str) -> Result<&'a PathNode, InspectCmdErr> {
    match find_node(pages, id) {
        None => Err(InspectCmdErr {
            message: format!(
                "error[inspect.node_not_found]: node '{id}' not found; run `zenith inspect \
                 <FILE>` to list node ids"
            ),
            exit_code: 2,
        }),
        Some(Node::Path(path)) => Ok(path),
        Some(other) => Err(InspectCmdErr {
            message: format!(
                "error: node '{id}' is kind '{}', expected 'path'",
                other.kind_str()
            ),
            exit_code: 2,
        }),
    }
}

fn find_node<'a>(pages: &'a [Page], id: &str) -> Option<&'a Node> {
    for page in pages {
        if let Some(node) = search_nodes(&page.children, id) {
            return Some(node);
        }
    }
    None
}

/// Depth-first search matching [`Node::id`]. Table cells are walked explicitly
/// because [`Node::children`] only covers frame/group/unknown containers.
fn search_nodes<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Node> {
    for node in nodes {
        if node.id() == Some(id) {
            return Some(node);
        }
        if let Some(children) = node.children()
            && let Some(found) = search_nodes(children, id)
        {
            return Some(found);
        }
        if let Node::Table(t) = node {
            for row in &t.rows {
                for cell in &row.cells {
                    if let Some(found) = search_nodes(&cell.children, id) {
                        return Some(found);
                    }
                }
            }
        }
    }
    None
}

// ── Human rendering ───────────────────────────────────────────────────────────

fn render_human(output: &PathInspectOutput) -> String {
    let mut lines = Vec::new();
    lines.push(format!("path {}", output.id));
    lines.push(format!("  fill_rule: {}", output.fill_rule));
    lines.push(format!("  has_fill: {}", output.has_fill));
    lines.push(format!("  has_stroke: {}", output.has_stroke));
    lines.push(format!(
        "  subpaths: {} (closed={} open={})",
        output.subpath_count, output.closed_subpath_count, output.open_subpath_count
    ));
    lines.push(format!(
        "  anchors: {}  segments: {}",
        output.anchor_count, output.segment_count
    ));
    lines.push(format!("  geometry_ok: {}", output.geometry_ok));
    lines.push(format!(
        "  bounds: {}",
        format_bounds(output.bounds.as_ref())
    ));
    lines.push(format!(
        "  fill_bounds: {}",
        format_bounds(output.fill_bounds.as_ref())
    ));

    if let Some(craft) = &output.craft {
        lines.push("  craft:".to_owned());
        lines.push(format!(
            "    anchor_economy_score: {:.3}",
            craft.anchor_economy_score
        ));
        match craft.tangent_quality_score_mean {
            Some(score) => lines.push(format!("    tangent_quality_score_mean: {score:.3}")),
            None => lines.push("    tangent_quality_score_mean: none".to_owned()),
        }
        lines.push(format!(
            "    small_legibility_score: {:.3}",
            craft.small_legibility_score
        ));
        if craft.diagnostics.is_empty() {
            lines.push("    diagnostics: (none)".to_owned());
        } else {
            lines.push("    diagnostics:".to_owned());
            for diagnostic in &craft.diagnostics {
                lines.push(format!(
                    "      {}[{}]: {}",
                    diagnostic.severity, diagnostic.code, diagnostic.message
                ));
            }
        }
    }

    lines.join("\n")
}

fn format_bounds(bounds: Option<&BoundsOutput>) -> String {
    match bounds {
        Some(b) => format!(
            "min_x={} min_y={} max_x={} max_y={}",
            b.min_x, b.min_y, b.max_x, b.max_y
        ),
        None => "none".to_owned(),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn wrap(node_src: &str) -> String {
        format!(
            r##"zenith version=1 {{
  project id="proj.ip" name="Inspect Path"
  tokens format="zenith-token-v1" {{
    color id="color.brand" value="#112233"
    color id="color.ink" value="#000000"
  }}
  styles {{}}
  document id="doc.ip" title="Inspect Path" {{
    page id="page.ip" w=(px)400 h=(px)300 {{
      {node_src}
    }}
  }}
}}
"##
        )
    }

    fn parse_json(out: &str) -> serde_json::Value {
        serde_json::from_str(out).expect("inspect path json must parse")
    }

    #[test]
    fn closed_square_topology_and_bounds() {
        let src = wrap(
            r##"path id="square" closed=#true fill=(token)"color.brand" {
        anchor x=(px)0 y=(px)0
        anchor x=(px)40 y=(px)0
        anchor x=(px)40 y=(px)40
        anchor x=(px)0 y=(px)40
      }"##,
        );

        let out = run(&src, "square", true, false).expect("inspect path");
        let v = parse_json(&out);

        assert_eq!(v["schema"], "zenith-inspect-path-v1");
        assert_eq!(v["id"], "square");
        assert_eq!(v["kind"], "path");
        assert_eq!(v["fill_rule"], "nonzero");
        assert_eq!(v["has_fill"], true);
        assert_eq!(v["has_stroke"], false);
        assert_eq!(v["subpath_count"], 1);
        assert_eq!(v["closed_subpath_count"], 1);
        assert_eq!(v["open_subpath_count"], 0);
        assert_eq!(v["anchor_count"], 4);
        assert_eq!(v["segment_count"], 4);
        assert_eq!(v["geometry_ok"], true);
        assert_eq!(v["bounds"]["min_x"], 0.0);
        assert_eq!(v["bounds"]["min_y"], 0.0);
        assert_eq!(v["bounds"]["max_x"], 40.0);
        assert_eq!(v["bounds"]["max_y"], 40.0);
        assert_eq!(v["fill_bounds"]["min_x"], 0.0);
        assert_eq!(v["fill_bounds"]["min_y"], 0.0);
        assert_eq!(v["fill_bounds"]["max_x"], 40.0);
        assert_eq!(v["fill_bounds"]["max_y"], 40.0);
        assert!(v.get("craft").is_none());
    }

    #[test]
    fn open_path_fill_bounds_none() {
        let src = wrap(
            r##"path id="stroke" stroke=(token)"color.ink" {
        anchor x=(px)0 y=(px)0
        anchor x=(px)40 y=(px)0
        anchor x=(px)40 y=(px)40
      }"##,
        );

        let out = run(&src, "stroke", true, false).expect("inspect path");
        let v = parse_json(&out);

        assert_eq!(v["id"], "stroke");
        assert_eq!(v["fill_rule"], "nonzero");
        assert_eq!(v["has_fill"], false);
        assert_eq!(v["has_stroke"], true);
        assert_eq!(v["subpath_count"], 1);
        assert_eq!(v["closed_subpath_count"], 0);
        assert_eq!(v["open_subpath_count"], 1);
        assert_eq!(v["anchor_count"], 3);
        assert_eq!(v["segment_count"], 2);
        assert_eq!(v["geometry_ok"], true);
        assert_eq!(v["bounds"]["min_x"], 0.0);
        assert_eq!(v["bounds"]["max_x"], 40.0);
        assert!(
            v.get("fill_bounds").is_none() || v["fill_bounds"].is_null(),
            "open path must omit fill_bounds, got {:?}",
            v.get("fill_bounds")
        );
    }

    #[test]
    fn compound_evenodd_topology() {
        let src = wrap(
            r##"path id="ring" fill=(token)"color.brand" fill-rule="evenodd" {
        subpath closed=#true {
          anchor x=(px)0 y=(px)0
          anchor x=(px)80 y=(px)0
          anchor x=(px)80 y=(px)80
          anchor x=(px)0 y=(px)80
        }
        subpath closed=#true {
          anchor x=(px)20 y=(px)20
          anchor x=(px)60 y=(px)20
          anchor x=(px)60 y=(px)60
          anchor x=(px)20 y=(px)60
        }
      }"##,
        );

        let out = run(&src, "ring", true, false).expect("inspect path");
        let v = parse_json(&out);

        assert_eq!(v["fill_rule"], "evenodd");
        assert_eq!(v["subpath_count"], 2);
        assert_eq!(v["closed_subpath_count"], 2);
        assert_eq!(v["open_subpath_count"], 0);
        assert_eq!(v["anchor_count"], 8);
        assert_eq!(v["segment_count"], 8);
        assert_eq!(v["geometry_ok"], true);
        assert_eq!(v["bounds"]["min_x"], 0.0);
        assert_eq!(v["bounds"]["max_x"], 80.0);
        assert_eq!(v["fill_bounds"]["min_x"], 0.0);
        assert_eq!(v["fill_bounds"]["max_x"], 80.0);
        assert_eq!(v["fill_bounds"]["max_y"], 80.0);
    }

    #[test]
    fn cubic_extrema_beyond_anchor_hull() {
        // Open cubic with handles at y=10; curve extrema y=7.5 exceeds anchors at y=0.
        let src = wrap(
            r##"path id="bow" {
        anchor x=(px)0 y=(px)0 out-x=(px)0 out-y=(px)10
        anchor x=(px)10 y=(px)0 in-x=(px)10 in-y=(px)10
      }"##,
        );

        let out = run(&src, "bow", true, false).expect("inspect path");
        let v = parse_json(&out);

        assert_eq!(v["geometry_ok"], true);
        assert_eq!(v["anchor_count"], 2);
        assert_eq!(v["segment_count"], 1);
        assert_eq!(v["bounds"]["min_x"], 0.0);
        assert_eq!(v["bounds"]["max_x"], 10.0);
        assert_eq!(v["bounds"]["min_y"], 0.0);
        assert_eq!(v["bounds"]["max_y"], 7.5);
        assert!(
            v.get("fill_bounds").is_none() || v["fill_bounds"].is_null(),
            "open bowed path has no fill_bounds"
        );
    }

    #[test]
    fn missing_node_errors() {
        let src = wrap(
            r##"path id="square" closed=#true {
        anchor x=(px)0 y=(px)0
        anchor x=(px)10 y=(px)0
        anchor x=(px)10 y=(px)10
      }"##,
        );

        let err = run(&src, "missing", true, false).expect_err("must fail");
        assert_eq!(err.exit_code, 2);
        assert!(
            err.message.contains("not found"),
            "expected not-found message, got {}",
            err.message
        );
    }

    #[test]
    fn wrong_kind_errors() {
        let src = wrap(r##"rect id="box" x=(px)0 y=(px)0 w=(px)10 h=(px)10"##);

        let err = run(&src, "box", true, false).expect_err("must fail");
        assert_eq!(err.exit_code, 2);
        assert!(
            err.message.contains("expected 'path'"),
            "expected wrong-kind message, got {}",
            err.message
        );
        assert!(
            err.message.contains("rect"),
            "expected actual kind in message, got {}",
            err.message
        );
    }

    #[test]
    fn craft_flag_includes_scores() {
        let src = wrap(
            r##"path id="square" closed=#true {
        anchor x=(px)0 y=(px)0
        anchor x=(px)40 y=(px)0
        anchor x=(px)40 y=(px)40
        anchor x=(px)0 y=(px)40
      }"##,
        );

        let out = run(&src, "square", true, true).expect("inspect path");
        let v = parse_json(&out);

        assert!(v.get("craft").is_some());
        assert!(v["craft"]["anchor_economy_score"].is_number());
        assert!(v["craft"]["small_legibility_score"].is_number());
        assert!(v["craft"]["diagnostics"].is_array());
    }

    #[test]
    fn human_output_mentions_topology() {
        let src = wrap(
            r##"path id="square" closed=#true {
        anchor x=(px)0 y=(px)0
        anchor x=(px)40 y=(px)0
        anchor x=(px)40 y=(px)40
        anchor x=(px)0 y=(px)40
      }"##,
        );

        let out = run(&src, "square", false, false).expect("inspect path");
        assert!(out.contains("path square"));
        assert!(out.contains("fill_rule: nonzero"));
        assert!(out.contains("subpaths: 1 (closed=1 open=0)"));
        assert!(out.contains("geometry_ok: true"));
        assert!(out.contains("fill_bounds:"));
    }

    #[test]
    fn tree_inspect_still_works_on_same_doc() {
        // Sanity: path module coexists with document inspect; does not own tree path.
        let src = wrap(
            r##"path id="square" closed=#true {
        anchor x=(px)0 y=(px)0
        anchor x=(px)40 y=(px)0
        anchor x=(px)40 y=(px)40
        anchor x=(px)0 y=(px)40
      }"##,
        );
        let tree = super::super::run(&src, None, true, None).expect("tree inspect");
        let v: serde_json::Value = serde_json::from_str(&tree).expect("tree json");
        assert_eq!(v["schema"], "zenith-inspect-v1");
    }
}
