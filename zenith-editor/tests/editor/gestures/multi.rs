//! Selection gestures: several nodes moved, resized, and turned as one
//! transaction, with mixed parents, refusals merged, and the selection
//! handles.

use serde_json::{Value, json};

use crate::common::{Driver, assert_moved, codes, corners_of, doc, near, offer_ids};

const MIXED: &str = r#"      rect id="a" x=(px)10 y=(px)10 w=(px)50 h=(px)40 fill=(token)"color.ink"
      group id="g" x=(px)200 y=(px)40 w=(px)120 h=(px)120 rotate=(deg)30 {
        rect id="inner" x=(px)10 y=(px)10 w=(px)40 h=(px)20 fill=(token)"color.ink"
      }
      line id="l" x1=(px)20 y1=(px)200 x2=(px)120 y2=(px)240 stroke=(token)"color.ink"
      polygon id="tri" fill=(token)"color.ink" {
        point x=(px)150 y=(px)200
        point x=(px)190 y=(px)260
        point x=(px)110 y=(px)260
      }"#;

fn ids(v: &Value) -> Vec<String> {
    v.as_array()
        .expect("ids")
        .iter()
        .map(|x| x.as_str().expect("id").to_owned())
        .collect()
}

#[test]
fn move_moves_every_node_by_the_page_delta_in_one_entry() {
    let text = doc(MIXED);
    let mut d = Driver::open(&text);
    let members = ["a", "inner", "l", "tri"];
    let before: Vec<_> = members.iter().map(|id| d.corners(id)).collect();
    let undo_before = d.session.history.undo.len();
    let reply = d.ok(
        "gesture.commit",
        json!({ "nodes": members, "dx": 12.5, "dy": -4.0 }),
    );
    assert_eq!(reply["changed"], true, "{reply}");
    assert_eq!(reply["reformatted"], false);
    assert_eq!(ids(&reply["selection"]), members);
    for (id, b) in members.iter().zip(&before) {
        assert_moved(*b, d.corners(id), 12.5, -4.0, id);
    }
    assert_eq!(d.session.history.undo.len(), undo_before + 1, "one entry");
    d.ok("history.undo", json!({}));
    assert_eq!(d.session.text, text, "one undo restores all");
}

#[test]
fn the_selection_is_the_default_and_a_nested_node_rides_along() {
    let mut d = Driver::open(&doc(MIXED));
    d.ok("select.set", json!({ "ids": ["g", "inner", "a"] }));
    let inner = d.corners("inner");
    let g = d.corners("g");
    let reply = d.ok("gesture.commit", json!({ "dx": 5, "dy": 6 }));
    // `inner` moves with `g`: one op for g, one for a.
    let ops = reply["ops"].as_array().expect("ops");
    assert_eq!(ops.len(), 2, "{reply}");
    assert_moved(g, d.corners("g"), 5.0, 6.0, "g");
    assert_moved(inner, d.corners("inner"), 5.0, 6.0, "inner");
}

#[test]
fn preview_names_the_nodes_and_their_corners_without_an_edit() {
    let mut d = Driver::open(&doc(MIXED));
    let text = d.session.text.clone();
    let a = d.corners("a");
    let l = d.corners("l");
    let v = d.ok(
        "gesture.preview",
        json!({ "nodes": ["a", "l"], "dx": 10, "dy": 20, "render": false }),
    );
    assert_eq!(v["nodes"], json!(["a", "l"]));
    assert!(v.get("node").is_none());
    let members = v["members"].as_array().expect("members");
    assert_eq!(members.len(), 2);
    let moved_a = corners_of(&members[0]["corners"]);
    assert_moved(a, moved_a, 10.0, 20.0, "preview a");
    let union = corners_of(&v["corners"]);
    let lx = l.iter().map(|p| p.0).fold(f64::MIN, f64::max);
    assert!(near(union[2], (lx + 10.0, union[2].1), 1e-6), "{v}");
    assert_eq!(d.session.text, text);
}

#[test]
fn any_refusing_node_refuses_the_whole_gesture_with_merged_offers() {
    let text = doc(r#"      rect id="a" x=(px)10 y=(px)10 w=(px)50 h=(px)40
      rect id="b" x=(token)"size.x" y=(px)100 w=(px)50 h=(px)40
      rect id="c" anchor="bottom-right" w=(px)40 h=(px)20
      rect id="locked" x=(px)300 y=(px)10 w=(px)20 h=(px)20 locked=#true"#);
    let mut d = Driver::open(&text);
    let e = d.err(
        "gesture.commit",
        json!({ "nodes": ["a", "b", "c"], "dx": 5 }),
    );
    assert_eq!(e.code, "editor.rejected");
    assert_eq!(codes(&e), vec!["tx.token_bound", "tx.anchored"]);
    assert_eq!(offer_ids(&e), vec!["detach", "detach_anchor"]);
    // An offer resends the whole gesture with its flag: both offers go.
    let detach = e.offers[0].clone();
    assert_eq!(detach.params["nodes"], json!(["a", "b", "c"]));
    let e2 = d.err(&detach.command, detach.params.clone());
    assert_eq!(codes(&e2), vec!["tx.anchored"]);
    let mut both = detach.params.clone();
    both["detach_anchor"] = json!(true);
    d.ok("gesture.commit", both);
    assert!(
        d.session.text.contains(r#"rect id="b" x=(px)35"#),
        "{}",
        d.session.text
    );
    // A locked node refuses first, with its unlock offer.
    let e = d.err(
        "gesture.commit",
        json!({ "nodes": ["a", "locked"], "dx": 5 }),
    );
    assert_eq!(e.code, "editor.locked");
    assert_eq!(offer_ids(&e), vec!["unlock"]);
    // Point handles belong to one node.
    let e = d.err(
        "gesture.commit",
        json!({ "nodes": ["a", "b"], "handle": "p0", "dx": 5 }),
    );
    assert_eq!(e.code, "editor.unknown_handle");
}

#[test]
fn resize_maps_every_node_through_the_selection_box() {
    let mut d = Driver::open(&doc(
        r#"      rect id="a" x=(px)100 y=(px)100 w=(px)50 h=(px)50
      rect id="b" x=(px)200 y=(px)150 w=(px)100 h=(px)50
      line id="l" x1=(px)100 y1=(px)250 x2=(px)300 y2=(px)250 stroke=(token)"color.ink""#,
    ));
    // Selection box: x 100..300, y 100..250 (the line's box has no height
    // in the drawn box: its bounds come from its paint, about 1 px thick).
    let union = d.ok("node.handles", json!({ "ids": ["a", "b", "l"] }));
    let [tl, _, br, _] = corners_of(&union["corners"]);
    let (w, h) = (br.0 - tl.0, br.1 - tl.1);
    // Drag the se grip by (+w/2, +h): x scales by 1.5, y by 2 about tl.
    d.ok(
        "gesture.commit",
        json!({ "nodes": ["a", "b", "l"], "handle": "se", "dx": w / 2.0, "dy": h }),
    );
    let map = |p: (f64, f64)| (tl.0 + (p.0 - tl.0) * 1.5, tl.1 + (p.1 - tl.1) * 2.0);
    for (id, before) in [
        (
            "a",
            [
                (100.0, 100.0),
                (150.0, 100.0),
                (150.0, 150.0),
                (100.0, 150.0),
            ],
        ),
        (
            "b",
            [
                (200.0, 150.0),
                (300.0, 150.0),
                (300.0, 200.0),
                (200.0, 200.0),
            ],
        ),
    ] {
        let after = d.corners(id);
        for (b, a) in before.iter().zip(after.iter()) {
            assert!(near(map(*b), *a, 1e-6), "{id}: {b:?} -> {a:?}");
        }
    }
    let text = &d.session.text;
    let (x1, y1) = map((100.0, 250.0));
    let (x2, _) = map((300.0, 250.0));
    let line = text
        .lines()
        .find(|l| l.contains(r#"line id="l""#))
        .expect("line");
    assert!(
        line.contains(&format!("x1=(px){x1} y1=(px){y1} x2=(px){x2} y2=(px){y1}")),
        "{line}"
    );
}

#[test]
fn rotate_turns_every_node_about_the_selection_centre() {
    let mut d = Driver::open(&doc(
        r#"      rect id="a" x=(px)100 y=(px)100 w=(px)40 h=(px)40
      rect id="b" x=(px)200 y=(px)100 w=(px)40 h=(px)40
      line id="l" x1=(px)100 y1=(px)180 x2=(px)240 y2=(px)180 stroke=(token)"color.ink""#,
    ));
    let union = d.ok("node.handles", json!({ "ids": ["a", "b", "l"] }));
    let c = union["center"].as_array().expect("center");
    let pivot = (c[0].as_f64().expect("x"), c[1].as_f64().expect("y"));
    let turn = |p: (f64, f64)| (pivot.0 - (p.1 - pivot.1), pivot.1 + (p.0 - pivot.0));
    let centre_a = (120.0, 120.0);
    d.ok(
        "gesture.commit",
        json!({ "nodes": ["a", "b", "l"], "handle": "rotate", "angle": 80, "snap": 45 }),
    );
    let text = d.session.text.clone();
    assert!(
        text.contains(r#"rect id="a""#) && text.contains("rotate=(deg)90"),
        "{text}"
    );
    let a = d.corners("a");
    let got = ((a[0].0 + a[2].0) / 2.0, (a[0].1 + a[2].1) / 2.0);
    assert!(
        near(got, turn(centre_a), 1e-6),
        "{got:?} vs {:?}",
        turn(centre_a)
    );
    // The line turns by its endpoints.
    let start = turn((100.0, 180.0));
    let line = text
        .lines()
        .find(|l| l.contains(r#"line id="l""#))
        .expect("line");
    assert!(!line.contains("rotate"), "{line}");
    let fmt = |v: f64| {
        let r = (v * 1e6).round() / 1e6;
        format!("{r}")
    };
    let v = d.ok("node.inspect", json!({ "id": "l" }));
    let corners = corners_of(&v["box"]["corners"]);
    let xs: Vec<f64> = corners.iter().map(|p| p.0).collect();
    let min_x = xs.iter().copied().fold(f64::MAX, f64::min);
    assert!(
        (min_x - start.0).abs() < 2.0,
        "line box {corners:?}, start {} ({})",
        fmt(start.0),
        line
    );
}

#[test]
fn selection_handles_cover_every_node_and_name_blocks() {
    let mut d = Driver::open(&doc(
        r#"      rect id="a" x=(px)10 y=(px)20 w=(px)50 h=(px)40
      rect id="b" x=(token)"size.x" y=(px)100 w=(px)50 h=(px)40
      text id="t" x=(px)200 y=(px)10 w=(px)100 h=(px)30 fill=(token)"color.ink" { span "Hi" }"#,
    ));
    let v = d.ok("node.handles", json!({ "ids": ["a", "b"] }));
    assert_eq!(v["kind"], "selection");
    assert_eq!(v["angle"], 0.0);
    let corners = corners_of(&v["corners"]);
    assert!(near(corners[0], (10.0, 20.0), 1e-9) && near(corners[2], (80.0, 140.0), 1e-9));
    let handles = v["handles"].as_array().expect("handles");
    assert_eq!(handles.len(), 9);
    assert!(handles.iter().all(|h| h["enabled"] == false), "{v}");
    let blocked = &v["disabled"][0];
    assert_eq!(blocked["node"], "b");
    assert_eq!(blocked["code"], "tx.token_bound");
    assert_eq!(v["members"].as_array().expect("members").len(), 2);
    // Nothing blocks a and t: every handle is enabled.
    let t = d.ok("node.handles", json!({ "ids": ["a", "t"] }));
    let handles = t["handles"].as_array().expect("handles");
    assert!(handles.iter().all(|h| h["enabled"] == true), "{t}");
    assert!(
        t["disabled"].as_array().expect("disabled").is_empty(),
        "{t}"
    );
    // One id is that node's handles.
    let one = d.ok("node.handles", json!({ "ids": ["a", "a"] }));
    assert_eq!(one["id"], "a");
}

#[test]
fn nodes_must_share_a_page() {
    let text = r##"zenith version=1 {
  project id="proj.t" name="T"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#203040"
  }
  styles {}
  document id="doc.t" title="T" {
    page id="p1" w=(px)200 h=(px)200 {
      rect id="a" x=(px)10 y=(px)10 w=(px)20 h=(px)20
    }
    page id="p2" w=(px)200 h=(px)200 {
      rect id="b" x=(px)10 y=(px)10 w=(px)20 h=(px)20
    }
  }
}
"##;
    let mut d = Driver::open(text);
    let e = d.err("gesture.commit", json!({ "nodes": ["a", "b"], "dx": 1 }));
    assert_eq!(e.code, "editor.mixed_pages");
}

#[test]
fn a_connector_rides_along_with_a_note() {
    let mut d = Driver::open(&doc(
        r#"      rect id="a" x=(px)10 y=(px)10 w=(px)50 h=(px)40
      rect id="b" x=(px)200 y=(px)10 w=(px)50 h=(px)40
      connector id="c" from="a" to="b" stroke=(token)"color.ink""#,
    ));
    let reply = d.ok(
        "gesture.commit",
        json!({ "nodes": ["a", "b", "c"], "dx": 0, "dy": 30 }),
    );
    let notes: Vec<&str> = reply["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .filter_map(|n| n["code"].as_str())
        .collect();
    assert_eq!(notes, vec!["editor.follows_targets"]);
    assert_eq!(reply["ops"].as_array().expect("ops").len(), 2);
}

#[test]
fn duplicate_copies_every_selected_node() {
    let mut d = Driver::open(&doc(
        r#"      rect id="a" x=(px)10 y=(px)10 w=(px)50 h=(px)40
      group id="g" {
        rect id="in" x=(px)100 y=(px)10 w=(px)20 h=(px)20
      }"#,
    ));
    d.ok("select.set", json!({ "ids": ["a", "g", "in"] }));
    let reply = d.ok("node.duplicate", json!({ "dx": 5 }));
    assert_eq!(reply["selection"], json!(["a-copy", "g-copy"]));
    let text = &d.session.text;
    assert!(text.contains(r#"rect id="a-copy" x=(px)15"#), "{text}");
    assert!(text.contains(r#"rect id="in-copy""#), "{text}");
    assert!(
        !text.contains("in-copy2"),
        "nested node copied once: {text}"
    );
    assert_eq!(
        d.err(
            "node.duplicate",
            json!({ "ids": ["a", "g"], "new_id": "x" })
        )
        .code,
        "editor.invalid_params"
    );
}
