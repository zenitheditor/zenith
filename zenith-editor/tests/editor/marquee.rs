//! `select.marquee`: the nodes a page rectangle meets or holds, with the
//! master, lock, hidden, container, and outermost rules.

use serde_json::{Value, json};

use crate::common::{Driver, doc};

fn picked(v: &Value) -> Vec<String> {
    v["hits"]
        .as_array()
        .expect("hits")
        .iter()
        .map(|h| h["id"].as_str().expect("id").to_owned())
        .collect()
}

const BODY: &str = r#"      rect id="a" x=(px)10 y=(px)10 w=(px)50 h=(px)50 fill=(token)"color.ink"
      rect id="b" x=(px)100 y=(px)10 w=(px)50 h=(px)50 fill=(token)"color.ink"
      rect id="lock" x=(px)170 y=(px)10 w=(px)20 h=(px)20 fill=(token)"color.ink" locked=#true
      rect id="hid" x=(px)10 y=(px)70 w=(px)20 h=(px)20 fill=(token)"color.ink" visible=#false
      line id="diag" x1=(px)200 y1=(px)100 x2=(px)300 y2=(px)200 stroke=(token)"color.ink"
      frame id="f" x=(px)10 y=(px)150 w=(px)150 h=(px)100 {
        rect id="k1" x=(px)10 y=(px)10 w=(px)30 h=(px)30 fill=(token)"color.ink"
        rect id="k2" x=(px)80 y=(px)10 w=(px)30 h=(px)30 fill=(token)"color.ink"
      }"#;

#[test]
fn intersect_selects_what_the_rectangle_meets_back_to_front() {
    let mut d = Driver::open(&doc(BODY));
    let v = d.ok(
        "select.marquee",
        json!({ "x": 40, "y": 0, "w": 160, "h": 40 }),
    );
    assert_eq!(picked(&v), ["a", "b"], "the locked node never counts");
    assert_eq!(v["selection"], json!(["a", "b"]));
    assert_eq!(v["page"], 1);
    // A negative size extends left and up.
    let back = d.ok(
        "select.marquee",
        json!({ "x": 200, "y": 40, "w": -160, "h": -40 }),
    );
    assert_eq!(picked(&back), ["a", "b"]);
    // Hidden nodes draw no box.
    let hid = d.ok(
        "select.marquee",
        json!({ "x": 0, "y": 65, "w": 40, "h": 30 }),
    );
    assert!(picked(&hid).is_empty(), "{hid}");
    assert_eq!(hid["selection"], json!([]), "a miss clears the selection");
}

#[test]
fn contain_needs_the_whole_box_and_extend_adds() {
    let mut d = Driver::open(&doc(BODY));
    let v = d.ok(
        "select.marquee",
        json!({ "x": 0, "y": 0, "w": 120, "h": 70, "contain": true }),
    );
    assert_eq!(picked(&v), ["a"], "b sticks out");
    let more = d.ok(
        "select.marquee",
        json!({ "x": 95, "y": 5, "w": 60, "h": 60, "extend": true }),
    );
    assert_eq!(more["selection"], json!(["a", "b"]));
    let peek = d.ok(
        "select.marquee",
        json!({ "x": 0, "y": 0, "w": 400, "h": 300, "select": false }),
    );
    assert_eq!(
        peek["selection"],
        json!(["a", "b"]),
        "select: false keeps it"
    );
}

#[test]
fn shapes_count_by_their_paint() {
    let mut d = Driver::open(&doc(BODY));
    // Inside the diagonal line's bounds, away from its stroke.
    let miss = d.ok(
        "select.marquee",
        json!({ "x": 270, "y": 105, "w": 25, "h": 25 }),
    );
    assert!(picked(&miss).is_empty(), "{miss}");
    let hit = d.ok(
        "select.marquee",
        json!({ "x": 245, "y": 140, "w": 20, "h": 20 }),
    );
    assert_eq!(picked(&hit), ["diag"]);
}

#[test]
fn containers_are_entered_and_the_outermost_node_wins() {
    let mut d = Driver::open(&doc(BODY));
    // Inside the frame: its content counts, not the frame.
    let inside = d.ok(
        "select.marquee",
        json!({ "x": 15, "y": 155, "w": 120, "h": 20 }),
    );
    assert_eq!(picked(&inside), ["k1", "k2"]);
    // Across the frame's edge: the frame counts and its content rides along.
    let across = d.ok(
        "select.marquee",
        json!({ "x": 0, "y": 140, "w": 60, "h": 30 }),
    );
    assert_eq!(picked(&across), ["f"]);
    // Holding the frame with contain: the frame only.
    let whole = d.ok(
        "select.marquee",
        json!({ "x": 0, "y": 140, "w": 200, "h": 120, "contain": true }),
    );
    assert_eq!(picked(&whole), ["f"]);
}

#[test]
fn master_and_instance_content_select_what_a_click_selects() {
    let mut d = Driver::open(crate::select::MASTER_DOC);
    let all = d.ok(
        "select.marquee",
        json!({ "x": 0, "y": 0, "w": 400, "h": 300 }),
    );
    assert_eq!(picked(&all), ["band", "inst"], "{all}");
    assert_eq!(all["hits"][0]["master"], "m");
    assert!(all["hits"][1].get("master").is_none());
}
