//! Area hits: the boxes a convex page region (a marquee) meets, exact for
//! vector shapes, through rotation and clips.

use std::collections::BTreeMap;

use zenith_core::{KdlAdapter, KdlSource, default_provider};
use zenith_scene::{CompiledBox, DocumentPrep, PageCompiler, hit_region};

const HEAD: &str = r##"zenith version=1 {
  project id="proj.region" name="Region"
  tokens format="zenith-token-v1" {
    token id="c.k" type="color" value="#000000"
    token id="s.w" type="dimension" value=(px)4
  }
  styles {}
"##;

fn boxes_of(children: &str) -> BTreeMap<String, CompiledBox> {
    let src = format!(
        r#"{HEAD}  document id="doc.region" title="Region" {{
    page id="p" w=(px)400 h=(px)400 {{
{children}
    }}
  }}
}}
"#
    );
    let doc = KdlAdapter.parse(src.as_bytes()).expect("fixture parses");
    let fonts = default_provider();
    let prep = DocumentPrep::new(&doc, None, None);
    PageCompiler::new(&prep, &fonts)
        .compile_page_with_boxes(0, false)
        .1
}

/// The axis-aligned rectangle `(x0, y0)`–`(x1, y1)` as a region.
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> [(f64, f64); 4] {
    [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

fn ids(boxes: &BTreeMap<String, CompiledBox>, region: &[(f64, f64)]) -> Vec<String> {
    hit_region(boxes, region)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

#[test]
fn boxes_meet_by_their_drawn_box_topmost_first() {
    let boxes = boxes_of(
        r#"      rect id="a" x=(px)10 y=(px)10 w=(px)50 h=(px)50 fill=(token)"c.k"
      rect id="b" x=(px)40 y=(px)40 w=(px)50 h=(px)50 fill=(token)"c.k"
      rect id="hid" x=(px)40 y=(px)40 w=(px)50 h=(px)50 fill=(token)"c.k" visible=#false"#,
    );
    assert_eq!(ids(&boxes, &rect(45.0, 45.0, 55.0, 55.0)), ["b", "a"]);
    assert_eq!(ids(&boxes, &rect(0.0, 0.0, 20.0, 20.0)), ["a"]);
    assert!(ids(&boxes, &rect(200.0, 200.0, 300.0, 300.0)).is_empty());
    let a = boxes.get("a").expect("a");
    assert!(a.within_region(&rect(0.0, 0.0, 100.0, 100.0)));
    assert!(!a.within_region(&rect(20.0, 0.0, 100.0, 100.0)));
    assert!(a.holds_region(&rect(20.0, 20.0, 30.0, 30.0)));
    assert!(!a.holds_region(&rect(20.0, 20.0, 70.0, 30.0)));
    assert!(!a.holds_region(&[]));
    // A region with no area meets nothing.
    assert!(ids(&boxes, &rect(45.0, 45.0, 45.0, 55.0)).is_empty());
}

#[test]
fn rotated_boxes_meet_by_their_turned_corners() {
    // A 100 × 20 bar turned 45° about its centre (200, 200): its bounds
    // cover (200 ± 42, 200 ± 42), but the corner of the bounds is empty.
    let boxes = boxes_of(
        r#"      rect id="bar" x=(px)150 y=(px)190 w=(px)100 h=(px)20 rotate=45 fill=(token)"c.k""#,
    );
    assert!(ids(&boxes, &rect(160.0, 160.0, 170.0, 170.0)).is_empty());
    assert_eq!(ids(&boxes, &rect(195.0, 195.0, 205.0, 205.0)), ["bar"]);
}

#[test]
fn vector_shapes_meet_by_their_paint() {
    // A stroke-only diagonal line and a triangle with an empty corner of
    // its bounds.
    let boxes = boxes_of(
        r#"      line id="diag" x1=(px)0 y1=(px)0 x2=(px)100 y2=(px)100 stroke=(token)"c.k" stroke-width=(token)"s.w"
      polygon id="tri" fill=(token)"c.k" {
        point x=(px)200 y=(px)200
        point x=(px)300 y=(px)200
        point x=(px)200 y=(px)300
      }"#,
    );
    // Inside the line's bounds, off its band.
    assert!(ids(&boxes, &rect(70.0, 10.0, 90.0, 30.0)).is_empty());
    // Within the 2 px half width of the band.
    assert_eq!(ids(&boxes, &rect(51.5, 40.0, 60.0, 50.0)), ["diag"]);
    // The empty corner of the triangle's bounds.
    assert!(ids(&boxes, &rect(280.0, 280.0, 299.0, 299.0)).is_empty());
    // A region wholly inside the fill.
    assert_eq!(ids(&boxes, &rect(210.0, 210.0, 220.0, 220.0)), ["tri"]);
}

#[test]
fn clips_cut_the_region() {
    let boxes = boxes_of(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)100 h=(px)100 clip=#true {
        rect id="big" x=(px)50 y=(px)50 w=(px)200 h=(px)200 fill=(token)"c.k"
      }"#,
    );
    // Over the clipped-away part of `big`: only the frame box is out there
    // too, so nothing.
    assert!(ids(&boxes, &rect(150.0, 150.0, 160.0, 160.0)).is_empty());
    assert_eq!(ids(&boxes, &rect(60.0, 60.0, 70.0, 70.0)), ["big", "f"]);
}
