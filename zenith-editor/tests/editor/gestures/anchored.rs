//! Selection gestures over members whose position derives from another
//! member: anchor chains, a container with its descendant. The anchor
//! carries such a member, so it gets no delta of its own on the anchored
//! axes.

use serde_json::{Value, json};

use crate::common::{Driver, assert_moved, doc};

/// `a` placed in px; `b` anchored below `a` with an 8 px gap; `c`
/// anchored after `b`; `free` unanchored.
const CHAIN: &str = r#"      rect id="a" x=(px)20 y=(px)20 w=(px)60 h=(px)40
      rect id="b" anchor-sibling="a" anchor-edge="below" anchor-gap=(px)8 w=(px)60 h=(px)20
      rect id="c" anchor-sibling="b" anchor-edge="after" anchor-gap=(px)4 w=(px)30 h=(px)20
      rect id="free" x=(px)200 y=(px)20 w=(px)40 h=(px)40"#;

fn line_of<'t>(text: &'t str, id: &str) -> &'t str {
    let key = format!(r#"id="{id}""#);
    text.lines()
        .find(|l| l.contains(&key))
        .unwrap_or_else(|| panic!("no {id} in {text}"))
}

fn note_codes(reply: &Value) -> Vec<String> {
    reply["notes"]
        .as_array()
        .expect("notes")
        .iter()
        .filter_map(|n| n["code"].as_str().map(str::to_owned))
        .collect()
}

fn op_nodes(reply: &Value) -> Vec<String> {
    reply["ops"]
        .as_array()
        .expect("ops")
        .iter()
        .filter_map(|o| o["node"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn moving_a_node_and_its_anchored_sibling_moves_both_once() {
    let mut d = Driver::open(&doc(CHAIN));
    let before = [d.corners("a"), d.corners("b"), d.corners("c")];
    let reply = d.ok(
        "gesture.commit",
        json!({ "nodes": ["a", "b"], "dx": 5, "dy": 10 }),
    );
    assert_eq!(op_nodes(&reply), vec!["a"], "{reply}");
    assert!(
        note_codes(&reply).contains(&"editor.follows_anchor".to_owned()),
        "{reply}"
    );
    for (id, b) in ["a", "b", "c"].iter().zip(before) {
        assert_moved(b, d.corners(id), 5.0, 10.0, id);
    }
    let text = d.session.text.clone();
    assert!(line_of(&text, "b").contains("anchor-gap=(px)8"), "{text}");
    assert!(!line_of(&text, "b").contains(" x="), "{text}");
}

#[test]
fn an_anchor_chain_through_an_unselected_sibling_still_follows() {
    let mut d = Driver::open(&doc(CHAIN));
    let before = d.corners("c");
    let reply = d.ok(
        "gesture.commit",
        json!({ "nodes": ["c", "a"], "dx": -3, "dy": 7 }),
    );
    assert_eq!(op_nodes(&reply), vec!["a"], "{reply}");
    assert_moved(before, d.corners("c"), -3.0, 7.0, "c");
    assert!(line_of(&d.session.text, "c").contains("anchor-gap=(px)4"));
}

#[test]
fn an_authored_axis_of_a_follower_still_moves() {
    let mut d = Driver::open(&doc(
        r#"      rect id="a" x=(px)20 y=(px)20 w=(px)60 h=(px)40
      rect id="b" anchor-sibling="a" anchor-edge="below" x=(px)100 w=(px)60 h=(px)20"#,
    ));
    let before = d.corners("b");
    let reply = d.ok(
        "gesture.commit",
        json!({ "nodes": ["a", "b"], "dx": 5, "dy": 10 }),
    );
    assert_eq!(op_nodes(&reply), vec!["a", "b"], "{reply}");
    assert_moved(before, d.corners("b"), 5.0, 10.0, "b");
    assert!(line_of(&d.session.text, "b").contains("x=(px)105"));
}

#[test]
fn a_keyboard_nudge_with_detach_keeps_the_follower_anchored() {
    let mut d = Driver::open(&doc(CHAIN));
    let before = d.corners("b");
    d.ok(
        "gesture.commit",
        json!({ "nodes": ["a", "b"], "dx": 0, "dy": 1, "detach": true, "detach_anchor": true }),
    );
    assert_moved(before, d.corners("b"), 0.0, 1.0, "b");
    let text = d.session.text.clone();
    assert!(
        line_of(&text, "b").contains(r#"anchor-sibling="a""#),
        "{text}"
    );
    assert!(line_of(&text, "b").contains("anchor-gap=(px)8"), "{text}");
}

#[test]
fn a_selection_resize_leaves_the_anchored_position_to_the_anchor() {
    let mut d = Driver::open(&doc(CHAIN));
    let union = d.ok("node.handles", json!({ "ids": ["a", "b"] }));
    assert!(
        union["handles"]
            .as_array()
            .expect("handles")
            .iter()
            .all(|h| h["enabled"] == true),
        "{union}"
    );
    let reply = d.ok(
        "gesture.commit",
        json!({ "nodes": ["a", "b"], "handle": "se", "dx": 60, "dy": 0 }),
    );
    assert!(
        note_codes(&reply).contains(&"editor.follows_anchor".to_owned()),
        "{reply}"
    );
    let text = d.session.text.clone();
    let b = line_of(&text, "b");
    assert!(
        b.contains("anchor-gap=(px)8") && b.contains("w=(px)120"),
        "{b}"
    );
    // b still sits 8 px below a and left-aligned with it.
    let a = d.corners("a");
    let bc = d.corners("b");
    assert!((bc[0].1 - (a[3].1 + 8.0)).abs() < 1e-9, "{a:?} {bc:?}");
    assert!((bc[0].0 - a[0].0).abs() < 1e-9, "{a:?} {bc:?}");
}

#[test]
fn a_selection_turn_turns_the_follower_without_moving_its_anchor() {
    let mut d = Driver::open(&doc(CHAIN));
    let reply = d.ok(
        "gesture.commit",
        json!({ "nodes": ["a", "b"], "handle": "rotate", "angle": 90 }),
    );
    let ops = reply["ops"].as_array().expect("ops");
    assert!(ops.iter().all(|o| o["op"] != "nudge_anchor_gap"), "{reply}");
    let text = d.session.text.clone();
    let b = line_of(&text, "b");
    assert!(
        b.contains("rotate=(deg)90") && b.contains("anchor-gap=(px)8"),
        "{b}"
    );
}

#[test]
fn a_container_and_its_descendant_move_and_resize_once() {
    let mut d = Driver::open(&doc(
        r#"      frame id="g" x=(px)100 y=(px)100 w=(px)80 h=(px)80 {
        rect id="in" x=(px)0 y=(px)0 w=(px)40 h=(px)40
      }
      rect id="a" x=(px)10 y=(px)10 w=(px)40 h=(px)40"#,
    ));
    let before = d.corners("in");
    let reply = d.ok(
        "gesture.commit",
        json!({ "nodes": ["in", "g", "a"], "dx": 3, "dy": 4 }),
    );
    assert_eq!(op_nodes(&reply), vec!["g", "a"], "{reply}");
    assert_moved(before, d.corners("in"), 3.0, 4.0, "in");
    let reply = d.ok(
        "gesture.commit",
        json!({ "nodes": ["in", "g", "a"], "handle": "se", "dx": 10, "dy": 10 }),
    );
    assert!(!op_nodes(&reply).contains(&"in".to_owned()), "{reply}");
}

#[test]
fn duplicating_a_node_and_its_follower_anchors_the_copies_together() {
    let mut d = Driver::open(&doc(CHAIN));
    let reply = d.ok("node.duplicate", json!({ "ids": ["b", "a"], "dx": 100 }));
    assert_eq!(reply["selection"], json!(["b-copy", "a-copy"]));
    let text = d.session.text.clone();
    let copy = line_of(&text, "b-copy");
    assert!(copy.contains(r#"anchor-sibling="a-copy""#), "{text}");
    assert!(!copy.contains(" x="), "{copy}");
    assert!(line_of(&text, "a-copy").contains("x=(px)120"), "{text}");
    let a = d.corners("a");
    let b = d.corners("b");
    let b_copy = d.corners("b-copy");
    assert_moved(b, b_copy, 100.0, 0.0, "b-copy");
    assert_moved(a, d.corners("a"), 0.0, 0.0, "a");
    // The original follower still follows the original.
    assert!(line_of(&text, "b").contains(r#"anchor-sibling="a""#));
}

#[test]
fn deleting_an_anchor_target_keeps_its_dependents_in_place() {
    let mut d = Driver::open(&doc(CHAIN));
    let b = d.corners("b");
    let c = d.corners("c");
    d.ok("node.remove", json!({ "ids": ["a"] }));
    assert_moved(b, d.corners("b"), 0.0, 0.0, "b");
    assert_moved(c, d.corners("c"), 0.0, 0.0, "c");
    let text = d.session.text.clone();
    assert!(!line_of(&text, "b").contains("anchor"), "{text}");
    assert!(
        line_of(&text, "c").contains(r#"anchor-sibling="b""#),
        "{text}"
    );
    let reply = d.ok("doc.diagnose", json!({}));
    assert_eq!(reply["valid"], true, "{reply}");
}

#[test]
fn deleting_a_node_with_its_follower_detaches_only_survivors() {
    let mut d = Driver::open(&doc(CHAIN));
    let c = d.corners("c");
    d.ok("node.remove", json!({ "ids": ["a", "b"] }));
    assert_moved(c, d.corners("c"), 0.0, 0.0, "c");
    assert!(!line_of(&d.session.text, "c").contains("anchor"));
    // A container and its descendant remove together.
    let mut d = Driver::open(&doc(r#"      group id="g" x=(px)100 y=(px)100 {
        rect id="in" x=(px)0 y=(px)0 w=(px)40 h=(px)40
      }"#));
    d.ok("node.remove", json!({ "ids": ["in", "g"] }));
    assert!(!d.session.text.contains(r#"id="g""#));
}
