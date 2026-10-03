//! Container child spaces in validation: placement checks and table contrast
//! read nodes in page space, through the translation of enclosing groups and
//! frames.

mod common;

use common::contrast::contrast_report;
use common::*;

fn parse_doc(body: &str) -> Document {
    let src = format!(
        r##"zenith version=1 {{
  project id="p" name="P"
  tokens format="zenith-token-v1" {{
    token id="color.page" type="color" value="#ffffff"
    token id="color.ink" type="color" value="#ffffff"
    token id="color.dark" type="color" value="#101010"
  }}
  styles {{}}
  document id="d" title="D" {{
    page id="pg" w=(px)400 h=(px)300 background=(token)"color.page" {{
      {body}
    }}
  }}
}}"##
    );
    KdlAdapter.parse(src.as_bytes()).expect("parse")
}

fn off_canvas_ids(report: &ValidationReport) -> Vec<&str> {
    report
        .diagnostics
        .iter()
        .filter(|d| d.code == "layout.off_canvas")
        .filter_map(|d| d.subject_id.as_deref())
        .collect()
}

#[test]
fn off_canvas_reads_rect_through_group_offset() {
    // Authored (300, 10) fits the 400-wide page. The group moves it to
    // page x 450, past the right edge.
    let doc = parse_doc(
        r##"group id="g" x=(px)150 y=(px)0 {
        rect id="r" x=(px)300 y=(px)10 w=(px)50 h=(px)50 fill=(token)"color.dark"
      }"##,
    );
    let report = validate(&doc);
    assert_eq!(off_canvas_ids(&report), vec!["r"], "{:?}", codes(&report));
}

#[test]
fn off_canvas_accepts_rect_moved_back_on_page_by_group() {
    // Authored x -40 leaves the page. The group moves it to page x 60.
    let doc = parse_doc(
        r##"group id="g" x=(px)100 y=(px)0 {
        rect id="r" x=(px)-40 y=(px)10 w=(px)50 h=(px)50 fill=(token)"color.dark"
      }"##,
    );
    let report = validate(&doc);
    assert!(off_canvas_ids(&report).is_empty(), "{:?}", codes(&report));
}

#[test]
fn off_canvas_accumulates_nested_groups() {
    // 120 + 120 + 120 = 360; a 50-wide rect ends at 410 > 400.
    let doc = parse_doc(
        r##"group id="g1" x=(px)120 {
        group id="g2" x=(px)120 {
          rect id="r" x=(px)120 y=(px)10 w=(px)50 h=(px)50 fill=(token)"color.dark"
        }
      }"##,
    );
    let report = validate(&doc);
    assert_eq!(off_canvas_ids(&report), vec!["r"], "{:?}", codes(&report));
}

#[test]
fn child_overflow_compares_page_space_boxes_inside_group() {
    // Frame and child both sit in the group; in page space the child
    // (110..150) lies inside the frame (100..200).
    let doc = parse_doc(
        r##"group id="g" x=(px)100 y=(px)100 {
        frame id="f" x=(px)0 y=(px)0 w=(px)100 h=(px)100 {
          rect id="inner" x=(px)10 y=(px)10 w=(px)40 h=(px)40 fill=(token)"color.dark"
        }
      }"##,
    );
    let report = validate(&doc);
    assert!(
        !has_code(&report, "frame.child_overflow"),
        "{:?}",
        codes(&report)
    );
}

#[test]
fn child_overflow_inside_group_still_reports_protrusion() {
    // The child (page 180..240) protrudes past the frame (page 100..200).
    let doc = parse_doc(
        r##"group id="g" x=(px)100 y=(px)100 {
        frame id="f" x=(px)0 y=(px)0 w=(px)100 h=(px)100 {
          rect id="inner" x=(px)80 y=(px)10 w=(px)60 h=(px)40 fill=(token)"color.dark"
        }
      }"##,
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "frame.child_overflow"),
        "{:?}",
        codes(&report)
    );
}

#[test]
fn child_overflow_frame_outside_group_compares_group_child_in_page_space() {
    // The frame (page 0..200) holds a group at 150; the rect sits at page
    // 150..190, inside the frame. Read authored (0..40) it would also fit,
    // so a second rect at authored 100 (page 250..290) must protrude.
    let doc = parse_doc(
        r##"frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 {
        group id="g" x=(px)150 y=(px)0 {
          rect id="fits" x=(px)0 y=(px)10 w=(px)40 h=(px)40 fill=(token)"color.dark"
          rect id="past" x=(px)100 y=(px)10 w=(px)40 h=(px)40 fill=(token)"color.dark"
        }
      }"##,
    );
    let report = validate(&doc);
    let overflow: Vec<&str> = report
        .diagnostics
        .iter()
        .filter(|d| d.code == "frame.child_overflow")
        .filter_map(|d| d.subject_id.as_deref())
        .collect();
    assert_eq!(overflow, vec!["past"], "{:?}", codes(&report));
}

fn table_in(container_open: &str, container_close: &str) -> Document {
    parse_doc(&format!(
        r##"{container_open}
        table id="t" x=(px)10 y=(px)10 w=(px)200 h=(px)60 {{
          column width=(px)200
          row {{
            cell {{ text id="cell.text" x=(px)0 y=(px)0 w=(px)100 h=(px)20 fill=(token)"color.ink" {{ span "Hi" }} }}
          }}
        }}
      {container_close}"##
    ))
}

#[test]
fn table_text_contrast_inside_translated_group_matches_page_root() {
    // White text on the white page warns at the page root and inside a
    // translated group alike.
    let root = contrast_report(&table_in("", ""));
    assert!(has_code(&root, "contrast.low") || has_code(&root, "contrast.invisible"));
    let grouped = contrast_report(&table_in(r#"group id="g" x=(px)120 y=(px)80 {"#, "}"));
    let low = |r: &ValidationReport| {
        r.diagnostics
            .iter()
            .filter(|d| d.code.starts_with("contrast."))
            .map(|d| d.code.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(low(&root), low(&grouped));
}

#[test]
fn table_text_contrast_inherits_unmodeled_group() {
    // A dark cell backdrop under white text is a pass at the page root. In a
    // rotated group the backdrop paint is not modeled, so the verdict is
    // indeterminate rather than a pass.
    let cell = |open: &str, close: &str| {
        parse_doc(&format!(
            r##"{open}
        table id="t" x=(px)10 y=(px)10 w=(px)200 h=(px)60 {{
          column width=(px)200
          row {{
            cell {{
              rect id="plate" x=(px)0 y=(px)0 w=(px)200 h=(px)40 fill=(token)"color.dark"
              text id="cell.text" x=(px)10 y=(px)10 w=(px)100 h=(px)20 fill=(token)"color.ink" {{ span "Hi" }}
            }}
          }}
        }}
      {close}"##
        ))
    };
    let root = contrast_report(&cell("", ""));
    assert!(
        !root
            .diagnostics
            .iter()
            .any(|d| d.code.starts_with("contrast.")),
        "{:?}",
        codes(&root)
    );
    let rotated = contrast_report(&cell(r#"group id="g" rotate=(deg)10 {"#, "}"));
    assert!(
        has_code(&rotated, "contrast.indeterminate_backdrop"),
        "{:?}",
        codes(&rotated)
    );
}

fn overflow_ids(report: &ValidationReport) -> Vec<&str> {
    report
        .diagnostics
        .iter()
        .filter(|d| d.code == "frame.child_overflow")
        .filter_map(|d| d.subject_id.as_deref())
        .collect()
}

#[test]
fn off_canvas_reads_rect_through_frame_offset() {
    // Frame-local (200, 10) moves to page x 350; a 100-wide rect ends at 450.
    let doc = parse_doc(
        r##"frame id="f" x=(px)150 y=(px)0 w=(px)250 h=(px)300 clip=#false {
        rect id="r" x=(px)200 y=(px)10 w=(px)100 h=(px)50 fill=(token)"color.dark"
      }"##,
    );
    let report = validate(&doc);
    assert_eq!(off_canvas_ids(&report), vec!["r"], "{:?}", codes(&report));
}

#[test]
fn off_canvas_accepts_rect_moved_back_on_page_by_frame() {
    // Frame-local x -40 is page x 60.
    let doc = parse_doc(
        r##"frame id="f" x=(px)100 y=(px)0 w=(px)200 h=(px)200 clip=#false {
        rect id="r" x=(px)-40 y=(px)10 w=(px)50 h=(px)50 fill=(token)"color.dark"
      }"##,
    );
    let report = validate(&doc);
    assert!(off_canvas_ids(&report).is_empty(), "{:?}", codes(&report));
}

#[test]
fn child_overflow_reads_frame_children_frame_local() {
    // f2 sits at page (100, 100, 100, 100). `fits` is page 110..150, `past`
    // is page 180..240.
    let doc = parse_doc(
        r##"frame id="f1" x=(px)50 y=(px)50 w=(px)300 h=(px)200 {
        frame id="f2" x=(px)50 y=(px)50 w=(px)100 h=(px)100 {
          rect id="fits" x=(px)10 y=(px)10 w=(px)40 h=(px)40 fill=(token)"color.dark"
          rect id="past" x=(px)80 y=(px)10 w=(px)60 h=(px)40 fill=(token)"color.dark"
        }
      }"##,
    );
    let report = validate(&doc);
    assert_eq!(overflow_ids(&report), vec!["past"], "{:?}", codes(&report));
}

#[test]
fn table_text_contrast_inside_frame_matches_page_root() {
    let contrast = |r: &ValidationReport| {
        r.diagnostics
            .iter()
            .filter(|d| d.code.starts_with("contrast."))
            .map(|d| d.code.clone())
            .collect::<Vec<_>>()
    };
    let root = contrast_report(&table_in("", ""));
    let framed = contrast_report(&table_in(
        r#"frame id="f" x=(px)120 y=(px)80 w=(px)250 h=(px)200 {"#,
        "}",
    ));
    assert!(!contrast(&root).is_empty());
    assert_eq!(contrast(&root), contrast(&framed));
}

#[test]
fn contrast_reads_a_framed_backdrop_frame_local() {
    // The dark plate and the white text share the frame's space: the plate
    // covers the text, so the pair passes. Authored in page space the plate
    // misses the text and the text sits on the white page.
    let plate = |x: i32| {
        parse_doc(&format!(
            r##"frame id="f" x=(px)200 y=(px)150 w=(px)180 h=(px)120 {{
        rect id="plate" x=(px){x} y=(px)0 w=(px)180 h=(px)60 fill=(token)"color.dark"
        text id="t" x=(px)10 y=(px)10 w=(px)100 h=(px)20 fill=(token)"color.ink" {{ span "Hi" }}
      }}"##
        ))
    };
    let local = contrast_report(&plate(0));
    assert!(
        !local
            .diagnostics
            .iter()
            .any(|d| d.code.starts_with("contrast.")),
        "{:?}",
        codes(&local)
    );
    let missed = contrast_report(&plate(-200));
    assert!(
        has_code(&missed, "contrast.low") || has_code(&missed, "contrast.invisible"),
        "{:?}",
        codes(&missed)
    );
}
