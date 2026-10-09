//! Move, resize, and rotate on box kinds: rect, rotated rect, ellipse,
//! text, frame (with children), and image.

use serde_json::json;

use crate::common::{Driver, assert_moved, corners_of, doc, near};

fn handle_at(d: &mut Driver, id: &str, handle: &str) -> (f64, f64) {
    let h = d.handle(id, handle);
    (h["x"].as_f64().expect("x"), h["y"].as_f64().expect("y"))
}

fn move_by(d: &mut Driver, id: &str, dx: f64, dy: f64) {
    let before = d.corners(id);
    let reply = d.ok("gesture.commit", json!({ "node": id, "dx": dx, "dy": dy }));
    assert_eq!(reply["changed"], true, "{reply}");
    assert_eq!(reply["reformatted"], false, "{reply}");
    let after = d.corners(id);
    assert_moved(before, after, dx, dy, id);
}

#[test]
fn move_box_kinds_by_exact_page_deltas() {
    let mut d = Driver::open(&doc(
        r#"      rect id="r" x=(px)10 y=(px)10 w=(px)50 h=(px)40 fill=(token)"color.ink"
      rect id="rot" x=(px)100 y=(px)100 w=(px)80 h=(px)30 rotate=(deg)35 fill=(token)"color.ink"
      ellipse id="e" x=(pt)150 y=(px)20 w=(px)40 h=(px)40 fill=(token)"color.ink"
      text id="t" x=(px)20 y=(px)200 w=(px)200 h=(px)40 fill=(token)"color.ink" { span "Hi" }"#,
    ));
    move_by(&mut d, "r", 12.5, -3.0);
    move_by(&mut d, "rot", -7.0, 11.0);
    move_by(&mut d, "e", 3.0, 4.0);
    move_by(&mut d, "t", 0.25, 30.0);
    // Units stay: the pt value is still pt.
    assert!(
        d.session.text.contains("ellipse id=\"e\" x=(pt)"),
        "{}",
        d.session.text
    );
}

#[test]
fn move_frame_and_image_examples() {
    let mut d = Driver::example("frame.zen");
    let inside = d.corners("rect.inside");
    move_by(&mut d, "frame.clip", 20.0, 10.0);
    assert_moved(
        inside,
        d.corners("rect.inside"),
        20.0,
        10.0,
        "child follows",
    );
    let mut img = Driver::example("image.zen");
    move_by(&mut img, "img.swatch", -15.0, 22.0);
}

#[test]
fn move_inside_rotated_group_maps_through_the_parent() {
    let mut d = Driver::open(&doc(
        r#"      group id="g" x=(px)100 y=(px)50 w=(px)200 h=(px)200 rotate=(deg)30 {
        rect id="r" x=(px)10 y=(px)10 w=(px)60 h=(px)20 fill=(token)"color.ink"
      }"#,
    ));
    move_by(&mut d, "r", 15.0, 5.0);
}

#[test]
fn resize_keeps_the_opposite_handle_on_the_page() {
    let mut d = Driver::open(&doc(
        r#"      rect id="r" x=(px)100 y=(px)80 w=(px)120 h=(px)60 rotate=(deg)25 fill=(token)"color.ink""#,
    ));
    for (handle, opposite) in [
        ("se", "nw"),
        ("nw", "se"),
        ("e", "w"),
        ("n", "s"),
        ("sw", "ne"),
    ] {
        let fixed = handle_at(&mut d, "r", opposite);
        let reply = d.ok(
            "gesture.commit",
            json!({ "node": "r", "handle": handle, "dx": 9.0, "dy": -6.0 }),
        );
        assert_eq!(reply["changed"], true, "{handle}: {reply}");
        let now = handle_at(&mut d, "r", opposite);
        assert!(near(fixed, now, 1e-6), "{handle}: {fixed:?} -> {now:?}");
    }
}

#[test]
fn resize_unrotated_east_edge_changes_only_w() {
    let mut d = Driver::open(&doc(
        r#"      rect id="r" x=(token)"size.x" y=(px)80 w=(px)120 h=(px)60 fill=(token)"color.ink""#,
    ));
    let reply = d.ok(
        "gesture.commit",
        json!({ "node": "r", "handle": "e", "dx": 10.0, "dy": 3.0 }),
    );
    assert_eq!(
        reply["ops"],
        json!([{ "op": "nudge_geometry", "node": "r", "dw": 10.0, "detach": false }])
    );
    assert!(d.session.text.contains("x=(token)\"size.x\""), "token kept");
    assert!(d.session.text.contains("w=(px)130"));
}

#[test]
fn constrain_and_from_center_resize() {
    let mut d = Driver::open(&doc(
        r#"      rect id="r" x=(px)100 y=(px)100 w=(px)100 h=(px)50 fill=(token)"color.ink""#,
    ));
    d.ok(
        "gesture.commit",
        json!({ "node": "r", "handle": "se", "dx": 50.0, "dy": 0.0, "constrain": true }),
    );
    assert!(
        d.session.text.contains("w=(px)150 h=(px)75"),
        "{}",
        d.session.text
    );
    let centre = |d: &mut Driver| {
        let v = d.ok("node.handles", json!({ "id": "r" }));
        (
            v["center"][0].as_f64().expect("x"),
            v["center"][1].as_f64().expect("y"),
        )
    };
    let c0 = centre(&mut d);
    d.ok(
        "gesture.commit",
        json!({ "node": "r", "handle": "e", "dx": 10.0, "from_center": true }),
    );
    assert!(near(c0, centre(&mut d), 1e-9));
    assert!(d.session.text.contains("w=(px)170"), "{}", d.session.text);
}

#[test]
fn rotate_about_the_centre_with_snap() {
    let mut d = Driver::open(&doc(
        r#"      rect id="r" x=(px)100 y=(px)100 w=(px)100 h=(px)50 fill=(token)"color.ink""#,
    ));
    let before = d.ok("node.handles", json!({ "id": "r" }));
    d.ok(
        "gesture.commit",
        json!({ "node": "r", "handle": "rotate", "angle": 28.0, "snap": 15.0 }),
    );
    assert!(
        d.session.text.contains("rotate=(deg)30"),
        "{}",
        d.session.text
    );
    let after = d.ok("node.handles", json!({ "id": "r" }));
    assert_eq!(before["center"], after["center"]);
    assert!((after["angle"].as_f64().expect("angle") - 30.0).abs() < 1e-9);
    let text = d.ok(
        "gesture.commit",
        json!({ "node": "r", "handle": "rotate", "angle": -30.0 }),
    );
    assert_eq!(text["changed"], true);
    assert!(
        !d.session.text.contains("rotate="),
        "0 removes the attribute"
    );
}

#[test]
fn text_without_h_does_not_rotate() {
    let mut d = Driver::open(&doc(
        r#"      frame id="col" x=(px)20 y=(px)20 w=(px)300 h=(px)200 layout="column" {
        text id="t" w=(px)200 fill=(token)"color.ink" { span "Hi" }
      }"#,
    ));
    assert!(d.session.valid);
    let e = d.err(
        "gesture.commit",
        json!({ "node": "t", "handle": "rotate", "angle": 10.0 }),
    );
    assert_eq!(e.code, "editor.rotate_needs_h");
    let h = d.ok("node.handles", json!({ "id": "t" }));
    let rotate = h["handles"]
        .as_array()
        .expect("handles")
        .iter()
        .find(|x| x["id"] == "rotate")
        .expect("rotate handle");
    assert_eq!(rotate["enabled"], false);
    assert_eq!(rotate["reason"], "editor.rotate_needs_h");
    // Its height is computed: a bottom resize needs confirmation.
    let s = h["handles"]
        .as_array()
        .expect("handles")
        .iter()
        .find(|x| x["id"] == "s")
        .expect("s");
    assert_eq!(s["reason"], "tx.computed_size");
}

#[test]
fn preview_matches_commit_and_leaves_the_session() {
    let mut d = Driver::open(&doc(
        r#"      rect id="r" x=(px)10 y=(px)10 w=(px)50 h=(px)40 rotate=(deg)10 fill=(token)"color.ink""#,
    ));
    let before = d.session.clone();
    let preview = d.outcome(
        "gesture.preview",
        json!({ "node": "r", "handle": "se", "dx": 20.0, "dy": 5.0, "scale": 1.0 }),
    );
    assert_eq!(d.session, before, "preview leaves the session");
    let reply = preview.result.expect("preview");
    let png = preview.image.expect("png").png;
    let ghost = corners_of(&reply["corners"]);
    let commit = d.ok(
        "gesture.commit",
        json!({ "node": "r", "handle": "se", "dx": 20.0, "dy": 5.0 }),
    );
    assert_eq!(reply["ops"], commit["ops"]);
    let after = d.corners("r");
    for (a, b) in ghost.iter().zip(after.iter()) {
        assert!(near(*a, *b, 1e-9));
    }
    let render = d.outcome("doc.render", json!({ "scale": 1.0 }));
    assert_eq!(
        render.image.expect("png").png,
        png,
        "preview raster = render"
    );
    let quiet = d.ok(
        "gesture.preview",
        json!({ "node": "r", "dx": 1.0, "render": false }),
    );
    assert!(quiet.get("sha256").is_none());
}

const MASTER_DOC: &str = r##"zenith version=1 {
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
    page id="p2" w=(px)400 h=(px)300 master="m" {
    }
  }
}
"##;

#[test]
fn master_content_and_instances_move_and_resize() {
    let mut d = Driver::open(MASTER_DOC);
    move_by(&mut d, "band", 0.0, 20.0);
    assert!(d.session.text.contains("rect id=\"band\" x=(px)0 y=(px)20"));
    // The second page draws the moved master too.
    d.ok("view.set", json!({ "page": 2 }));
    let hit = d.ok("select.hit", json!({ "x": 10, "y": 30 }));
    assert_eq!(hit["hits"][0]["raw_id"], "p2/band");
    move_by(&mut d, "inst", 10.0, -10.0);
    let nw = handle_at(&mut d, "inst", "nw");
    d.ok(
        "gesture.commit",
        json!({ "node": "inst", "handle": "se", "dx": 50, "dy": 50 }),
    );
    assert!(
        d.session.text.contains("w=(px)100 h=(px)100"),
        "{}",
        d.session.text
    );
    assert!(near(handle_at(&mut d, "inst", "nw"), nw, 1e-9));
    let e = d.err(
        "gesture.commit",
        json!({ "node": "inst", "handle": "rotate", "angle": 10 }),
    );
    assert_eq!(e.code, "editor.unsupported");
}

#[test]
fn constrained_move_keeps_the_dominant_axis() {
    let mut d = Driver::open(&doc(
        r#"      rect id="r" x=(px)10 y=(px)10 w=(px)50 h=(px)40 fill=(token)"color.ink""#,
    ));
    let before = d.corners("r");
    let r = d.ok(
        "gesture.commit",
        json!({ "node": "r", "dx": 10, "dy": 3, "constrain": true }),
    );
    assert_eq!(r["ops"][0]["dx"], 10.0);
    assert!(r["ops"][0].get("dy").is_none(), "{r}");
    assert_moved(before, d.corners("r"), 10.0, 0.0, "constrained");
}
