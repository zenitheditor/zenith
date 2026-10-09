//! Snapping: moves and grip drags land on other nodes' edges and centres,
//! their containers, and the page, within `snap_distance` page px; the
//! reply carries the snapped delta and the guides, and the commit writes
//! ordinary nudges.

use serde_json::{Value, json};

use crate::common::{Driver, assert_moved, doc};

const BODY: &str = r#"      rect id="a" x=(px)10 y=(px)10 w=(px)50 h=(px)40 fill=(token)"color.ink"
      rect id="b" x=(px)100 y=(px)120 w=(px)60 h=(px)30 fill=(token)"color.ink"
      frame id="f" x=(px)200 y=(px)100 w=(px)150 h=(px)150 {
        rect id="kid" x=(px)20 y=(px)20 w=(px)30 h=(px)30 fill=(token)"color.ink"
      }"#;

fn preview(d: &mut Driver, params: Value) -> Value {
    let mut params = params;
    params["render"] = json!(false);
    d.ok("gesture.preview", params)
}

fn line_of<'t>(text: &'t str, id: &str) -> &'t str {
    text.lines()
        .find(|l| l.contains(&format!("id=\"{id}\"")))
        .unwrap_or_else(|| panic!("no line for {id}"))
}

#[test]
fn a_move_snaps_its_edge_to_another_node_and_reports_the_guide() {
    let mut d = Driver::open(&doc(BODY));
    // The right edge 60 + 37 = 97 is 3 px from b's left edge 100.
    let v = preview(
        &mut d,
        json!({ "node": "a", "dx": 37, "dy": 0, "snap_distance": 4 }),
    );
    assert_eq!(v["snap"]["dx"], 40.0, "{v}");
    assert_eq!(v["snap"]["dy"], 0.0);
    let guides = v["snap"]["guides"].as_array().expect("guides");
    assert!(
        guides
            .iter()
            .any(|g| g["axis"] == "x" && g["at"] == 100.0 && g["from"] == 10.0 && g["to"] == 150.0),
        "{v}"
    );
    let before = d.corners("a");
    d.ok(
        "gesture.commit",
        json!({ "node": "a", "dx": 37, "dy": 0, "snap_distance": 4 }),
    );
    assert_moved(before, d.corners("a"), 40.0, 0.0, "snapped");
    assert!(line_of(&d.session.text, "a").contains("x=(px)50 y=(px)10"));
    // Out of reach, or without snap_distance: the pointer delta.
    let far = preview(
        &mut d,
        json!({ "node": "b", "dx": 7, "dy": 0, "snap_distance": 2 }),
    );
    assert_eq!(far["snap"]["dx"], 7.0);
    // b already lines up with kid on y: those guides stay; none on x.
    let guides = far["snap"]["guides"].as_array().expect("guides");
    assert!(guides.iter().all(|g| g["axis"] == "y"), "{far}");
    let off = preview(&mut d, json!({ "node": "b", "dx": 7, "dy": 0 }));
    assert!(off.get("snap").is_none());
}

#[test]
fn centres_the_page_and_the_parent_box_are_targets() {
    let mut d = Driver::open(&doc(BODY));
    // b's centre x 130 + 68 = 198 is 2 px from the page centre 200.
    let v = preview(
        &mut d,
        json!({ "node": "b", "dx": 68, "dy": 0, "snap_distance": 3 }),
    );
    assert_eq!(v["snap"]["dx"], 70.0, "{v}");
    // kid's left edge 220 - 18 = 202 lands on its frame's left edge 200.
    let k = preview(
        &mut d,
        json!({ "node": "kid", "dx": -18, "dy": 0, "snap_distance": 3 }),
    );
    assert_eq!(k["snap"]["dx"], -20.0, "{k}");
    d.ok(
        "gesture.commit",
        json!({ "node": "kid", "dx": -18, "dy": 0, "snap_distance": 3 }),
    );
    assert!(line_of(&d.session.text, "kid").contains("x=(px)0 y=(px)20"));
}

#[test]
fn a_grip_snaps_only_the_edge_it_moves() {
    let mut d = Driver::open(&doc(BODY));
    // a's right edge 60 + 38 = 98 snaps to b's left edge 100: w 50 -> 90.
    let v = preview(
        &mut d,
        json!({ "node": "a", "handle": "e", "dx": 38, "dy": 5, "snap_distance": 4 }),
    );
    assert_eq!(v["snap"]["dx"], 40.0, "{v}");
    d.ok(
        "gesture.commit",
        json!({ "node": "a", "handle": "e", "dx": 38, "dy": 5, "snap_distance": 4 }),
    );
    assert!(line_of(&d.session.text, "a").contains("w=(px)90 h=(px)40"));
    // A turned node's grips do not snap.
    let mut r = Driver::open(&doc(
        r#"      rect id="r" x=(px)10 y=(px)10 w=(px)50 h=(px)40 rotate=(deg)20
      rect id="o" x=(px)100 y=(px)10 w=(px)50 h=(px)40"#,
    ));
    let t = preview(
        &mut r,
        json!({ "node": "r", "handle": "e", "dx": 37, "dy": 0, "snap_distance": 10 }),
    );
    assert_eq!(t["snap"]["dx"], 37.0, "{t}");
}

#[test]
fn a_selection_snaps_its_box_and_snapping_is_deterministic() {
    let mut d = Driver::open(&doc(BODY));
    // The selection a + b spans x 10..160; its right edge 160 + 38 = 198
    // snaps to the frame's left edge 200.
    let params = json!({ "nodes": ["a", "b"], "dx": 38, "dy": 0, "snap_distance": 4 });
    let first = preview(&mut d, params.clone());
    assert_eq!(first["snap"]["dx"], 40.0, "{first}");
    let again = preview(&mut d, params.clone());
    assert_eq!(first, again);
    let a = d.corners("a");
    let b = d.corners("b");
    d.ok("gesture.commit", params);
    assert_moved(a, d.corners("a"), 40.0, 0.0, "a");
    assert_moved(b, d.corners("b"), 40.0, 0.0, "b");
}
