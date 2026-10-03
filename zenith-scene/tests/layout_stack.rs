//! Auto-layout `row` / `column` frames: justify × align, gap and padding,
//! fill distribution with min/max, hug sizing, wrapping, nesting, and
//! out-of-flow children.

mod common;
use common::*;
use std::collections::BTreeMap;
use zenith_scene::{LayoutBox, layout_boxes};

/// A one-page document with the color tokens the fixtures use.
fn doc(children: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.ls" name="LS"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
    token id="color.w" type="color" value="#ffffff"
    token id="space.max" type="dimension" value=(px)50
  }}
  styles {{}}
  document id="doc.ls" title="LS" {{
    page id="p" w=(px)800 h=(px)600 {{
{children}
    }}
  }}
}}
"##
    )
}

fn boxes(src: &str) -> BTreeMap<String, LayoutBox> {
    layout_boxes(&parse(src), 0, &default_provider())
}

fn b(boxes: &BTreeMap<String, LayoutBox>, id: &str) -> (f64, f64, f64, f64) {
    let lb = boxes
        .get(id)
        .unwrap_or_else(|| panic!("no box for {id}: {boxes:?}"));
    (lb.x, lb.y, lb.w, lb.h)
}

fn codes(result: &CompileResult) -> Vec<String> {
    result.diagnostics.iter().map(|d| d.code.clone()).collect()
}

#[test]
fn row_justify_align_matrix() {
    let cases = [
        ("start", "start", (10.0, 20.0), (60.0, 20.0)),
        ("center", "center", (55.0, 60.0), (105.0, 55.0)),
        ("end", "end", (100.0, 100.0), (150.0, 90.0)),
        ("space-between", "stretch", (10.0, 20.0), (150.0, 20.0)),
        ("start", "end", (10.0, 100.0), (60.0, 90.0)),
        ("end", "center", (100.0, 60.0), (150.0, 55.0)),
    ];
    for (justify, align, a, bb) in cases {
        let src = doc(&format!(
            r#"      frame id="f" x=(px)10 y=(px)20 w=(px)200 h=(px)100 layout="row" gap=(px)10 justify="{justify}" align="{align}" {{
        rect id="a" w=(px)40 h=(px)20 fill=(token)"color.k"
        rect id="b" w=(px)60 h=(px)30 fill=(token)"color.k"
      }}"#
        ));
        let m = boxes(&src);
        assert_eq!(b(&m, "a"), (a.0, a.1, 40.0, 20.0), "{justify}/{align} a");
        assert_eq!(b(&m, "b"), (bb.0, bb.1, 60.0, 30.0), "{justify}/{align} b");
        let result = compile(&parse(&src), &default_provider());
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(
            fill_rects(&result),
            vec![(a.0, a.1, 40.0, 20.0), (bb.0, bb.1, 60.0, 30.0)]
        );
    }
}

#[test]
fn column_justify_align_matrix() {
    let cases = [
        ("start", "start", (10.0, 20.0), (10.0, 50.0)),
        ("center", "center", (90.0, 85.0), (80.0, 115.0)),
        ("end", "end", (170.0, 150.0), (150.0, 180.0)),
        ("space-between", "start", (10.0, 20.0), (10.0, 180.0)),
    ];
    for (justify, align, a, bb) in cases {
        let src = doc(&format!(
            r#"      frame id="f" x=(px)10 y=(px)20 w=(px)200 h=(px)200 layout="column" gap=(px)10 justify="{justify}" align="{align}" {{
        rect id="a" w=(px)40 h=(px)20 fill=(token)"color.k"
        rect id="b" w=(px)60 h=(px)40 fill=(token)"color.k"
      }}"#
        ));
        let m = boxes(&src);
        assert_eq!(b(&m, "a"), (a.0, a.1, 40.0, 20.0), "{justify}/{align} a");
        assert_eq!(b(&m, "b"), (bb.0, bb.1, 60.0, 40.0), "{justify}/{align} b");
    }
}

#[test]
fn stretch_fills_the_cross_axis() {
    let m = boxes(&doc(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)300 h=(px)80 layout="row" padding=(px)5 {
        rect id="a" w=(px)40 fill=(token)"color.k"
      }"#,
    ));
    assert_eq!(b(&m, "a"), (5.0, 5.0, 40.0, 70.0));
}

#[test]
fn padding_sides_and_gap_size_a_hugging_column() {
    let m = boxes(&doc(
        r#"      frame id="f" x=(px)10 y=(px)10 w=(px)100 layout="column" gap=(px)4 padding-top=(px)5 padding-left=(px)7 padding-right=(px)3 padding-bottom=(px)11 {
        rect id="a" h=(px)20 fill=(token)"color.k"
        rect id="b" h=(px)30 fill=(token)"color.k"
      }"#,
    ));
    assert_eq!(b(&m, "f"), (10.0, 10.0, 100.0, 70.0));
    assert_eq!(b(&m, "a"), (17.0, 15.0, 90.0, 20.0));
    assert_eq!(b(&m, "b"), (17.0, 39.0, 90.0, 30.0));
}

#[test]
fn fill_splits_free_space_with_max_and_min() {
    let m = boxes(&doc(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)300 h=(px)50 layout="row" {
        rect id="a" w="fill" max-w=(token)"space.max" fill=(token)"color.k"
        rect id="b" w="fill" fill=(token)"color.k"
        rect id="c" w=(px)100 fill=(token)"color.k"
      }"#,
    ));
    assert_eq!(b(&m, "a"), (0.0, 0.0, 50.0, 50.0));
    assert_eq!(b(&m, "b"), (50.0, 0.0, 150.0, 50.0));
    assert_eq!(b(&m, "c"), (200.0, 0.0, 100.0, 50.0));

    let m = boxes(&doc(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)50 layout="row" {
        rect id="a" w="fill" min-w=(px)180 fill=(token)"color.k"
        rect id="b" w="fill" fill=(token)"color.k"
      }"#,
    ));
    assert_eq!(b(&m, "a").2, 180.0);
    assert_eq!(b(&m, "b"), (180.0, 0.0, 20.0, 50.0));
}

#[test]
fn fill_in_a_hugging_parent_hugs_and_advises() {
    let src = doc(r#"      frame id="f" x=(px)0 y=(px)0 layout="row" {
        text id="t" w="fill" font-size=(px)20 fill=(token)"color.k" {
          span "Hug"
        }
      }"#);
    let result = compile(&parse(&src), &default_provider());
    let advisories: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "layout.fill_in_hug_parent")
        .collect();
    assert_eq!(advisories.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(advisories[0].subject_id.as_deref(), Some("t"));
    let m = boxes(&src);
    let (_, _, w, h) = b(&m, "t");
    assert!(w > 0.0 && w < 100.0, "hug width {w}");
    assert_eq!(b(&m, "f"), (0.0, 0.0, w, h));
}

#[test]
fn wrap_breaks_lines_and_stacks_them() {
    let m = boxes(&doc(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)100 layout="row" wrap=#true gap=(px)10 wrap-gap=(px)5 {
        rect id="a" w=(px)40 h=(px)20 fill=(token)"color.k"
        rect id="b" w=(px)40 h=(px)20 fill=(token)"color.k"
        rect id="c" w=(px)40 h=(px)20 fill=(token)"color.k"
        rect id="d" w=(px)40 h=(px)30 fill=(token)"color.k"
      }"#,
    ));
    assert_eq!(b(&m, "a"), (0.0, 0.0, 40.0, 20.0));
    assert_eq!(b(&m, "b"), (50.0, 0.0, 40.0, 20.0));
    assert_eq!(b(&m, "c"), (0.0, 25.0, 40.0, 20.0));
    assert_eq!(b(&m, "d"), (50.0, 25.0, 40.0, 30.0));
    assert_eq!(b(&m, "f"), (0.0, 0.0, 100.0, 55.0));
}

#[test]
fn nested_hug_frame_inside_a_fill_frame() {
    let m = boxes(&doc(
        r#"      frame id="outer" x=(px)0 y=(px)0 w=(px)400 h=(px)200 layout="row" gap=(px)20 {
        frame id="col" w="fill" layout="column" align="start" {
          frame id="inner" layout="row" gap=(px)10 {
            rect id="r1" w=(px)30 h=(px)10 fill=(token)"color.k"
            rect id="r2" w=(px)50 h=(px)10 fill=(token)"color.k"
          }
        }
        rect id="side" w=(px)100 fill=(token)"color.k"
      }"#,
    ));
    assert_eq!(b(&m, "col"), (0.0, 0.0, 280.0, 200.0));
    assert_eq!(b(&m, "inner"), (0.0, 0.0, 90.0, 10.0));
    assert_eq!(b(&m, "r2"), (40.0, 0.0, 50.0, 10.0));
    assert_eq!(b(&m, "side"), (300.0, 0.0, 100.0, 200.0));
}

#[test]
fn absolute_children_count_from_the_frame_top_left() {
    let src = doc(
        r#"      frame id="f" x=(px)50 y=(px)60 w=(px)200 h=(px)100 layout="column" {
        rect id="a" h=(px)20 fill=(token)"color.k"
        rect id="pin" x=(px)5 y=(px)6 w=(px)10 h=(px)10 position="absolute" fill=(token)"color.k"
        line id="rule" x1=(px)0 y1=(px)0 x2=(px)10 y2=(px)0 stroke=(token)"color.k"
      }"#,
    );
    let result = compile(&parse(&src), &default_provider());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let rects = fill_rects(&result);
    assert!(rects.contains(&(50.0, 60.0, 200.0, 20.0)), "{rects:?}");
    assert!(rects.contains(&(55.0, 66.0, 10.0, 10.0)), "{rects:?}");
    let line = result.scene.commands.iter().find_map(|c| match c {
        SceneCommand::StrokeLine { x1, y1, x2, y2, .. } => Some((*x1, *y1, *x2, *y2)),
        _ => None,
    });
    assert_eq!(line, Some((50.0, 60.0, 60.0, 60.0)));
}

#[test]
fn group_child_moves_and_keeps_local_children() {
    let src = doc(
        r#"      frame id="f" x=(px)10 y=(px)10 w=(px)200 layout="column" gap=(px)5 align="start" {
        group id="g" {
          rect id="gr" x=(px)5 y=(px)5 w=(px)20 h=(px)20 fill=(token)"color.k"
        }
        rect id="after" w=(px)10 h=(px)10 fill=(token)"color.k"
      }"#,
    );
    let m = boxes(&src);
    assert_eq!(b(&m, "g"), (10.0, 10.0, 25.0, 25.0));
    assert_eq!(b(&m, "after"), (10.0, 40.0, 10.0, 10.0));
    let result = compile(&parse(&src), &default_provider());
    assert!(fill_rects(&result).contains(&(15.0, 15.0, 20.0, 20.0)));
}

#[test]
fn unsized_rect_is_an_error() {
    let src = doc(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)50 layout="row" {
        rect id="r" fill=(token)"color.k"
      }"#,
    );
    let result = compile(&parse(&src), &default_provider());
    let d: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "layout.unsized_child")
        .collect();
    assert_eq!(d.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(d[0].severity, zenith_core::Severity::Error);
    assert_eq!(d[0].subject_id.as_deref(), Some("r"));
    assert!(d[0].message.contains("set w"), "{}", d[0].message);
}

#[test]
fn overflowing_children_advise() {
    let src = doc(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)100 h=(px)50 layout="row" gap=(px)10 {
        rect id="a" w=(px)60 fill=(token)"color.k"
        rect id="b" w=(px)60 fill=(token)"color.k"
      }"#,
    );
    let result = compile(&parse(&src), &default_provider());
    assert!(
        codes(&result).contains(&"layout.child_overflow".to_owned()),
        "{:?}",
        result.diagnostics
    );
}

#[test]
fn token_min_above_max_is_a_conflict() {
    let src = doc(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)300 h=(px)50 layout="row" {
        rect id="a" w="fill" min-w=(px)80 max-w=(token)"space.max" fill=(token)"color.k"
      }"#,
    );
    let result = compile(&parse(&src), &default_provider());
    assert!(
        codes(&result).contains(&"layout.conflicting_size".to_owned()),
        "{:?}",
        result.diagnostics
    );
    assert_eq!(b(&boxes(&src), "a").2, 80.0, "min wins");
}

#[test]
fn hugging_root_frame_sizes_to_children() {
    let src = doc(
        r#"      frame id="f" x=(px)20 y=(px)30 layout="row" gap=(px)6 padding=(px)4 fill=(token)"color.w" {
        rect id="a" w=(px)40 h=(px)10 fill=(token)"color.k"
        rect id="b" w=(px)20 h=(px)30 fill=(token)"color.k"
      }"#,
    );
    let m = boxes(&src);
    assert_eq!(b(&m, "f"), (20.0, 30.0, 74.0, 38.0));
    let result = compile(&parse(&src), &default_provider());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(fill_rects(&result).first(), Some(&(20.0, 30.0, 74.0, 38.0)));
}

#[test]
fn nested_hugging_frame_keeps_fill_as_hug_and_advises_once() {
    let src = doc(
        r#"      frame id="outer" x=(px)0 y=(px)0 w=(px)300 layout="column" align="start" {
        frame id="inner" layout="row" gap=(px)4 {
          rect id="a" w=(px)30 h=(px)10 fill=(token)"color.k"
          group id="g" w="fill" {
            rect id="gr" x=(px)0 y=(px)0 w=(px)20 h=(px)10 fill=(token)"color.k"
          }
        }
      }"#,
    );
    let result = compile(&parse(&src), &default_provider());
    let advisories: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "layout.fill_in_hug_parent")
        .collect();
    assert_eq!(advisories.len(), 1, "{:?}", result.diagnostics);
    assert_eq!(advisories[0].subject_id.as_deref(), Some("g"));
    let m = boxes(&src);
    assert_eq!(b(&m, "inner"), (0.0, 0.0, 54.0, 10.0));
    assert_eq!(b(&m, "g"), (34.0, 0.0, 20.0, 10.0));
}
