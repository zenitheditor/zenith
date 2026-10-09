//! Line, polygon / polyline, and path gestures.

use serde_json::{Value, json};

use crate::common::{Driver, doc, near};

fn handles(d: &mut Driver, id: &str) -> Value {
    d.ok("node.handles", json!({ "id": id }))
}

fn at(v: &Value, handle: &str) -> (f64, f64) {
    let h = v["handles"]
        .as_array()
        .expect("handles")
        .iter()
        .find(|h| h["id"] == handle)
        .unwrap_or_else(|| panic!("no {handle}: {v}"));
    (h["x"].as_f64().expect("x"), h["y"].as_f64().expect("y"))
}

fn center(v: &Value) -> (f64, f64) {
    (
        v["center"][0].as_f64().expect("x"),
        v["center"][1].as_f64().expect("y"),
    )
}

fn moved(a: (f64, f64), dx: f64, dy: f64) -> (f64, f64) {
    (a.0 + dx, a.1 + dy)
}

#[test]
fn line_moves_and_drags_endpoints() {
    let mut d = Driver::example("line.zen");
    let h0 = handles(&mut d, "line.divider");
    let (s0, e0) = (at(&h0, "start"), at(&h0, "end"));
    let r = d.ok(
        "gesture.commit",
        json!({ "node": "line.divider", "dx": 5, "dy": -20 }),
    );
    assert_eq!(r["ops"][0]["op"], "nudge_line_points");
    let h1 = handles(&mut d, "line.divider");
    assert!(near(at(&h1, "start"), moved(s0, 5.0, -20.0), 1e-9));
    assert!(near(at(&h1, "end"), moved(e0, 5.0, -20.0), 1e-9));
    d.ok(
        "gesture.commit",
        json!({ "node": "line.divider", "handle": "end", "dx": -30, "dy": 40 }),
    );
    let h2 = handles(&mut d, "line.divider");
    assert!(
        near(at(&h2, "start"), at(&h1, "start"), 1e-9),
        "start stays"
    );
    assert!(near(
        at(&h2, "end"),
        moved(at(&h1, "end"), -30.0, 40.0),
        1e-9
    ));
    let e = d.err(
        "gesture.commit",
        json!({ "node": "line.divider", "handle": "se", "dx": 1 }),
    );
    assert_eq!(e.code, "editor.unknown_handle");
    let rot = d.err(
        "gesture.commit",
        json!({ "node": "line.divider", "handle": "rotate", "angle": 5 }),
    );
    assert_eq!(rot.code, "editor.unsupported");
}

#[test]
fn polygon_moves_resizes_and_drags_vertices() {
    let mut d = Driver::example("polygon.zen");
    let id = "poly.tri";
    let h0 = handles(&mut d, id);
    let r = d.ok("gesture.commit", json!({ "node": id, "dx": 10, "dy": 10 }));
    assert_eq!(r["ops"][0]["op"], "set_points");
    let h1 = handles(&mut d, id);
    for p in ["p0", "p1", "p2"] {
        assert!(near(at(&h1, p), moved(at(&h0, p), 10.0, 10.0), 1e-9), "{p}");
    }
    let nw = at(&h1, "nw");
    d.ok(
        "gesture.commit",
        json!({ "node": id, "handle": "se", "dx": 50, "dy": 25 }),
    );
    let h2 = handles(&mut d, id);
    assert!(near(at(&h2, "nw"), nw, 1e-9), "nw fixed");
    assert!(near(at(&h2, "se"), moved(at(&h1, "se"), 50.0, 25.0), 1e-9));
    d.ok(
        "gesture.commit",
        json!({ "node": id, "handle": "p0", "dx": -5, "dy": 7 }),
    );
    let h3 = handles(&mut d, id);
    assert!(near(at(&h3, "p0"), moved(at(&h2, "p0"), -5.0, 7.0), 1e-9));
    assert!(near(at(&h3, "p1"), at(&h2, "p1"), 1e-9));
}

#[test]
fn rotated_polygon_vertex_drag_keeps_the_others() {
    let mut d = Driver::open(&doc(
        r#"      polygon id="tri" rotate=(deg)40 fill=(token)"color.ink" {
        point x=(px)100 y=(px)100
        point x=(px)200 y=(px)100
        point x=(px)150 y=(px)180
      }"#,
    ));
    let h0 = handles(&mut d, "tri");
    d.ok(
        "gesture.commit",
        json!({ "node": "tri", "handle": "p2", "dx": 20, "dy": 15 }),
    );
    let h1 = handles(&mut d, "tri");
    assert!(near(at(&h1, "p0"), at(&h0, "p0"), 1e-6));
    assert!(near(at(&h1, "p1"), at(&h0, "p1"), 1e-6));
    assert!(near(at(&h1, "p2"), moved(at(&h0, "p2"), 20.0, 15.0), 1e-6));
    // A resize keeps the opposite grip.
    let sw = at(&h1, "sw");
    d.ok(
        "gesture.commit",
        json!({ "node": "tri", "handle": "ne", "dx": 12, "dy": -9 }),
    );
    let h2 = handles(&mut d, "tri");
    assert!(near(at(&h2, "sw"), sw, 1e-6));
    // Rotate about the bounds centre.
    d.ok(
        "gesture.commit",
        json!({ "node": "tri", "handle": "rotate", "angle": 20 }),
    );
    let h3 = handles(&mut d, "tri");
    assert!(near(center(&h2), center(&h3), 1e-6));
    assert!((h3["angle"].as_f64().expect("angle") - 60.0).abs() < 1e-9);
}

#[test]
fn polyline_moves() {
    let mut d = Driver::example("polyline.zen");
    let h0 = handles(&mut d, "poly.connector");
    d.ok(
        "gesture.commit",
        json!({ "node": "poly.connector", "dx": -12, "dy": 3 }),
    );
    let h1 = handles(&mut d, "poly.connector");
    assert!(near(at(&h1, "p3"), moved(at(&h0, "p3"), -12.0, 3.0), 1e-9));
}

const PATH: &str = r#"      path id="pa" rotate=(deg)30 stroke=(token)"color.ink" {
        anchor x=(px)100 y=(px)100 out-x=(px)130 out-y=(px)80
        anchor x=(px)200 y=(px)100 in-x=(px)170 in-y=(px)80
        anchor x=(px)150 y=(px)200
      }"#;

#[test]
fn path_moves_resizes_and_edits_anchors() {
    let mut d = Driver::open(&doc(PATH));
    let h0 = handles(&mut d, "pa");
    let r = d.ok("gesture.commit", json!({ "node": "pa", "dx": 7, "dy": -4 }));
    assert_eq!(r["ops"][0]["transform"]["mode"], "translate");
    let h1 = handles(&mut d, "pa");
    for h in ["a0.0", "a0.1", "a0.2", "a0.0.out", "a0.1.in"] {
        assert!(near(at(&h1, h), moved(at(&h0, h), 7.0, -4.0), 1e-6), "{h}");
    }
    let nw = at(&h1, "nw");
    let r = d.ok(
        "gesture.commit",
        json!({ "node": "pa", "handle": "se", "dx": 30, "dy": 10 }),
    );
    assert_eq!(r["ops"][0]["transform"]["mode"], "scale", "{r}");
    let h2 = handles(&mut d, "pa");
    assert!(near(at(&h2, "nw"), nw, 1e-6), "nw fixed under rotation");
    d.ok(
        "gesture.commit",
        json!({ "node": "pa", "handle": "a0.2", "dx": 5, "dy": 5 }),
    );
    let h3 = handles(&mut d, "pa");
    assert!(
        near(at(&h3, "a0.0"), at(&h2, "a0.0"), 1e-6),
        "other anchors stay"
    );
    assert!(near(
        at(&h3, "a0.2"),
        moved(at(&h2, "a0.2"), 5.0, 5.0),
        1e-6
    ));
    d.ok(
        "gesture.commit",
        json!({ "node": "pa", "handle": "a0.0.out", "dx": 4, "dy": 0 }),
    );
    let h4 = handles(&mut d, "pa");
    assert!(near(
        at(&h4, "a0.0.out"),
        moved(at(&h3, "a0.0.out"), 4.0, 0.0),
        1e-6
    ));
    assert!(near(at(&h4, "a0.0"), at(&h3, "a0.0"), 1e-6));
    let e = d.err(
        "gesture.commit",
        json!({ "node": "pa", "handle": "a0.2.in", "dx": 1 }),
    );
    assert_eq!(e.code, "editor.unknown_handle", "a0.2 has no in handle");
}

#[test]
fn connector_is_derived() {
    let mut d = Driver::example("flowchart.zen");
    let outline = d.ok("doc.outline", json!({}));
    let connector = outline["pages"][0]["children"]
        .as_array()
        .expect("children")
        .iter()
        .find(|c| c["kind"] == "connector")
        .expect("a connector")["id"]
        .as_str()
        .expect("id")
        .to_owned();
    let e = d.err("gesture.commit", json!({ "node": connector, "dx": 5 }));
    assert_eq!(e.code, "editor.rejected");
    assert_eq!(crate::common::codes(&e), vec!["tx.derived_geometry"]);
    let r = d.err(
        "gesture.commit",
        json!({ "node": connector, "handle": "rotate", "angle": 5 }),
    );
    assert_eq!(r.code, "editor.rejected");
    assert_eq!(crate::common::codes(&r), vec!["tx.derived_geometry"]);
    let h = d.ok("node.handles", json!({ "id": connector }));
    assert!(h["handles"].as_array().expect("handles").is_empty(), "{h}");
}
