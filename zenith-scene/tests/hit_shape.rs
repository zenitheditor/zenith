//! Exact hit tests for lines, polygons, polylines, and paths: fills under
//! their fill rule, stroke bands, rotation, clips, and click slop.

use std::collections::BTreeMap;

use zenith_core::{KdlAdapter, KdlSource, default_provider};
use zenith_scene::{CompiledBox, DocumentPrep, PageCompiler, hit_test, hit_test_within};

const HEAD: &str = r##"zenith version=1 {
  project id="proj.shape" name="Shape"
  tokens format="zenith-token-v1" {
    token id="c.k" type="color" value="#000000"
  }
  styles {}
"##;

/// The boxes of a one-page 400 × 400 document whose page holds `children`.
fn boxes_of(children: &str) -> BTreeMap<String, CompiledBox> {
    let src = format!(
        r#"{HEAD}  document id="doc.shape" title="Shape" {{
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

fn hits(boxes: &BTreeMap<String, CompiledBox>, x: f64, y: f64) -> Vec<String> {
    hit_test(boxes, x, y)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

fn hits_within(boxes: &BTreeMap<String, CompiledBox>, x: f64, y: f64, tol: f64) -> Vec<String> {
    hit_test_within(boxes, x, y, tol)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

/// `true` when `(x, y)` lies in the axis-aligned `rect` summary of `id`.
fn in_bounds(boxes: &BTreeMap<String, CompiledBox>, id: &str, x: f64, y: f64) -> bool {
    let r = boxes.get(id).expect("box").rect;
    x >= r.x && x <= r.x + r.w && y >= r.y && y <= r.y + r.h
}

/// `(x, y)` turned `deg` degrees about `(cx, cy)`, as the renderer turns it.
fn turn(deg: f64, (cx, cy): (f64, f64), (x, y): (f64, f64)) -> (f64, f64) {
    let (sin, cos) = deg.to_radians().sin_cos();
    let (dx, dy) = (x - cx, y - cy);
    (cx + cos * dx - sin * dy, cy + sin * dx + cos * dy)
}

/// A U-shaped polygon: arms at x 100–150 and 250–300, the notch between
/// them open from y 100 down to 250.
const U_POINTS: &str = r#"        point x=(px)100 y=(px)50
        point x=(px)300 y=(px)50
        point x=(px)300 y=(px)250
        point x=(px)250 y=(px)250
        point x=(px)250 y=(px)100
        point x=(px)150 y=(px)100
        point x=(px)150 y=(px)250
        point x=(px)100 y=(px)250"#;

#[test]
fn concave_polygon_notch_misses() {
    let boxes = boxes_of(&format!(
        "      polygon id=\"u\" fill=(token)\"c.k\" {{\n{U_POINTS}\n      }}"
    ));
    assert!(in_bounds(&boxes, "u", 200.0, 200.0));
    assert!(hits(&boxes, 200.0, 200.0).is_empty());
    assert_eq!(hits(&boxes, 125.0, 200.0), vec!["u"]);
    assert_eq!(hits(&boxes, 200.0, 75.0), vec!["u"]);
    // An edge point is inside.
    assert_eq!(hits(&boxes, 150.0, 200.0), vec!["u"]);
}

#[test]
fn point_in_bounds_outside_a_triangle_misses() {
    let boxes = boxes_of(
        r#"      polygon id="tri" fill=(token)"c.k" {
        point x=(px)160 y=(px)40
        point x=(px)260 y=(px)170
        point x=(px)60 y=(px)170
      }"#,
    );
    assert!(in_bounds(&boxes, "tri", 70.0, 50.0));
    assert!(hits(&boxes, 70.0, 50.0).is_empty());
    assert!(hits(&boxes, 250.0, 60.0).is_empty());
    assert_eq!(hits(&boxes, 160.0, 120.0), vec!["tri"]);
}

#[test]
fn path_hole_misses_under_each_fill_rule() {
    let path = |id: &str, rule: &str, inner: &str| {
        format!(
            r#"      path id="{id}" fill=(token)"c.k" fill-rule="{rule}" {{
        subpath closed=#true {{
          anchor x=(px)100 y=(px)100
          anchor x=(px)300 y=(px)100
          anchor x=(px)300 y=(px)300
          anchor x=(px)100 y=(px)300
        }}
        subpath closed=#true {{
{inner}
        }}
      }}"#
        )
    };
    let same = r#"          anchor x=(px)150 y=(px)150
          anchor x=(px)250 y=(px)150
          anchor x=(px)250 y=(px)250
          anchor x=(px)150 y=(px)250"#;
    let reversed = r#"          anchor x=(px)150 y=(px)150
          anchor x=(px)150 y=(px)250
          anchor x=(px)250 y=(px)250
          anchor x=(px)250 y=(px)150"#;

    let even_odd = boxes_of(&path("eo", "evenodd", same));
    assert!(hits(&even_odd, 200.0, 200.0).is_empty());
    assert_eq!(hits(&even_odd, 120.0, 200.0), vec!["eo"]);

    let nonzero_hole = boxes_of(&path("nz", "nonzero", reversed));
    assert!(hits(&nonzero_hole, 200.0, 200.0).is_empty());
    assert_eq!(hits(&nonzero_hole, 120.0, 200.0), vec!["nz"]);

    // Same direction under nonzero: the inner ring adds, no hole.
    let nonzero_solid = boxes_of(&path("ns", "nonzero", same));
    assert_eq!(hits(&nonzero_solid, 200.0, 200.0), vec!["ns"]);
}

#[test]
fn curved_path_hits_inside_the_curve_only() {
    let boxes = boxes_of(
        r#"      path id="arc" closed=#true fill=(token)"c.k" {
        anchor x=(px)100 y=(px)100 out-x=(px)100 out-y=(px)300
        anchor x=(px)300 y=(px)100 in-x=(px)300 in-y=(px)300
        anchor x=(px)200 y=(px)50
      }"#,
    );
    // The cubic bulges down to y = 250 at x = 200.
    assert_eq!(hits(&boxes, 200.0, 245.0), vec!["arc"]);
    assert!(hits(&boxes, 200.0, 255.0).is_empty());
    assert!(in_bounds(&boxes, "arc", 105.0, 240.0));
    assert!(hits(&boxes, 105.0, 240.0).is_empty());
}

#[test]
fn open_polyline_hits_on_its_stroke_only() {
    let boxes = boxes_of(
        r#"      polyline id="zig" stroke=(token)"c.k" stroke-width=(px)4 {
        point x=(px)40 y=(px)160
        point x=(px)120 y=(px)60
        point x=(px)200 y=(px)160
        point x=(px)280 y=(px)60
      }"#,
    );
    // Midpoint of the first segment, and 1.5 px across it.
    assert_eq!(hits(&boxes, 80.0, 110.0), vec!["zig"]);
    let len = 80.0_f64.hypot(100.0);
    let (nx, ny) = (100.0 / len * 1.5, 80.0 / len * 1.5);
    assert_eq!(hits(&boxes, 80.0 + nx, 110.0 + ny), vec!["zig"]);
    // Between the segments, inside the bounds: no fill, so a miss.
    assert!(in_bounds(&boxes, "zig", 120.0, 140.0));
    assert!(hits(&boxes, 120.0, 140.0).is_empty());
    // The open end does not close back to the start: a point on the chord
    // from (280, 60) to (40, 160) misses.
    assert!(hits(&boxes, 220.0, 85.0).is_empty());
}

#[test]
fn stroke_only_polygon_misses_inside_and_filled_polygon_hits() {
    let boxes = boxes_of(&format!(
        "      polygon id=\"ring\" stroke=(token)\"c.k\" stroke-width=(px)6 {{\n{U_POINTS}\n      }}"
    ));
    assert!(hits(&boxes, 125.0, 200.0).is_empty());
    assert!(hits_within(&boxes, 125.0, 200.0, 4.0).is_empty());
    // On the outline, and the closing edge from (100, 250) to (100, 50).
    assert_eq!(hits(&boxes, 102.0, 150.0), vec!["ring"]);
    assert_eq!(hits(&boxes, 200.0, 52.0), vec!["ring"]);
    assert!(hits(&boxes, 106.0, 150.0).is_empty());
}

#[test]
fn diagonal_line_hits_on_its_segment_only() {
    let boxes = boxes_of(
        r#"      line id="d" x1=(px)0 y1=(px)0 x2=(px)200 y2=(px)200 stroke=(token)"c.k" stroke-width=(px)2"#,
    );
    assert!(in_bounds(&boxes, "d", 150.0, 20.0));
    assert!(hits(&boxes, 150.0, 20.0).is_empty());
    assert_eq!(hits(&boxes, 100.0, 100.0), vec!["d"]);
    // 0.7 px off the centerline across a 2 px stroke.
    assert_eq!(hits(&boxes, 100.5, 99.5), vec!["d"]);
    assert!(hits(&boxes, 102.0, 98.0).is_empty());
}

#[test]
fn rotated_polygon_hits_its_turned_shape() {
    // A right triangle, its bounds centred on (150, 150), turned 90°.
    let boxes = boxes_of(
        r#"      polygon id="tri" fill=(token)"c.k" rotate=(deg)90 {
        point x=(px)100 y=(px)100
        point x=(px)200 y=(px)100
        point x=(px)100 y=(px)200
      }"#,
    );
    // Turned, it covers x ≥ y in the bounds.
    assert_eq!(hits(&boxes, 190.0, 130.0), vec!["tri"]);
    assert!(hits(&boxes, 110.0, 180.0).is_empty());
}

#[test]
fn polygon_under_a_rotated_group_follows_the_world_transform() {
    let boxes = boxes_of(&format!(
        "      group id=\"g\" x=(px)0 y=(px)0 w=(px)400 h=(px)400 rotate=(deg)45 {{\n      polygon id=\"u\" fill=(token)\"c.k\" {{\n{U_POINTS}\n      }}\n      }}"
    ));
    let pivot = (200.0, 200.0);
    let notch = turn(45.0, pivot, (200.0, 200.0));
    assert_eq!(hits(&boxes, notch.0, notch.1), vec!["g"]);
    let arm = turn(45.0, pivot, (125.0, 200.0));
    assert_eq!(hits(&boxes, arm.0, arm.1), vec!["u", "g"]);
    // The unturned arm is now outside the shape.
    assert_eq!(hits(&boxes, 125.0, 240.0), vec!["g"]);
}

#[test]
fn clipped_polygon_misses_outside_its_clip() {
    let boxes = boxes_of(
        r#"      frame id="f" x=(px)100 y=(px)100 w=(px)100 h=(px)100 clip=#true {
        polygon id="big" fill=(token)"c.k" {
          point x=(px)-50 y=(px)-50
          point x=(px)150 y=(px)-50
          point x=(px)-50 y=(px)150
        }
      }"#,
    );
    // Inside the triangle and the frame.
    assert_eq!(hits(&boxes, 120.0, 120.0), vec!["big", "f"]);
    // Inside the triangle, outside the frame.
    assert!(hits(&boxes, 90.0, 120.0).is_empty());
    // Inside the frame, outside the triangle.
    assert_eq!(hits(&boxes, 190.0, 190.0), vec!["f"]);
}

#[test]
fn tolerance_reaches_strokes_and_thin_fills_only() {
    let boxes = boxes_of(&format!(
        r#"      polyline id="pl" stroke=(token)"c.k" stroke-width=(px)2 {{
        point x=(px)20 y=(px)20
        point x=(px)380 y=(px)20
      }}
      line id="d" x1=(px)310 y1=(px)60 x2=(px)390 y2=(px)140 stroke=(token)"c.k" stroke-width=(px)2
      polygon id="sliver" fill=(token)"c.k" {{
        point x=(px)20 y=(px)300
        point x=(px)380 y=(px)300
        point x=(px)380 y=(px)302
      }}
      polygon id="u" fill=(token)"c.k" {{
{U_POINTS}
      }}"#
    ));
    // 3 px off a 2 px stroke: 2 px from its band.
    assert!(hits(&boxes, 200.0, 23.0).is_empty());
    assert_eq!(hits_within(&boxes, 200.0, 23.0, 2.5), vec!["pl"]);
    assert!(hits_within(&boxes, 200.0, 23.0, 1.5).is_empty());
    // Near the diagonal line, and far from it inside its bounds.
    assert_eq!(hits_within(&boxes, 350.0, 103.0, 3.0), vec!["d"]);
    assert!(in_bounds(&boxes, "d", 380.0, 70.0));
    assert!(hits_within(&boxes, 380.0, 70.0, 3.0).is_empty());
    // A thin fill takes slop on its edge.
    assert_eq!(hits_within(&boxes, 200.0, 304.0, 4.0), vec!["sliver"]);
    // A large fill takes none: just outside the U's arm, and in its notch.
    assert!(hits_within(&boxes, 98.0, 200.0, 4.0).is_empty());
    assert!(hits_within(&boxes, 152.0, 200.0, 4.0).is_empty());
    // Zero tolerance is the plain hit test.
    assert_eq!(
        hits_within(&boxes, 125.0, 200.0, 0.0),
        hits(&boxes, 125.0, 200.0)
    );
}

#[test]
fn connector_hits_on_its_route_not_its_bounds() {
    let boxes = boxes_of(
        r#"      rect id="a" x=(px)0 y=(px)0 w=(px)40 h=(px)40 fill=(token)"c.k"
      rect id="b" x=(px)300 y=(px)300 w=(px)40 h=(px)40 fill=(token)"c.k"
      connector id="c" from="a" to="b" stroke=(token)"c.k" stroke-width=(px)2"#,
    );
    let c = boxes.get("c").expect("connector box");
    assert!(c.shape.is_some());
    let r = c.rect;
    // Sample the bounds on a grid: the route covers a thin part of them.
    let mut inside = 0;
    let mut total = 0;
    for i in 1..20 {
        for j in 1..20 {
            let (x, y) = (
                r.x + r.w * f64::from(i) / 20.0,
                r.y + r.h * f64::from(j) / 20.0,
            );
            total += 1;
            if hits(&boxes, x, y).contains(&"c".to_owned()) {
                inside += 1;
            }
        }
    }
    assert!(inside * 4 < total, "{inside} of {total} grid points hit");
}

#[test]
fn clipped_stroke_takes_no_slop_outside_the_clip() {
    let boxes = boxes_of(
        r#"      frame id="f" x=(px)100 y=(px)100 w=(px)100 h=(px)100 clip=#true {
        line id="l" x1=(px)-50 y1=(px)50 x2=(px)150 y2=(px)50 stroke=(token)"c.k" stroke-width=(px)2
      }"#,
    );
    // Inside the clip, 3 px off the line.
    assert_eq!(hits_within(&boxes, 150.0, 153.0, 3.0), vec!["l", "f"]);
    // Outside the clip, 3 px off the line.
    assert!(hits_within(&boxes, 50.0, 153.0, 3.0).is_empty());
}
