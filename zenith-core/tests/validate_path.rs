//! Integration tests for structured `path` node validation.

mod common;

use common::*;

fn parse_doc(node_src: &str) -> Document {
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.path" name="Path"
  tokens format="zenith-token-v1" {{
    token id="color.brand" type="color" value="#112233"
    token id="color.ink" type="color" value="#000000"
    token id="size.stroke" type="dimension" value=(px)2
  }}
  styles {{
  }}
  document id="doc.path" title="Path" {{
    page id="page.path" w=(px)400 h=(px)300 {{
      {node_src}
    }}
  }}
}}
"##
    );
    KdlAdapter
        .parse(src.as_bytes())
        .expect("parse must succeed")
}

#[test]
fn valid_path_with_handles_has_no_validation_errors() {
    let doc = parse_doc(
        r##"path id="logo.mark" closed=#true fill=(token)"color.brand" stroke=(token)"color.ink" stroke-width=(token)"size.stroke" stroke-alignment="center" fill-rule="evenodd" {
        anchor x=(px)0 y=(px)0 out-x=(px)20 out-y=(px)0
        anchor x=(px)80 y=(px)0 in-x=(px)60 in-y=(px)0 out-x=(px)100 out-y=(px)40
        anchor x=(px)80 y=(px)80 in-x=(px)100 in-y=(px)40
      }"##,
    );

    let report = validate(&doc);

    assert!(
        report
            .diagnostics
            .iter()
            .all(|d| d.severity != Severity::Error),
        "expected no validation errors; got {:?}",
        report.diagnostics
    );
}

#[test]
fn open_path_requires_two_anchors() {
    let doc = parse_doc(
        r##"path id="line.curve" {
        anchor x=(px)0 y=(px)0
      }"##,
    );

    let report = validate(&doc);

    assert!(
        has_code(&report, "shape.insufficient_points"),
        "expected insufficient anchors diagnostic; got {:?}",
        report.diagnostics
    );
}

#[test]
fn closed_path_requires_three_anchors() {
    let doc = parse_doc(
        r##"path id="shape.curve" closed=#true {
        anchor x=(px)0 y=(px)0
        anchor x=(px)40 y=(px)0
      }"##,
    );

    let report = validate(&doc);

    assert!(
        has_code(&report, "shape.insufficient_points"),
        "expected insufficient anchors diagnostic; got {:?}",
        report.diagnostics
    );
}

#[test]
fn missing_handle_pair_is_invalid_geometry() {
    let doc = parse_doc(
        r##"path id="line.curve" {
        anchor x=(px)0 y=(px)0 out-x=(px)20
        anchor x=(px)80 y=(px)0
      }"##,
    );

    let report = validate(&doc);

    assert!(
        has_code(&report, "node.invalid_geometry"),
        "expected invalid geometry diagnostic; got {:?}",
        report.diagnostics
    );
}

#[test]
fn path_unknown_property_suggests_fill() {
    let doc = parse_doc(
        r##"path id="line.curve" fil=(token)"color.brand" {
        anchor x=(px)0 y=(px)0
        anchor x=(px)80 y=(px)0
      }"##,
    );

    let report = validate(&doc);
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property")
        .expect("unknown property diagnostic");

    assert!(
        diag.message.contains("did you mean 'fill'?"),
        "expected fill suggestion; got {diag:?}"
    );
}

#[test]
fn unknown_anchor_kind_warns_without_parse_failure() {
    let doc = parse_doc(
        r##"path id="line.curve" {
        anchor x=(px)0 y=(px)0 kind="auto"
        anchor x=(px)80 y=(px)0
      }"##,
    );

    let report = validate(&doc);
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property")
        .expect("unknown anchor kind diagnostic");

    assert_eq!(diag.severity, Severity::Warning);
    assert!(
        diag.message.contains("anchor[0]") && diag.message.contains("kind 'auto'"),
        "expected anchor kind warning; got {diag:?}"
    );
}

#[test]
fn unknown_path_stroke_linejoin_warns_without_parse_failure() {
    let doc = parse_doc(
        r##"path id="line.curve" stroke-linejoin="arced" {
        anchor x=(px)0 y=(px)0
        anchor x=(px)80 y=(px)0
      }"##,
    );

    let report = validate(&doc);
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property" && d.message.contains("stroke-linejoin"))
        .expect("unknown stroke-linejoin diagnostic");

    assert_eq!(diag.severity, Severity::Warning);
    assert!(!report.has_errors());
}

#[test]
fn invalid_path_stroke_miter_limit_is_geometry_error() {
    let doc = parse_doc(
        r##"path id="line.curve" stroke-miter-limit=0 {
        anchor x=(px)0 y=(px)0
        anchor x=(px)80 y=(px)0
      }"##,
    );

    let report = validate(&doc);
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.invalid_geometry" && d.message.contains("stroke-miter-limit"))
        .expect("invalid stroke-miter-limit diagnostic");

    assert_eq!(diag.severity, Severity::Error);
    assert!(report.has_errors());
}
