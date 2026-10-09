//! Click-to-inspect data: `PageCompiler::compile_page_with_boxes`,
//! `CompiledBox::{corners, contains}`, `hit_test`, and `selectable_id`.

use std::collections::BTreeMap;

use zenith_core::{Document, KdlAdapter, KdlSource, default_provider};
use zenith_scene::{
    CompileResult, CompiledBox, DocumentPrep, PageCompiler, SceneCommand, hit_test, selectable_id,
};

const HEAD: &str = r##"zenith version=1 {
  project id="proj.hit" name="Hit"
  tokens format="zenith-token-v1" {
    token id="c.k" type="color" value="#000000"
    token id="sh" type="shadow" {
      layer dx=(px)2 dy=(px)3 blur=(px)4 color=(token)"c.k"
    }
  }
  styles {}
"##;

fn doc_of(body: &str) -> Document {
    let src = format!("{HEAD}{body}\n}}\n");
    KdlAdapter.parse(src.as_bytes()).expect("fixture parses")
}

/// The boxes of page 0 of a document with `body` after the shared head.
fn boxes_of(body: &str) -> (Document, BTreeMap<String, CompiledBox>) {
    let (doc, _, boxes) = compiled_of(body);
    (doc, boxes)
}

/// Page 0 of a document with `body` after the shared head: the document,
/// the compile result, and the boxes, from one pass.
fn compiled_of(body: &str) -> (Document, CompileResult, BTreeMap<String, CompiledBox>) {
    let doc = doc_of(body);
    let fonts = default_provider();
    let (result, boxes) = {
        let prep = DocumentPrep::new(&doc, None, None);
        PageCompiler::new(&prep, &fonts).compile_page_with_boxes(0, false)
    };
    (doc, result, boxes)
}

/// A one-page document whose page holds `children`.
fn page(children: &str) -> String {
    format!(
        r#"  document id="doc.hit" title="Hit" {{
    page id="p" w=(px)400 h=(px)400 {{
{children}
    }}
  }}"#
    )
}

fn hits(boxes: &BTreeMap<String, CompiledBox>, x: f64, y: f64) -> Vec<String> {
    hit_test(boxes, x, y)
        .into_iter()
        .map(str::to_owned)
        .collect()
}

/// `(x, y)` turned `deg` degrees about `(cx, cy)`, as the renderer turns it.
fn turn(deg: f64, (cx, cy): (f64, f64), (x, y): (f64, f64)) -> (f64, f64) {
    let (sin, cos) = deg.to_radians().sin_cos();
    let (dx, dy) = (x - cx, y - cy);
    (cx + cos * dx - sin * dy, cy + sin * dx + cos * dy)
}

fn assert_corners(b: &CompiledBox, want: [(f64, f64); 4]) {
    for (got, want) in b.corners().iter().zip(want) {
        assert!(
            (got.0 - want.0).abs() < 1e-6 && (got.1 - want.1).abs() < 1e-6,
            "corners {:?}, want {want:?}",
            b.corners()
        );
    }
}

fn get<'a>(boxes: &'a BTreeMap<String, CompiledBox>, id: &str) -> &'a CompiledBox {
    boxes
        .get(id)
        .unwrap_or_else(|| panic!("no box for {id}: {:?}", boxes.keys()))
}

#[test]
fn rotated_group_child_is_hit_on_its_drawn_box_only() {
    let (_, boxes) = boxes_of(&page(
        r#"      group id="g" x=(px)100 y=(px)100 w=(px)200 h=(px)200 rotate=(deg)30 {
        rect id="r" x=(px)0 y=(px)0 w=(px)200 h=(px)20 fill=(token)"c.k"
      }"#,
    ));
    let pivot = (200.0, 200.0);
    let r = get(&boxes, "r");
    assert_corners(
        r,
        [
            turn(30.0, pivot, (100.0, 100.0)),
            turn(30.0, pivot, (300.0, 100.0)),
            turn(30.0, pivot, (300.0, 120.0)),
            turn(30.0, pivot, (100.0, 120.0)),
        ],
    );
    // `world` and `local` reproduce the axis-aligned `rect` summary.
    let xs = r.corners().map(|c| c.0);
    let ys = r.corners().map(|c| c.1);
    let min = |v: [f64; 4]| v.iter().copied().fold(f64::INFINITY, f64::min);
    let max = |v: [f64; 4]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    assert!((min(xs) - r.rect.x).abs() < 1e-6 && (min(ys) - r.rect.y).abs() < 1e-6);
    assert!((max(xs) - (r.rect.x + r.rect.w)).abs() < 1e-6);
    assert!((max(ys) - (r.rect.y + r.rect.h)).abs() < 1e-6);
    assert_eq!(r.rotate, None);

    // Inside the turned child, outside its unturned box: a hit.
    let inside = turn(30.0, pivot, (280.0, 110.0));
    assert!(inside.1 < 100.0 || inside.1 > 120.0, "{inside:?}");
    assert_eq!(hits(&boxes, inside.0, inside.1), vec!["r", "g"]);
    // Inside the unturned box, outside the turned child: a miss.
    assert!(!hits(&boxes, 120.0, 110.0).contains(&"r".to_owned()));
    // Inside the axis-aligned `rect` summary, outside the drawn box: a miss.
    let corner = (r.rect.x + 1.0, r.rect.y + 1.0);
    assert!(hits(&boxes, corner.0, corner.1).is_empty());
}

#[test]
fn own_rotation_turns_the_box_about_its_pivot() {
    let (_, boxes) = boxes_of(&page(
        r#"      rect id="r" x=(px)100 y=(px)100 w=(px)100 h=(px)40 rotate=(deg)90 fill=(token)"c.k""#,
    ));
    let r = get(&boxes, "r");
    assert_eq!(r.rotate, Some(90.0));
    // The box keeps its unturned `local`; `spin` turns it about its centre.
    assert_eq!(
        (r.local.x, r.local.y, r.local.w, r.local.h),
        (100.0, 100.0, 100.0, 40.0)
    );
    let pivot = (150.0, 120.0);
    assert_corners(
        r,
        [
            turn(90.0, pivot, (100.0, 100.0)),
            turn(90.0, pivot, (200.0, 100.0)),
            turn(90.0, pivot, (200.0, 140.0)),
            turn(90.0, pivot, (100.0, 140.0)),
        ],
    );
    assert_eq!(hits(&boxes, 150.0, 75.0), vec!["r"]);
    assert!(hits(&boxes, 105.0, 120.0).is_empty());
}

#[test]
fn nested_rotations_and_instance_scale_compose() {
    let (_, boxes) = boxes_of(&format!(
        r#"  components {{
    component id="tile" {{
      rect id="cr" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"c.k"
    }}
  }}
{}"#,
        page(
            r#"      group id="outer" x=(px)0 y=(px)0 w=(px)400 h=(px)400 rotate=(deg)30 {
        frame id="inner" x=(px)100 y=(px)100 w=(px)200 h=(px)200 rotate=(deg)45 {
          rect id="leaf" x=(px)50 y=(px)50 w=(px)40 h=(px)20 fill=(token)"c.k"
          instance id="si" component="tile" x=(px)100 y=(px)100 w=(px)50 h=(px)50 fit="fill"
        }
      }"#
        )
    ));
    let outer = |p| turn(30.0, (200.0, 200.0), p);
    let both = |p| outer(turn(45.0, (200.0, 200.0), p));
    let leaf = get(&boxes, "leaf");
    assert_corners(
        leaf,
        [
            both((150.0, 150.0)),
            both((190.0, 150.0)),
            both((190.0, 170.0)),
            both((150.0, 170.0)),
        ],
    );
    let centre = both((170.0, 160.0));
    assert_eq!(hits(&boxes, centre.0, centre.1)[0], "leaf");

    // The 10 px tile fills the 50 px instance box: scale 5.
    let tile = get(&boxes, "si/cr");
    assert_corners(
        tile,
        [
            both((200.0, 200.0)),
            both((250.0, 200.0)),
            both((250.0, 250.0)),
            both((200.0, 250.0)),
        ],
    );
    let centre = both((225.0, 225.0));
    assert_eq!(
        hits(&boxes, centre.0, centre.1),
        vec!["si/cr", "si", "inner", "outer"]
    );
}

#[test]
fn instance_content_selects_its_instance() {
    let (doc, boxes) = boxes_of(&format!(
        r#"  components {{
    component id="card" {{
      rect id="bg" x=(px)0 y=(px)0 w=(px)50 h=(px)30 fill=(token)"c.k"
    }}
  }}
{}"#,
        page(
            r#"      group id="wrap" x=(px)10 y=(px)10 {
        instance id="c1" component="card" x=(px)100 y=(px)100
      }"#
        )
    ));
    let bg = get(&boxes, "c1/bg");
    assert_eq!(
        (bg.rect.x, bg.rect.y, bg.rect.w, bg.rect.h),
        (110.0, 110.0, 50.0, 30.0)
    );
    let hit = hits(&boxes, 120.0, 120.0);
    assert_eq!(hit, vec!["c1/bg", "c1", "wrap"]);
    assert_eq!(selectable_id(&doc, 0, "c1/bg"), Some("c1"));
    assert_eq!(selectable_id(&doc, 0, "c1"), Some("c1"));
    assert_eq!(selectable_id(&doc, 0, "wrap"), Some("wrap"));
    assert_eq!(selectable_id(&doc, 0, "nope"), None);
    assert_eq!(selectable_id(&doc, 3, "c1"), None);
}

#[test]
fn master_projection_selects_the_master_node_and_sits_under_page_content() {
    let (doc, boxes) = boxes_of(
        r#"  masters {
    master id="m" {
      group id="mg" {
        rect id="band" x=(px)0 y=(px)0 w=(px)400 h=(px)50 fill=(token)"c.k"
      }
    }
  }
  document id="doc.hit" title="Hit" {
    page id="p" w=(px)400 h=(px)400 master="m" {
      rect id="over" x=(px)10 y=(px)10 w=(px)20 h=(px)20 fill=(token)"c.k"
    }
  }"#,
    );
    assert_eq!(hits(&boxes, 200.0, 25.0), vec!["p/band", "p/mg"]);
    assert_eq!(hits(&boxes, 15.0, 15.0), vec!["over", "p/band", "p/mg"]);
    assert_eq!(selectable_id(&doc, 0, "p/band"), Some("band"));
    assert_eq!(selectable_id(&doc, 0, "p/mg"), Some("mg"));
    assert_eq!(selectable_id(&doc, 0, "over"), Some("over"));
    assert_eq!(selectable_id(&doc, 0, "p/ghost"), None);
}

#[test]
fn clipped_child_misses_outside_its_clip() {
    let (_, boxes) = boxes_of(&page(
        r#"      frame id="f" x=(px)100 y=(px)100 w=(px)100 h=(px)100 clip=#true {
        rect id="big" x=(px)50 y=(px)50 w=(px)200 h=(px)200 fill=(token)"c.k"
      }
      frame id="rf" x=(px)100 y=(px)250 w=(px)100 h=(px)100 rotate=(deg)45 clip=#true {
        rect id="rbig" x=(px)-100 y=(px)-100 w=(px)300 h=(px)300 fill=(token)"c.k"
      }"#,
    ));
    let big = get(&boxes, "big");
    assert!(!big.clip.is_empty());
    assert_eq!(hits(&boxes, 175.0, 175.0), vec!["big", "f"]);
    // Inside `big`, outside the frame's clip.
    let in_box = |b: &CompiledBox, x: f64, y: f64| {
        x >= b.local.x && x <= b.local.x + b.local.w && y >= b.local.y && y <= b.local.y + b.local.h
    };
    assert!(in_box(big, 225.0, 225.0) && !big.contains(225.0, 225.0));
    assert!(hits(&boxes, 225.0, 225.0).is_empty());

    // A turned clip is exact: the frame's turned square, not its bounds.
    let pivot = (150.0, 300.0);
    let tip = turn(45.0, pivot, (100.0, 250.0));
    let just_in = (tip.0, tip.1 + 2.0);
    assert_eq!(hits(&boxes, just_in.0, just_in.1), vec!["rbig", "rf"]);
    // Inside the axis-aligned bounds of the turned clip, outside the clip.
    assert!(hits(&boxes, tip.0 - 30.0, tip.1 + 5.0).is_empty());
}

#[test]
fn later_sibling_beats_every_node_of_an_earlier_sibling() {
    let (_, boxes) = boxes_of(&page(
        r#"      group id="low" {
        rect id="low.child" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"c.k"
      }
      rect id="high" x=(px)50 y=(px)50 w=(px)100 h=(px)100 fill=(token)"c.k""#,
    ));
    assert_eq!(hits(&boxes, 75.0, 75.0), vec!["high", "low.child", "low"]);
    assert_eq!(hits(&boxes, 25.0, 25.0), vec!["low.child", "low"]);
    let (low, child, high) = (
        get(&boxes, "low").paint_order,
        get(&boxes, "low.child").paint_order,
        get(&boxes, "high").paint_order,
    );
    assert!(low < child && child < high);
}

#[test]
fn text_box_is_hit_on_its_measured_box() {
    let (_, boxes) = boxes_of(&page(
        r#"      text id="t" x=(px)20 y=(px)20 w=(px)200 font-size=(px)20 fill=(token)"c.k" {
        span "Hello"
      }
      text id="tr" x=(px)100 y=(px)200 w=(px)200 h=(px)40 rotate=(deg)90 font-size=(px)20 fill=(token)"c.k" {
        span "Turned"
      }"#,
    ));
    let t = get(&boxes, "t");
    assert!(t.local.h > 15.0, "{:?}", t.local);
    // Inside the measured box, past the ink of "Hello".
    assert_eq!(hits(&boxes, 210.0, 21.0), vec!["t"]);
    assert!(hits(&boxes, 210.0, 20.0 + t.local.h + 1.0).is_empty());
    // A turned text box with `h` turns about its centre.
    let tr = get(&boxes, "tr");
    assert_eq!(tr.rotate, Some(90.0));
    assert_eq!(hits(&boxes, 200.0, 140.0), vec!["tr"]);
    assert!(hits(&boxes, 110.0, 220.0).is_empty());
}

#[test]
fn line_box_holds_half_the_stroke_and_hidden_nodes_never_hit() {
    let (doc, boxes) = boxes_of(&page(
        r#"      line id="l" x1=(px)10 y1=(px)300 x2=(px)110 y2=(px)300 stroke=(token)"c.k" stroke-width=(px)4
      rect id="gone" x=(px)200 y=(px)200 w=(px)50 h=(px)50 visible=#false fill=(token)"c.k"
      group id="hg" visible=#false {
        rect id="hg.child" x=(px)300 y=(px)300 w=(px)50 h=(px)50 fill=(token)"c.k"
      }"#,
    ));
    assert_eq!(hits(&boxes, 50.0, 301.9), vec!["l"]);
    assert!(hits(&boxes, 50.0, 302.5).is_empty());
    let gone = get(&boxes, "gone");
    assert!(gone.hidden);
    assert!(hits(&boxes, 225.0, 225.0).is_empty());
    assert!(!boxes.contains_key("hg.child"));
    assert!(hits(&boxes, 325.0, 325.0).is_empty());
    // A hidden node stays selectable by id (a layers list shows it).
    assert_eq!(selectable_id(&doc, 0, "gone"), Some("gone"));
}

#[test]
fn pattern_motifs_hit_above_their_pattern_and_select_it() {
    let (doc, result, boxes) = compiled_of(&page(
        r#"      pattern id="pt" kind="grid" x=(px)0 y=(px)0 w=(px)100 h=(px)100 spacing=(px)50 shadow=(token)"sh" {
        rect id="m" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"c.k" rotate=(deg)45
      }"#,
    ));
    let motif = boxes
        .iter()
        .find(|(id, _)| id.starts_with("pt/") && id.ends_with("/m"))
        .map(|(id, b)| (id.clone(), b.clone()))
        .expect("a motif box");
    let (cx, cy) = motif
        .1
        .transform()
        .apply(5.0 + motif.1.local.x, 5.0 + motif.1.local.y);
    let hit = hits(&boxes, cx, cy);
    assert_eq!(hit.first(), Some(&motif.0), "{hit:?}");
    assert_eq!(hit.last(), Some(&"pt".to_owned()));
    assert!(motif.1.paint_order > get(&boxes, "pt").paint_order);
    assert_eq!(selectable_id(&doc, 0, &motif.0), Some("pt"));
    // The shadow bracket opens before the tiling: the motif's first command
    // (its own rotation) still sits at `command_index`.
    assert!(
        result
            .scene
            .commands
            .iter()
            .any(|c| matches!(c, SceneCommand::BeginShadow { .. }))
    );
    assert!(matches!(
        result.scene.commands.get(motif.1.command_index),
        Some(SceneCommand::PushTransform { angle_deg, .. }) if *angle_deg == 45.0
    ));
}

#[test]
fn guides_have_no_box_and_select_nothing() {
    let (doc, boxes) = boxes_of(&page(
        r#"      rect id="guide" role="guide" x=(px)0 y=(px)0 w=(px)400 h=(px)400 fill=(token)"c.k"
      group id="gg" role="guide" {
        rect id="gg.child" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"c.k"
      }
      rect id="real" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"c.k""#,
    ));
    assert!(!boxes.contains_key("guide") && !boxes.contains_key("gg.child"));
    assert_eq!(hits(&boxes, 5.0, 5.0), vec!["real"]);
    assert_eq!(selectable_id(&doc, 0, "guide"), None);
    assert_eq!(selectable_id(&doc, 0, "gg.child"), None);
}

#[test]
fn gap_line_jumps_keep_command_indices_on_the_scene() {
    let doc = doc_of(
        r#"  document id="doc.hit" title="Hit" {
    page id="p" w=(px)400 h=(px)400 line-jumps="gap" {
      rect id="a" x=(px)0 y=(px)180 w=(px)20 h=(px)40 fill=(token)"c.k"
      rect id="b" x=(px)380 y=(px)180 w=(px)20 h=(px)40 fill=(token)"c.k"
      rect id="c" x=(px)180 y=(px)0 w=(px)40 h=(px)20 fill=(token)"c.k"
      rect id="d" x=(px)180 y=(px)380 w=(px)40 h=(px)20 fill=(token)"c.k"
      connector id="h" from="a" to="b" stroke=(token)"c.k"
      connector id="v" from="c" to="d" stroke=(token)"c.k"
      rect id="after" x=(px)300 y=(px)300 w=(px)10 h=(px)10 fill=(token)"c.k"
    }
  }"#,
    );
    let fonts = default_provider();
    let prep = DocumentPrep::new(&doc, None, None);
    let (result, boxes) = PageCompiler::new(&prep, &fonts).compile_page_with_boxes(0, false);
    // The crossing split one connector stroke in two.
    let strokes = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::StrokePolyline { .. }))
        .count();
    assert_eq!(strokes, 3);
    let after = get(&boxes, "after");
    assert!(
        matches!(
            result.scene.commands.get(after.command_index),
            Some(SceneCommand::FillRect { x, .. }) if *x == 300.0
        ),
        "{:?}",
        result.scene.commands.get(after.command_index)
    );
    let v = get(&boxes, "v");
    assert!(matches!(
        result.scene.commands.get(v.command_index),
        Some(SceneCommand::StrokePolyline { .. })
    ));
}
