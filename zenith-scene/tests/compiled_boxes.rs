//! `PageCompiler::compiled_boxes`: the final box of every compiled node —
//! anchored nodes, text without `h`, stroked lines and connectors, table
//! cell children, master projections, and laid-out children.

mod common;
use common::*;
use std::collections::BTreeMap;
use zenith_scene::{CompiledBox, DocumentPrep, PageCompiler};

const DOC: &str = r##"zenith version=1 {
  project id="proj.cb" name="CB"
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
    token id="size.stroke" type="dimension" value=(px)4
  }
  styles {}
  masters {
    master id="m" {
      field id="mf" type="page-number" x=(px)700 y=(px)10 w=(px)80 h=(px)30 fill=(token)"color.k"
      text id="mt" x=(px)10 y=(px)560 w=(px)100 h=(px)20 fill=(token)"color.k" {
        span "Footer"
      }
    }
  }
  document id="doc.cb" title="CB" {
    page id="p" w=(px)800 h=(px)600 master="m" {
      rect id="s" x=(px)100 y=(px)100 w=(px)200 h=(px)100 fill=(token)"color.k"
      rect id="anch" anchor="bottom-right" anchor-sibling="s" w=(px)40 h=(px)20 fill=(token)"color.k"
      text id="t" x=(px)10 y=(px)300 w=(px)200 font-size=(px)20 fill=(token)"color.k" {
        span "Hello"
      }
      line id="l" x1=(px)10 y1=(px)400 x2=(px)110 y2=(px)400 stroke=(token)"color.k" stroke-width=(token)"size.stroke"
      rect id="p1" x=(px)400 y=(px)100 w=(px)50 h=(px)50 fill=(token)"color.k"
      rect id="p2" x=(px)600 y=(px)100 w=(px)50 h=(px)50 fill=(token)"color.k"
      connector id="c" from="p1" to="p2" stroke=(token)"color.k"
      table id="tb" x=(px)20 y=(px)450 w=(px)200 h=(px)40 cell-padding=(px)0 gap=(px)0 {
        column
        row { cell { rect id="cellr" x=(px)5 y=(px)5 w=(px)10 h=(px)10 fill=(token)"color.k" } }
      }
      group id="g" x=(px)300 y=(px)300 {
        frame id="row" x=(px)10 y=(px)10 w=(px)200 h=(px)40 layout="row" gap=(px)10 align="start" {
          rect id="ra" w=(px)30 h=(px)20 fill=(token)"color.k"
          rect id="rb" w=(px)40 h=(px)25 fill=(token)"color.k"
        }
      }
    }
  }
}
"##;

fn boxes() -> BTreeMap<String, CompiledBox> {
    boxes_of(DOC)
}

fn boxes_of(src: &str) -> BTreeMap<String, CompiledBox> {
    let doc = parse(src);
    let prep = DocumentPrep::new(&doc, None, None);
    let fonts = default_provider();
    PageCompiler::new(&prep, &fonts).compiled_boxes(0)
}

fn b(m: &BTreeMap<String, CompiledBox>, id: &str) -> (f64, f64, f64, f64) {
    let lb = m
        .get(id)
        .unwrap_or_else(|| panic!("no box for {id}: {m:?}"))
        .rect;
    (lb.x, lb.y, lb.w, lb.h)
}

fn visual(m: &BTreeMap<String, CompiledBox>, id: &str) -> (f64, f64, f64, f64) {
    let v = m
        .get(id)
        .unwrap_or_else(|| panic!("no box for {id}: {m:?}"))
        .visual;
    (v.x, v.y, v.w, v.h)
}

fn round(t: (f64, f64, f64, f64)) -> (f64, f64, f64, f64) {
    let r = |v: f64| (v * 1000.0).round() / 1000.0;
    (r(t.0), r(t.1), r(t.2), r(t.3))
}

#[test]
fn anchored_rect_reports_its_anchor_derived_origin() {
    assert_eq!(b(&boxes(), "anch"), (260.0, 180.0, 40.0, 20.0));
}

#[test]
fn text_without_h_reports_its_measured_height() {
    let (x, y, w, h) = b(&boxes(), "t");
    assert_eq!((x, y, w), (10.0, 300.0, 200.0));
    assert!(h > 15.0 && h < 60.0, "measured height {h}");
}

#[test]
fn line_reports_stroked_bounds() {
    assert_eq!(b(&boxes(), "l"), (8.0, 398.0, 104.0, 4.0));
}

#[test]
fn connector_reports_its_route_bounds() {
    let (x, y, w, h) = b(&boxes(), "c");
    assert!(x >= 440.0 && x + w <= 610.0, "{x} {w}");
    assert!(w >= 140.0, "{w}");
    assert!(y <= 125.0 && y + h >= 125.0, "{y} {h}");
}

#[test]
fn table_cell_child_reports_its_cell_position() {
    assert_eq!(b(&boxes(), "cellr"), (25.0, 455.0, 10.0, 10.0));
}

#[test]
fn master_content_reports_under_expanded_ids() {
    let m = boxes();
    assert_eq!(b(&m, "p/mf"), (700.0, 10.0, 80.0, 30.0));
    assert_eq!(b(&m, "p/mt"), (10.0, 560.0, 100.0, 20.0));
}

#[test]
fn laid_out_children_in_a_group_report_page_absolute_boxes() {
    let m = boxes();
    assert_eq!(b(&m, "row"), (310.0, 310.0, 200.0, 40.0));
    assert_eq!(b(&m, "ra"), (310.0, 310.0, 30.0, 20.0));
    assert_eq!(b(&m, "rb"), (350.0, 310.0, 40.0, 25.0));
    // A group without w/h takes the bounds of what it paints.
    assert_eq!(b(&m, "g"), (310.0, 310.0, 80.0, 25.0));
}

#[test]
fn out_of_range_page_has_no_boxes() {
    let doc = parse(DOC);
    let prep = DocumentPrep::new(&doc, None, None);
    let fonts = default_provider();
    assert!(
        PageCompiler::new(&prep, &fonts)
            .compiled_boxes(9)
            .is_empty()
    );
}

/// Rotation, glyph-only fallbacks, footnotes, and pattern instances.
const MORE: &str = r##"zenith version=1 {
  project id="proj.cb2" name="CB2"
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
  }
  styles {}
  document id="doc.cb2" title="CB2" {
    page id="p" w=(px)600 h=(px)900 margin-inner=(px)60 margin-outer=(px)60 margin-top=(px)80 margin-bottom=(px)80 {
      rect id="rot" x=(px)100 y=(px)100 w=(px)40 h=(px)20 rotate=(deg)90 fill=(token)"color.k"
      group id="spin" rotate=(deg)90 {
        rect id="inside" x=(px)300 y=(px)100 w=(px)40 h=(px)20 fill=(token)"color.k"
      }
      group id="words" {
        text id="word" x=(px)100 y=(px)300 w=(px)200 h=(px)40 font-size=(px)20 fill=(token)"color.k" {
          span "Ink" footnote-ref="fn.1"
        }
      }
      footnote id="fn.1" fill=(token)"color.k" { span "A note." }
      pattern id="pat" kind="grid" x=(px)100 y=(px)500 w=(px)100 h=(px)100 spacing=(px)50 {
        ellipse id="dot" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k"
      }
    }
  }
}
"##;

#[test]
fn rotated_rect_keeps_its_box_and_reports_rotated_bounds() {
    let m = boxes_of(MORE);
    assert_eq!(b(&m, "rot"), (100.0, 100.0, 40.0, 20.0));
    assert_eq!(m.get("rot").and_then(|c| c.rotate), Some(90.0));
    assert_eq!(round(visual(&m, "rot")), (110.0, 90.0, 20.0, 40.0));
}

#[test]
fn ancestor_rotation_moves_the_child_bounds() {
    let m = boxes_of(MORE);
    // The group turns about the center of its painted extent (320, 110),
    // which is the child's center: the child's footprint turns in place.
    assert_eq!(round(visual(&m, "inside")), (310.0, 90.0, 20.0, 40.0));
    assert_eq!(m.get("spin").and_then(|c| c.rotate), Some(90.0));
}

#[test]
fn glyph_only_group_takes_its_box_from_ink() {
    let m = boxes_of(MORE);
    let (x, y, w, h) = b(&m, "words");
    assert!((100.0..140.0).contains(&x), "{x}");
    assert!((300.0..340.0).contains(&y), "{y}");
    assert!(w > 10.0 && w < 200.0, "{w}");
    assert!(h > 5.0 && h < 40.0, "{h}");
}

#[test]
fn text_visual_bounds_are_its_ink() {
    let m = boxes_of(MORE);
    assert_eq!(b(&m, "word"), (100.0, 300.0, 200.0, 40.0));
    let (_, _, w, _) = visual(&m, "word");
    assert!(w < 200.0, "ink narrower than the box: {w}");
}

#[test]
fn footnote_records_its_slot_in_the_zone() {
    let m = boxes_of(MORE);
    let (x, y, w, h) = b(&m, "fn.1");
    assert_eq!((x, w), (60.0, 480.0));
    assert!(h > 0.0 && y > 700.0 && y + h <= 820.5, "{y} {h}");
}

#[test]
fn pattern_instances_record_under_indexed_ids() {
    let m = boxes_of(MORE);
    assert_eq!(b(&m, "pat"), (100.0, 500.0, 100.0, 100.0));
    assert_eq!(b(&m, "pat/0/dot"), (100.0, 500.0, 10.0, 10.0));
    assert_eq!(b(&m, "pat/1/dot"), (150.0, 500.0, 10.0, 10.0));
    assert_eq!(b(&m, "pat/3/dot"), (150.0, 550.0, 10.0, 10.0));
    assert!(!m.contains_key("dot"), "the probe records nothing");
}
