//! Integration tests: `role="decoration"` / `role="background"` exempt a node
//! and its whole subtree from `frame.child_overflow` and `layout.off_canvas`.

use zenith_core::{Document, KdlAdapter, KdlSource, layout_geometry_checks, validate};

const OVERFLOW: &str = "frame.child_overflow";
const OFF_CANVAS: &str = "layout.off_canvas";

/// A 200x200 page holding `children`.
fn doc_src(children: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.pe" name="PE"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
  }}
  styles {{
  }}
  document id="doc.pe" title="PE" {{
    page id="p" w=(px)200 h=(px)200 {{
{children}
    }}
  }}
}}
"##
    )
}

fn parse(src: &str) -> Document {
    KdlAdapter.parse(src.as_bytes()).expect("parse")
}

/// Codes from the main validation walk (absolute pages).
fn codes(children: &str) -> Vec<String> {
    validate(&parse(&doc_src(children)))
        .diagnostics
        .into_iter()
        .map(|d| d.code.to_string())
        .collect()
}

/// Codes from the layout-managed geometry pass. A row frame on the page makes
/// it layout-managed, so placement runs through `placement_walk`.
fn layout_codes(children: &str) -> Vec<String> {
    let with_row = format!(
        r#"      frame id="row" x=(px)0 y=(px)0 w=(px)100 h=(px)20 layout="row" {{
        rect id="in.row" h=(px)10 w=(px)10 fill=(token)"color.k"
      }}
{children}"#
    );
    let doc = parse(&doc_src(&with_row));
    layout_geometry_checks(&doc)
        .into_iter()
        .flatten()
        .map(|d| d.code.to_string())
        .collect()
}

fn has(codes: &[String], code: &str) -> bool {
    codes.iter().any(|c| c == code)
}

fn silent(codes: &[String]) {
    assert!(!has(codes, OVERFLOW), "{codes:?}");
    assert!(!has(codes, OFF_CANVAS), "{codes:?}");
}

/// A frame at (10,10) 50x50 holding `inner`.
fn in_frame(inner: &str) -> String {
    format!(
        r#"      frame id="f" x=(px)10 y=(px)10 w=(px)50 h=(px)50 {{
{inner}
      }}"#
    )
}

const PAST_FRAME: &str =
    r#"ellipse id="e" x=(px)30 y=(px)30 w=(px)80 h=(px)20 fill=(token)"color.k""#;

#[test]
fn plain_ellipse_past_frame_warns() {
    let c = codes(&in_frame(PAST_FRAME));
    assert!(has(&c, OVERFLOW), "{c:?}");
}

#[test]
fn decoration_ellipse_past_frame_is_silent() {
    let c = codes(&in_frame(
        r#"ellipse id="e" role="decoration" x=(px)30 y=(px)30 w=(px)80 h=(px)20 fill=(token)"color.k""#,
    ));
    silent(&c);
}

#[test]
fn decoration_ellipse_past_page_is_silent() {
    let c = codes(
        r#"      ellipse id="e" role="decoration" x=(px)150 y=(px)150 w=(px)100 h=(px)100 fill=(token)"color.k""#,
    );
    silent(&c);
}

#[test]
fn plain_ellipse_past_page_warns() {
    let c = codes(
        r#"      ellipse id="e" x=(px)150 y=(px)150 w=(px)100 h=(px)100 fill=(token)"color.k""#,
    );
    assert!(has(&c, OFF_CANVAS), "{c:?}");
}

#[test]
fn background_role_is_silent() {
    let c = codes(
        r#"      ellipse id="e" role="background" x=(px)-50 y=(px)-50 w=(px)400 h=(px)400 fill=(token)"color.k""#,
    );
    silent(&c);
    let c = codes(&in_frame(
        r#"ellipse id="e" role="background" x=(px)30 y=(px)30 w=(px)80 h=(px)20 fill=(token)"color.k""#,
    ));
    silent(&c);
}

#[test]
fn plain_child_of_decoration_group_is_silent() {
    let c = codes(
        r#"      group id="g" role="decoration" x=(px)0 y=(px)0 w=(px)200 h=(px)200 {
        ellipse id="e" x=(px)150 y=(px)150 w=(px)100 h=(px)100 fill=(token)"color.k"
      }"#,
    );
    silent(&c);
}

#[test]
fn nested_group_under_decoration_group_is_silent() {
    let c = codes(
        r#"      group id="g" role="decoration" x=(px)0 y=(px)0 w=(px)200 h=(px)200 {
        group id="g2" x=(px)0 y=(px)0 w=(px)200 h=(px)200 {
          ellipse id="e" x=(px)150 y=(px)150 w=(px)100 h=(px)100 fill=(token)"color.k"
        }
      }"#,
    );
    silent(&c);
}

#[test]
fn decoration_group_in_frame_exempts_overflow() {
    let c = codes(&in_frame(
        r#"group id="g" role="decoration" x=(px)0 y=(px)0 w=(px)50 h=(px)50 {
          ellipse id="e" x=(px)30 y=(px)30 w=(px)80 h=(px)20 fill=(token)"color.k"
        }"#,
    ));
    silent(&c);
}

#[test]
fn non_decoration_sibling_still_warns() {
    let src = doc_src(
        r#"      group id="g" role="decoration" x=(px)0 y=(px)0 w=(px)200 h=(px)200 {
        ellipse id="e1" x=(px)150 y=(px)150 w=(px)100 h=(px)100 fill=(token)"color.k"
      }
      ellipse id="e2" x=(px)150 y=(px)150 w=(px)100 h=(px)100 fill=(token)"color.k""#,
    );
    let report = validate(&parse(&src));
    let subjects: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.code == OFF_CANVAS)
        .map(|d| d.subject_id.as_deref())
        .collect();
    assert_eq!(subjects, vec![Some("e2")]);
}

// ── layout-managed page (placement_walk) ────────────────────────────────

#[test]
fn layout_page_plain_ellipse_warns() {
    let c = layout_codes(
        r#"      ellipse id="e" x=(px)150 y=(px)150 w=(px)100 h=(px)100 fill=(token)"color.k""#,
    );
    assert!(has(&c, OFF_CANVAS), "{c:?}");
    let c = layout_codes(&in_frame(PAST_FRAME));
    assert!(has(&c, OVERFLOW), "{c:?}");
}

#[test]
fn layout_page_decoration_ellipse_is_silent() {
    let c = layout_codes(
        r#"      ellipse id="e" role="decoration" x=(px)150 y=(px)150 w=(px)100 h=(px)100 fill=(token)"color.k""#,
    );
    silent(&c);
    let c = layout_codes(&in_frame(
        r#"ellipse id="e" role="decoration" x=(px)30 y=(px)30 w=(px)80 h=(px)20 fill=(token)"color.k""#,
    ));
    silent(&c);
}

#[test]
fn layout_page_background_is_silent() {
    let c = layout_codes(
        r#"      ellipse id="e" role="background" x=(px)-50 y=(px)-50 w=(px)400 h=(px)400 fill=(token)"color.k""#,
    );
    silent(&c);
}

#[test]
fn layout_page_decoration_group_subtree_is_silent() {
    let c = layout_codes(
        r#"      group id="g" role="decoration" x=(px)0 y=(px)0 w=(px)200 h=(px)200 {
        ellipse id="e" x=(px)150 y=(px)150 w=(px)100 h=(px)100 fill=(token)"color.k"
        group id="g2" x=(px)0 y=(px)0 w=(px)200 h=(px)200 {
          ellipse id="e2" x=(px)150 y=(px)150 w=(px)100 h=(px)100 fill=(token)"color.k"
        }
      }"#,
    );
    silent(&c);
}

#[test]
fn layout_page_non_decoration_sibling_still_warns() {
    let c = layout_codes(
        r#"      group id="g" role="decoration" x=(px)0 y=(px)0 w=(px)200 h=(px)200 {
        ellipse id="e1" x=(px)150 y=(px)150 w=(px)100 h=(px)100 fill=(token)"color.k"
      }
      ellipse id="e2" x=(px)150 y=(px)150 w=(px)100 h=(px)100 fill=(token)"color.k""#,
    );
    assert!(has(&c, OFF_CANVAS), "{c:?}");
}
