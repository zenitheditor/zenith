//! Rejections with offers, and the offers going ahead: token-bound,
//! anchored, computed size, unresolved units, absent x / y, locked,
//! hidden, and layout-managed nodes.

use serde_json::{Value, json};
use zenith_editor::{EditorError, Offer};

use crate::common::{Driver, assert_moved, codes, doc, offer_ids};

/// Send `offer` as its own request and return the reply.
fn accept(d: &mut Driver, offer: &Offer) -> Value {
    d.ok(&offer.command, offer.params.clone())
}

fn offer<'a>(e: &'a EditorError, id: &str) -> &'a Offer {
    e.offers
        .iter()
        .find(|o| o.id == id)
        .unwrap_or_else(|| panic!("no offer {id}: {e:?}"))
}

#[test]
fn token_bound_axis_offers_detach() {
    let mut d = Driver::open(&doc(
        r#"      rect id="r" x=(token)"size.x" y=(px)40 w=(token)"size.w" h=(px)20 fill=(token)"color.ink""#,
    ));
    let before = d.corners("r");
    let e = d.err("gesture.commit", json!({ "node": "r", "dx": 10, "dy": 5 }));
    assert_eq!(e.code, "editor.rejected");
    assert_eq!(codes(&e), vec!["tx.token_bound"]);
    assert_eq!(offer_ids(&e), vec!["detach"]);
    let reply = accept(&mut d, offer(&e, "detach"));
    assert_eq!(reply["changed"], true);
    assert_moved(before, d.corners("r"), 10.0, 5.0, "detached move");
    assert!(
        d.session.text.contains("x=(px)40 y=(px)45"),
        "{}",
        d.session.text
    );
    // The token-bound width disables the east grip.
    let h = d.handle("r", "e");
    assert_eq!(h["reason"], "tx.token_bound");
    // A vertical move does not touch the token-bound w.
    let ok = d.ok("gesture.commit", json!({ "node": "r", "dy": 3 }));
    assert_eq!(ok["changed"], true);
}

#[test]
fn other_anchors_offer_detach_anchor_and_edge_anchors_move_the_gap() {
    let mut d = Driver::open(&doc(
        r#"      rect id="base" x=(px)20 y=(px)20 w=(px)100 h=(px)40 fill=(token)"color.ink"
      rect id="below" anchor-sibling="base" anchor-edge="below" anchor-gap=(px)10 w=(px)100 h=(px)20 fill=(token)"color.ink"
      rect id="corner" anchor="bottom-right" w=(px)40 h=(px)20 fill=(token)"color.ink""#,
    ));
    let before = d.corners("below");
    let r = d.ok(
        "gesture.commit",
        json!({ "node": "below", "dx": 7, "dy": 15 }),
    );
    assert_eq!(r["ops"][0]["op"], "nudge_anchor_gap");
    assert_eq!(r["notes"][0]["code"], "editor.axis_dropped");
    assert!(
        d.session.text.contains("anchor-gap=(px)25"),
        "{}",
        d.session.text
    );
    assert_moved(before, d.corners("below"), 0.0, 15.0, "gap move");

    let corner = d.corners("corner");
    let e = d.err(
        "gesture.commit",
        json!({ "node": "corner", "dx": -30, "dy": -10 }),
    );
    assert_eq!(codes(&e), vec!["tx.anchored", "tx.anchored"]);
    assert_eq!(offer_ids(&e), vec!["detach_anchor"]);
    let reply = accept(&mut d, offer(&e, "detach_anchor"));
    assert_eq!(reply["ops"][0]["op"], "detach_anchor");
    assert_eq!(reply["ops"][1]["op"], "nudge_geometry");
    assert_moved(corner, d.corners("corner"), -30.0, -10.0, "detached anchor");
}

#[test]
fn computed_size_offers_set_size() {
    let mut d = Driver::open(&doc(
        r#"      frame id="col" x=(px)20 y=(px)20 w=(px)300 h=(px)200 layout="column" {
        text id="t" w=(px)200 fill=(token)"color.ink" { span "Hello" }
      }"#,
    ));
    assert!(d.session.valid);
    let nw = d.handle("t", "nw");
    let e = d.err(
        "gesture.commit",
        json!({ "node": "t", "handle": "se", "dx": 10, "dy": 30 }),
    );
    assert_eq!(codes(&e), vec!["tx.computed_size"]);
    let reply = accept(&mut d, offer(&e, "set_size"));
    assert_eq!(reply["ops"][0]["op"], "set_geometry");
    assert!(d.session.text.contains("w=(px)210"), "{}", d.session.text);
    assert!(d.session.text.contains(" h=(px)"), "{}", d.session.text);
    let now = d.handle("t", "nw");
    assert_eq!((&now["x"], &now["y"]), (&nw["x"], &nw["y"]));
}

#[test]
fn undrawn_units_and_absent_group_xy() {
    let mut d = Driver::open(&doc(
        r#"      rect id="pct" x=(pct)10 y=(px)10 w=(px)50 h=(px)50 fill=(token)"color.ink"
      group id="bare" {
        rect id="in" x=(px)5 y=(px)5 w=(px)50 h=(px)50 fill=(token)"color.ink"
      }"#,
    ));
    assert!(d.session.valid);
    // The scene draws no box for a pct x, so the canvas cannot edit it.
    let e = d.err("gesture.commit", json!({ "node": "pct", "dx": 5 }));
    assert_eq!(e.code, "editor.no_box", "{e:?}");
    // A group without x / y sits at 0: the move writes them.
    let inner = d.corners("in");
    let r = d.ok(
        "gesture.commit",
        json!({ "node": "bare", "dx": 12, "dy": 8 }),
    );
    assert_eq!(r["ops"][0]["op"], "nudge_geometry");
    assert!(
        d.session
            .text
            .contains("group id=\"bare\" x=(px)12 y=(px)8"),
        "{}",
        d.session.text
    );
    assert_moved(inner, d.corners("in"), 12.0, 8.0, "absent x/y written");
}

#[test]
fn locked_and_hidden_nodes_offer_unlock_and_show() {
    let mut d = Driver::open(&doc(
        r#"      rect id="lk" x=(px)10 y=(px)10 w=(px)20 h=(px)20 locked=#true fill=(token)"color.ink"
      group id="lg" locked=#true {
        rect id="inner" x=(px)50 y=(px)10 w=(px)20 h=(px)20 fill=(token)"color.ink"
      }
      rect id="hd" x=(px)90 y=(px)10 w=(px)20 h=(px)20 visible=#false fill=(token)"color.ink""#,
    ));
    let e = d.err("gesture.commit", json!({ "node": "lk", "dx": 5 }));
    assert_eq!(e.code, "editor.locked");
    assert_eq!(e.offers[0].params["ops"][0]["node"], "lk");
    let inner = d.err("gesture.commit", json!({ "node": "inner", "dx": 5 }));
    assert_eq!(inner.code, "editor.locked");
    assert_eq!(inner.offers[0].params["ops"][0]["node"], "lg");
    let handles = d.ok("node.handles", json!({ "id": "lk" }));
    assert!(
        handles["handles"]
            .as_array()
            .expect("handles")
            .iter()
            .all(|h| h["enabled"] == false),
        "{handles}"
    );
    let hidden = d.err("gesture.commit", json!({ "node": "hd", "dx": 5 }));
    assert_eq!(hidden.code, "editor.hidden");
    assert_eq!(offer_ids(&hidden), vec!["show"]);
    // tx.apply on a locked node is rejected by the engine and offers unlock.
    let tx = d.err(
        "tx.apply",
        json!({ "ops": [{ "op": "set_opacity", "node": "lk", "opacity": 0.5 }] }),
    );
    assert_eq!(tx.code, "editor.rejected");
    assert_eq!(codes(&tx), vec!["node.locked"]);
    assert_eq!(offer_ids(&tx), vec!["unlock"]);
    accept(&mut d, &e.offers[0]);
    let moved = d.ok("gesture.commit", json!({ "node": "lk", "dx": 5 }));
    assert_eq!(moved["changed"], true);
}

const ROW: &str = r#"      frame id="row" x=(px)20 y=(px)20 w=(px)360 h=(px)60 layout="row" gap=(px)10 align="start" {
        rect id="a" w=(px)50 h=(px)40 fill=(token)"color.ink"
        rect id="b" w=(px)50 h=(px)40 fill=(token)"color.ink"
        rect id="c" w=(px)50 h=(px)40 fill=(token)"color.ink"
      }"#;

#[test]
fn in_flow_children_offer_reorder_and_absolute() {
    let mut d = Driver::open(&doc(ROW));
    let e = d.err("gesture.commit", json!({ "node": "a", "dx": 130, "dy": 0 }));
    assert_eq!(codes(&e), vec!["tx.layout_managed"]);
    assert_eq!(offer_ids(&e), vec!["reorder", "absolute"]);
    let reply = accept(&mut d, offer(&e, "reorder"));
    assert_eq!(reply["ops"][0]["op"], "reparent", "{reply}");
    let order: Vec<usize> = ["b", "c", "a"]
        .iter()
        .map(|id| d.session.text.find(&format!("rect id=\"{id}\"")).expect(id))
        .collect();
    assert!(
        order[0] < order[1] && order[1] < order[2],
        "{}",
        d.session.text
    );
    // The same slot is no change.
    let same = d.ok(
        "gesture.commit",
        json!({ "node": "b", "dx": 2, "reorder": true }),
    );
    assert_eq!(same["changed"], false);
    // Take one out of the layout: it keeps its page box, then moves.
    let before = d.corners("c");
    let e = d.err("gesture.commit", json!({ "node": "c", "dx": 0, "dy": 100 }));
    let reply = accept(&mut d, offer(&e, "absolute"));
    assert_eq!(reply["ops"][0]["op"], "set_layout");
    assert_moved(before, d.corners("c"), 0.0, 100.0, "absolute");
    // Resizing an in-flow child is fine; its x / y shift is dropped.
    let r = d.ok(
        "gesture.commit",
        json!({ "node": "a", "handle": "e", "dx": 10 }),
    );
    assert_eq!(r["changed"], true);
    let handles = d.ok("node.handles", json!({ "id": "a" }));
    assert!(
        handles["disabled"]
            .as_array()
            .expect("disabled")
            .iter()
            .any(|b| b["code"] == "tx.layout_managed"),
        "{handles}"
    );
}
