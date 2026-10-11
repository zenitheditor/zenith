//! Per-child `margin*` in `row` / `column` frames: main-axis offsets, cross
//! alignment and stretch, hug sizes, `space-between`, `fill` shares, and
//! the cases where margin is inert (absolute children, grid children).

mod common;
use common::parse;
#[path = "common/fill_rects.rs"]
mod fill_rects;
use fill_rects::fill_rects;
use std::collections::BTreeMap;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::{LayoutBox, layout_boxes};

fn doc(children: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.lm" name="LM"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
    token id="space.lg" type="dimension" value=(px)40
  }}
  styles {{}}
  document id="doc.lm" title="LM" {{
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

#[test]
fn column_margin_top_moves_one_child_and_later_siblings() {
    let src = doc(
        r#"      frame id="f" x=(px)10 y=(px)20 w=(px)200 h=(px)300 layout="column" gap=(px)10 align="start" {
        rect id="a" w=(px)50 h=(px)20 fill=(token)"color.k"
        rect id="b" w=(px)50 h=(px)30 margin-top=(px)40 margin-bottom=(px)5 fill=(token)"color.k"
        rect id="c" w=(px)50 h=(px)10 fill=(token)"color.k"
      }"#,
    );
    let m = boxes(&src);
    assert_eq!(b(&m, "a"), (10.0, 20.0, 50.0, 20.0));
    assert_eq!(b(&m, "b"), (10.0, 90.0, 50.0, 30.0));
    assert_eq!(b(&m, "c"), (10.0, 135.0, 50.0, 10.0));
    let result = compile(&parse(&src), &default_provider());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
}

#[test]
fn margin_token_resolves() {
    let src = doc(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)300 layout="column" align="start" {
        rect id="a" w=(px)50 h=(px)20 margin-top=(token)"space.lg" fill=(token)"color.k"
      }"#,
    );
    assert_eq!(b(&boxes(&src), "a"), (0.0, 40.0, 50.0, 20.0));
}

#[test]
fn row_margin_left_adds_to_gap() {
    let src = doc(
        r#"      frame id="f" x=(px)10 y=(px)20 w=(px)300 h=(px)50 layout="row" gap=(px)10 align="start" {
        rect id="a" w=(px)40 h=(px)20 fill=(token)"color.k"
        rect id="b" w=(px)40 h=(px)20 margin-left=(px)25 fill=(token)"color.k"
        rect id="c" w=(px)40 h=(px)20 fill=(token)"color.k"
      }"#,
    );
    let m = boxes(&src);
    assert_eq!(b(&m, "a").0, 10.0);
    assert_eq!(b(&m, "b").0, 85.0);
    assert_eq!(b(&m, "c").0, 135.0);
}

#[test]
fn cross_stretch_subtracts_cross_margins() {
    let src = doc(
        r#"      frame id="f" x=(px)10 y=(px)20 w=(px)200 h=(px)300 layout="column" {
        rect id="a" h=(px)20 margin-x=(px)15 fill=(token)"color.k"
      }
      frame id="r" x=(px)300 y=(px)20 w=(px)200 h=(px)100 layout="row" {
        rect id="b" w=(px)40 margin-y=(px)10 fill=(token)"color.k"
      }"#,
    );
    let m = boxes(&src);
    assert_eq!(b(&m, "a"), (25.0, 20.0, 170.0, 20.0));
    assert_eq!(b(&m, "b"), (300.0, 30.0, 40.0, 80.0));
}

#[test]
fn cross_center_and_end_align_the_margin_box() {
    let src = doc(
        r#"      frame id="f" x=(px)10 y=(px)20 w=(px)200 h=(px)300 layout="column" align="center" {
        rect id="a" w=(px)40 h=(px)20 margin-left=(px)20 fill=(token)"color.k"
      }
      frame id="g" x=(px)300 y=(px)20 w=(px)200 h=(px)300 layout="column" align="end" {
        rect id="b" w=(px)40 h=(px)20 margin-right=(px)30 fill=(token)"color.k"
      }"#,
    );
    let m = boxes(&src);
    assert_eq!(b(&m, "a").0, 100.0);
    assert_eq!(b(&m, "b").0, 430.0);
}

#[test]
fn hugging_frame_includes_child_margins() {
    let src = doc(
        r#"      frame id="f" x=(px)10 y=(px)20 w="hug" h="hug" layout="column" gap=(px)10 {
        rect id="a" w=(px)40 h=(px)20 margin=(px)10 fill=(token)"color.k"
        rect id="b" w=(px)40 h=(px)20 fill=(token)"color.k"
      }"#,
    );
    let m = boxes(&src);
    assert_eq!(b(&m, "f"), (10.0, 20.0, 60.0, 70.0));
    assert_eq!(b(&m, "a"), (20.0, 30.0, 40.0, 20.0));
    assert_eq!(b(&m, "b"), (10.0, 70.0, 40.0, 20.0));
}

#[test]
fn space_between_treats_margin_boxes_as_items() {
    let src = doc(
        r#"      frame id="f" x=(px)10 y=(px)20 w=(px)200 h=(px)50 layout="row" justify="space-between" align="start" {
        rect id="a" w=(px)40 h=(px)20 margin-right=(px)10 fill=(token)"color.k"
        rect id="b" w=(px)40 h=(px)20 margin-left=(px)20 fill=(token)"color.k"
      }"#,
    );
    let m = boxes(&src);
    assert_eq!(b(&m, "a").0, 10.0);
    assert_eq!(b(&m, "b").0, 170.0);
}

#[test]
fn justify_end_keeps_trailing_margin() {
    let src = doc(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)50 layout="row" justify="end" align="start" {
        rect id="a" w=(px)40 h=(px)20 margin-right=(px)30 fill=(token)"color.k"
      }"#,
    );
    assert_eq!(b(&boxes(&src), "a").0, 130.0);
}

#[test]
fn fill_shares_space_after_margins() {
    let src = doc(
        r#"      frame id="f" x=(px)10 y=(px)20 w=(px)200 h=(px)50 layout="row" align="start" {
        rect id="a" w="fill" h=(px)20 fill=(token)"color.k"
        rect id="b" w="fill" h=(px)20 margin-left=(px)20 fill=(token)"color.k"
      }"#,
    );
    let m = boxes(&src);
    assert_eq!(b(&m, "a"), (10.0, 20.0, 90.0, 20.0));
    assert_eq!(b(&m, "b"), (120.0, 20.0, 90.0, 20.0));
}

#[test]
fn min_max_clamp_the_border_box() {
    let src = doc(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)50 layout="row" align="start" {
        rect id="a" w="fill" max-w=(px)100 h=(px)20 margin-left=(px)50 fill=(token)"color.k"
      }"#,
    );
    assert_eq!(b(&boxes(&src), "a"), (50.0, 0.0, 100.0, 20.0));
}

#[test]
fn absolute_child_ignores_margin() {
    let child = |margin: &str| {
        doc(&format!(
            r#"      frame id="f" x=(px)10 y=(px)20 w=(px)200 h=(px)300 layout="column" align="start" {{
        rect id="a" w=(px)40 h=(px)20 fill=(token)"color.k"
        rect id="abs" x=(px)5 y=(px)5 w=(px)40 h=(px)20 position="absolute" {margin} fill=(token)"color.k"
      }}"#
        ))
    };
    let plain = compile(&parse(&child("")), &default_provider());
    let margined = compile(&parse(&child("margin-top=(px)40")), &default_provider());
    assert_eq!(fill_rects(&margined), fill_rects(&plain));
    assert_eq!(
        fill_rects(&margined),
        vec![(10.0, 20.0, 40.0, 20.0), (15.0, 25.0, 40.0, 20.0)]
    );
}

#[test]
fn grid_child_ignores_margin() {
    let plain = doc(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)100 layout="grid" columns=2 {
        rect id="a" fill=(token)"color.k"
        rect id="b" fill=(token)"color.k"
      }"#,
    );
    let margined = doc(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)100 layout="grid" columns=2 {
        rect id="a" margin=(px)10 fill=(token)"color.k"
        rect id="b" fill=(token)"color.k"
      }"#,
    );
    assert_eq!(boxes(&plain), boxes(&margined));
}
