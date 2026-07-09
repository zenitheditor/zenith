//! Integration tests for the `node.unsupported_child` diagnostic: child nodes
//! authored under a kind that does not consume them are captured at parse time
//! and reported (with severity Warning) from validation.

mod common;

use common::*;
use zenith_core::{KdlAdapter, KdlSource};

/// Parse a `.zen` document whose page body is `body_src`. Parsing is lenient, so
/// this succeeds even for documents that will produce validation diagnostics.
fn parse_doc(body_src: &str) -> Document {
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.uc" name="Unsupported children"
  tokens format="zenith-token-v1" {{
  }}
  styles {{
  }}
  document id="doc.uc" title="UC" {{
    page id="page.uc" w=(px)400 h=(px)300 {{
      {body_src}
    }}
  }}
}}
"##
    );
    KdlAdapter
        .parse(src.as_bytes())
        .expect("parse must succeed")
}

fn warns(body_src: &str) -> bool {
    has_code(&validate(&parse_doc(body_src)), "node.unsupported_child")
}

// ── Positive cases: the discarded child must be reported ─────────────────────

#[test]
fn ellipse_with_text_child_warns() {
    assert!(warns(
        r##"ellipse id="disc" x=(px)0 y=(px)0 w=(px)80 h=(px)80 { text id="t" { span "M" } }"##
    ));
}

#[test]
fn ellipse_with_group_child_warns() {
    // The `group` (which carries the text) is the discarded child; the engine
    // does not descend into a dropped subtree, so exactly one warning fires.
    assert!(warns(
        r##"ellipse id="disc" x=(px)0 y=(px)0 w=(px)80 h=(px)80 {
            group id="g" { text id="t" { span "M" } }
        }"##
    ));
}

#[test]
fn rect_with_child_warns() {
    assert!(warns(
        r##"rect id="r" x=(px)0 y=(px)0 w=(px)80 h=(px)80 { text id="t" { span "x" } }"##
    ));
}

#[test]
fn image_with_child_warns() {
    assert!(warns(
        r##"image id="im" asset="a1" x=(px)0 y=(px)0 w=(px)80 h=(px)80 { rect id="r" x=(px)0 y=(px)0 w=(px)8 h=(px)8 }"##
    ));
}

#[test]
fn line_with_child_warns() {
    assert!(warns(
        r##"line id="ln" x1=(px)0 y1=(px)0 x2=(px)80 y2=(px)80 { text id="t" { span "x" } }"##
    ));
}

// ── Negative cases: recognized children must NOT be reported ─────────────────

#[test]
fn text_with_span_does_not_warn() {
    assert!(!warns(
        r##"text id="t" x=(px)0 y=(px)0 w=(px)80 h=(px)80 { span "hello" }"##
    ));
}

#[test]
fn path_with_anchor_does_not_warn() {
    assert!(!warns(
        r##"path id="p" { anchor x=(px)0 y=(px)0
            anchor x=(px)10 y=(px)10 }"##
    ));
}

#[test]
fn path_with_subpath_anchor_does_not_warn() {
    assert!(!warns(
        r##"path id="p" { subpath { anchor x=(px)0 y=(px)0
            anchor x=(px)10 y=(px)10 } }"##
    ));
}

#[test]
fn polygon_with_point_does_not_warn() {
    assert!(!warns(
        r##"polygon id="pg" { point x=(px)0 y=(px)0
            point x=(px)10 y=(px)0
            point x=(px)5 y=(px)9 }"##
    ));
}

#[test]
fn polyline_with_point_does_not_warn() {
    assert!(!warns(
        r##"polyline id="pl" { point x=(px)0 y=(px)0
            point x=(px)10 y=(px)0 }"##
    ));
}

#[test]
fn shape_with_span_does_not_warn() {
    assert!(!warns(
        r##"shape id="s" x=(px)0 y=(px)0 w=(px)80 h=(px)80 kind="rounded" { span "OK" }"##
    ));
}

#[test]
fn instance_with_override_does_not_warn() {
    assert!(!warns(
        r##"instance id="inst" component="c1" { override ref="headline" { span "New" } }"##
    ));
}

#[test]
fn table_with_column_and_rows_does_not_warn() {
    assert!(!warns(
        r##"table id="tb" x=(px)0 y=(px)0 w=(px)200 h=(px)100 {
            column width=(px)100
            row { cell { text id="ct" { span "A" } } }
        }"##
    ));
}

#[test]
fn group_with_rect_does_not_warn() {
    assert!(!warns(
        r##"group id="g" { rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 }"##
    ));
}

#[test]
fn frame_with_text_does_not_warn() {
    assert!(!warns(
        r##"frame id="f" x=(px)0 y=(px)0 w=(px)80 h=(px)80 { text id="t" { span "x" } }"##
    ));
}

#[test]
fn chart_with_series_does_not_warn() {
    assert!(!warns(
        r##"chart id="ch" kind="bar" x=(px)0 y=(px)0 w=(px)200 h=(px)100 {
            series label="Rev" 1.0 2.0 3.0
        }"##
    ));
}

#[test]
fn code_with_content_does_not_warn() {
    assert!(!warns(
        r##"code id="cd" x=(px)0 y=(px)0 w=(px)200 h=(px)100 { content "fn main() {}" }"##
    ));
}

#[test]
fn code_with_span_warns() {
    // `code` carries its source in a `content` child; a `span` is `text`'s shape,
    // is never consumed here, and would be silently discarded.
    assert!(warns(
        r##"code id="cd" x=(px)0 y=(px)0 w=(px)200 h=(px)100 { span "x" }"##
    ));
}

// ── Diagnostic payload: span + parent id + message ───────────────────────────

#[test]
fn diagnostic_carries_child_span_parent_id_and_message() {
    let doc = parse_doc(
        r##"ellipse id="disc" x=(px)0 y=(px)0 w=(px)80 h=(px)80 { text id="t" { span "M" } }"##,
    );
    let report = validate(&doc);
    let d = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unsupported_child")
        .expect("must emit node.unsupported_child");

    assert_eq!(d.severity, Severity::Warning);
    // Parent id is carried as the subject.
    assert_eq!(d.subject_id.as_deref(), Some("disc"));
    // The child's source span is present (byte offsets into the source).
    assert!(d.span.is_some(), "diagnostic must carry the child span");
    // Message names the parent kind, parent id, and the dropped child kind.
    assert!(d.message.contains("ellipse"), "message: {}", d.message);
    assert!(d.message.contains("disc"), "message: {}", d.message);
    assert!(d.message.contains("text"), "message: {}", d.message);
    assert!(d.message.contains("discarded"), "message: {}", d.message);
}

#[test]
fn diagnostic_is_governable_warning_in_catalog() {
    let info = zenith_core::diag_catalog::lookup("node.unsupported_child")
        .expect("node.unsupported_child must be catalogued");
    assert_eq!(info.severity, Severity::Warning);
    assert!(info.is_governable(), "Warning ⇒ governable");
}
