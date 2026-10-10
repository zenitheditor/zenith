//! `select.hit` and `select.set`: rotated group children, overlapping
//! nodes, thin lines with tolerance, master content, and instances.

use serde_json::{Value, json};

use crate::common::{Driver, doc};

fn ids(reply: &Value) -> Vec<String> {
    reply["hits"]
        .as_array()
        .expect("hits")
        .iter()
        .map(|h| h["id"].as_str().expect("id").to_owned())
        .collect()
}

/// `(x, y)` turned `deg` degrees about `(cx, cy)`.
fn turn(deg: f64, (cx, cy): (f64, f64), (x, y): (f64, f64)) -> (f64, f64) {
    let (sin, cos) = deg.to_radians().sin_cos();
    let (dx, dy) = (x - cx, y - cy);
    (cx + cos * dx - sin * dy, cy + sin * dx + cos * dy)
}

#[test]
fn rotated_group_child_is_hit_on_its_drawn_box() {
    let mut d = Driver::open(&doc(
        r#"      group id="g" x=(px)100 y=(px)50 w=(px)200 h=(px)200 rotate=(deg)30 {
        rect id="r" x=(px)0 y=(px)0 w=(px)200 h=(px)20 fill=(token)"color.ink"
      }"#,
    ));
    let pivot = (200.0, 150.0);
    let inside = turn(30.0, pivot, (280.0, 60.0));
    let hit = d.ok("select.hit", json!({ "x": inside.0, "y": inside.1 }));
    assert_eq!(ids(&hit), vec!["r", "g"]);
    assert_eq!(d.session.selection, vec!["r".to_owned()]);
    // Inside the unturned box but outside the turned child: the group only.
    let miss = d.ok("select.hit", json!({ "x": 120.0, "y": 60.0 }));
    assert!(!ids(&miss).contains(&"r".to_owned()), "{miss}");
}

#[test]
fn overlapping_nodes_come_topmost_first_and_extend_toggles() {
    let mut d = Driver::open(&doc(
        r#"      rect id="low" x=(px)10 y=(px)10 w=(px)100 h=(px)100 fill=(token)"color.bg"
      rect id="high" x=(px)50 y=(px)50 w=(px)100 h=(px)100 fill=(token)"color.ink""#,
    ));
    let hit = d.ok("select.hit", json!({ "x": 80, "y": 80 }));
    assert_eq!(ids(&hit), vec!["high", "low"]);
    assert_eq!(d.session.selection, vec!["high".to_owned()]);
    d.ok("select.hit", json!({ "x": 20, "y": 20, "extend": true }));
    assert_eq!(
        d.session.selection,
        vec!["high".to_owned(), "low".to_owned()]
    );
    d.ok("select.hit", json!({ "x": 20, "y": 20, "extend": true }));
    assert_eq!(d.session.selection, vec!["high".to_owned()]);
    d.ok("select.hit", json!({ "x": 390, "y": 290 }));
    assert!(d.session.selection.is_empty(), "a miss clears");
    let quiet = d.ok("select.hit", json!({ "x": 80, "y": 80, "select": false }));
    assert_eq!(ids(&quiet), vec!["high", "low"]);
    assert!(d.session.selection.is_empty());
}

#[test]
fn thin_lines_take_tolerance_and_diagonals_hit_on_the_segment() {
    let mut d = Driver::open(&doc(
        r#"      line id="h" x1=(px)20 y1=(px)100 x2=(px)300 y2=(px)100 stroke=(token)"color.ink"
      line id="diag" x1=(px)20 y1=(px)150 x2=(px)220 y2=(px)290 stroke=(token)"color.ink""#,
    ));
    let exact = d.ok("select.hit", json!({ "x": 100, "y": 104, "tolerance": 0 }));
    assert!(ids(&exact).is_empty(), "{exact}");
    let near = d.ok("select.hit", json!({ "x": 100, "y": 104, "tolerance": 5 }));
    assert_eq!(ids(&near), vec!["h"]);
    // Inside the diagonal's bounds but far from the segment: no hit.
    let off = d.ok("select.hit", json!({ "x": 200, "y": 160, "tolerance": 3 }));
    assert!(ids(&off).is_empty(), "{off}");
    // On the segment (t = 0.5): a hit.
    let on = d.ok("select.hit", json!({ "x": 120, "y": 220, "tolerance": 3 }));
    assert_eq!(ids(&on), vec!["diag"]);
}

#[test]
fn concave_polygon_notch_selects_what_lies_behind_it() {
    let mut d = Driver::open(&doc(
        r#"      rect id="back" x=(px)0 y=(px)0 w=(px)400 h=(px)300 fill=(token)"color.bg"
      polygon id="u" fill=(token)"color.ink" {
        point x=(px)100 y=(px)50
        point x=(px)300 y=(px)50
        point x=(px)300 y=(px)250
        point x=(px)250 y=(px)250
        point x=(px)250 y=(px)100
        point x=(px)150 y=(px)100
        point x=(px)150 y=(px)250
        point x=(px)100 y=(px)250
      }"#,
    ));
    // In the notch of the U: inside its bounds, outside its fill.
    let notch = d.ok("select.hit", json!({ "x": 200, "y": 200, "tolerance": 4 }));
    assert_eq!(ids(&notch), vec!["back"]);
    assert_eq!(d.session.selection, vec!["back".to_owned()]);
    let arm = d.ok("select.hit", json!({ "x": 125, "y": 200 }));
    assert_eq!(ids(&arm), vec!["u", "back"]);
    assert_eq!(d.session.selection, vec!["u".to_owned()]);
}

#[test]
fn guides_and_hidden_nodes_never_hit() {
    let mut d = Driver::open(&doc(
        r#"      rect id="g" x=(px)0 y=(px)0 w=(px)400 h=(px)300 role="guide"
      rect id="hid" x=(px)0 y=(px)0 w=(px)400 h=(px)300 visible=#false fill=(token)"color.ink""#,
    ));
    let hit = d.ok("select.hit", json!({ "x": 50, "y": 50, "tolerance": 4 }));
    assert!(ids(&hit).is_empty(), "{hit}");
}

pub(crate) const MASTER_DOC: &str = r##"zenith version=1 {
  project id="proj.m" name="M"
  tokens format="zenith-token-v1" {
    token id="c" type="color" value="#102030"
  }
  styles {}
  components {
    component id="tile" {
      rect id="cr" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"c"
    }
  }
  masters {
    master id="m" {
      rect id="band" x=(px)0 y=(px)0 w=(px)400 h=(px)30 fill=(token)"c"
    }
  }
  document id="doc.m" title="M" {
    page id="p1" w=(px)400 h=(px)300 master="m" {
      instance id="inst" component="tile" x=(px)100 y=(px)100 w=(px)50 h=(px)50 fit="fill"
    }
  }
}
"##;

#[test]
fn master_content_and_instances_report_where_they_come_from() {
    let mut d = Driver::open(MASTER_DOC);
    let band = d.ok("select.hit", json!({ "x": 50, "y": 10 }));
    let top = &band["hits"][0];
    assert_eq!(top["id"], "band");
    assert_eq!(top["raw_id"], "p1/band");
    assert_eq!(top["master"], "m");
    let inst = d.ok("select.hit", json!({ "x": 120, "y": 120 }));
    let top = &inst["hits"][0];
    assert_eq!(top["id"], "inst");
    assert_eq!(top["via"], "instance");
    assert_eq!(ids(&inst), vec!["inst"], "content and instance are one hit");
    let handles = d.ok("node.handles", json!({ "id": "band" }));
    assert_eq!(handles["master"], "m");
}

#[test]
fn select_set_checks_ids_and_drops_repeats() {
    let mut d = Driver::open(&doc(r#"      rect id="a" x=(px)0 y=(px)0 w=(px)10 h=(px)10
      rect id="b" x=(px)0 y=(px)0 w=(px)10 h=(px)10"#));
    let r = d.ok("select.set", json!({ "ids": ["b", "a", "b"] }));
    assert_eq!(r["selection"], json!(["b", "a"]));
    let e = d.err("select.set", json!({ "ids": ["zz"] }));
    assert_eq!(e.code, "editor.unknown_node");
}

#[test]
fn select_at_offset_names_the_innermost_node_under_the_cursor() {
    let mut d = Driver::open(&doc(r#"      group id="g" {
        rect id="inner" x=(px)0 y=(px)0 w=(px)10 h=(px)10
      }
      // A comment with a multi-byte dash — before the text node.
      text id="t" x=(px)0 y=(px)40 w=(px)100 h=(px)20 { span "naïve" }"#));
    let text = d.session.text.clone();
    let inner = text.find("rect id=\"inner\"").expect("inner") + 4;
    let r = d.ok("select.at_offset", json!({ "offset": inner }));
    assert_eq!(
        (r["id"].as_str(), r["kind"].as_str()),
        (Some("inner"), Some("rect"))
    );
    assert_eq!(r["page"], 1);
    assert_eq!(d.session.selection, vec!["inner".to_owned()]);
    // A byte offset past the multi-byte characters still lands in `t`.
    let span = text.find("naïve").expect("span") + "naï".len();
    let r = d.ok("select.at_offset", json!({ "offset": span }));
    assert_eq!(r["id"], "t");
    // The comment line sits in the page but in no node: the selection clears.
    let comment = text.find("// A comment").expect("comment");
    let r = d.ok("select.at_offset", json!({ "offset": comment }));
    assert_eq!(
        (r["id"].clone(), r["page"].clone()),
        (Value::Null, json!(1))
    );
    assert!(d.session.selection.is_empty());
    // `select: false` reports without touching the session.
    let quiet = d.ok(
        "select.at_offset",
        json!({ "offset": inner, "select": false }),
    );
    assert_eq!(quiet["id"], "inner");
    assert!(d.session.selection.is_empty());
    let e = d.err("select.at_offset", json!({ "offset": text.len() + 1 }));
    assert_eq!(e.code, "editor.invalid_params");
}

#[test]
fn select_at_offset_keeps_the_session_while_the_text_does_not_parse() {
    let mut d = Driver::open(&doc(
        r#"      rect id="a" x=(px)0 y=(px)0 w=(px)10 h=(px)10"#,
    ));
    d.ok("select.set", json!({ "ids": ["a"] }));
    let broken = d.session.text.replace("rect id=\"a\"", "rect id=\"a");
    d.ok("buffer.set", json!({ "text": broken }));
    let before = d.session.clone();
    let r = d.ok("select.at_offset", json!({ "offset": 10 }));
    assert_eq!(r["parsed"], false);
    assert_eq!(d.session, before);
}

#[test]
fn shadowed_rotated_group_child_is_hit_on_its_drawn_box_inside_the_clip() {
    let shadowed = doc(
        r#"      frame id="clipper" x=(px)0 y=(px)0 w=(px)250 h=(px)300 {
        group id="g" x=(px)100 y=(px)50 w=(px)200 h=(px)200 rotate=(deg)30 shadow=(token)"sh" {
          rect id="r" x=(px)0 y=(px)0 w=(px)200 h=(px)20 fill=(token)"color.ink"
        }
      }"#,
    )
    .replace(
        r#"    token id="size.w" type="dimension" value=(px)80"#,
        r#"    token id="size.w" type="dimension" value=(px)80
    token id="sh" type="shadow" {
      layer dx=(px)2 dy=(px)4 blur=(px)6 color=(token)"color.ink"
    }"#,
    );
    let mut d = Driver::open(&shadowed);
    let pivot = (200.0, 150.0);
    // On the turned child, inside the clipping frame.
    let inside = turn(30.0, pivot, (180.0, 60.0));
    let hit = d.ok("select.hit", json!({ "x": inside.0, "y": inside.1 }));
    assert_eq!(ids(&hit), vec!["r", "g", "clipper"]);
    assert_eq!(d.session.selection, vec!["r".to_owned()]);
    // Inside the unturned box but outside the turned child.
    let miss = d.ok("select.hit", json!({ "x": 120.0, "y": 60.0 }));
    assert!(!ids(&miss).contains(&"r".to_owned()), "{miss}");
    // On the turned child, but past the clipping frame's right edge.
    let clipped = turn(30.0, pivot, (280.0, 60.0));
    assert!(clipped.0 > 250.0);
    let hit = d.ok("select.hit", json!({ "x": clipped.0, "y": clipped.1 }));
    assert!(!ids(&hit).contains(&"r".to_owned()), "{hit}");
}
