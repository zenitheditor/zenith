//! Duplicate, remove (with comments), group / ungroup, reorder, and
//! comment-preserving patches.

use serde_json::json;

use crate::common::{Driver, doc};

const COMMENTED: &str = r#"      // the backdrop
      rect id="bg" x=(px)0 y=(px)0 w=(px)400 h=(px)300 fill=(token)"color.bg"
      // card: keep in sync with the brand kit
      rect id="card" x=(px)40 y=(px)40 w=(px)120 h=(px)80 fill=(token)"color.ink" // inline note
      rect id="other" x=(px)200 y=(px)40 w=(px)120 h=(px)80 fill=(token)"color.ink""#;

#[test]
fn duplicate_a_container_selects_the_copy() {
    let mut d = Driver::example("group.zen");
    d.ok("select.set", json!({ "ids": ["group.shapes"] }));
    let r = d.ok("node.duplicate", json!({ "dx": 10, "dy": 0 }));
    assert_eq!(r["selection"], json!(["group.shapes-copy"]));
    assert!(d.session.text.contains("rect id=\"rect.blue-copy\""));
    assert!(
        d.session
            .text
            .contains("ellipse id=\"ellipse.orange-copy\"")
    );
    assert_eq!(r["reformatted"], false);
    let again = d.ok("node.duplicate", json!({ "id": "group.shapes" }));
    assert_eq!(again["selection"], json!(["group.shapes-copy2"]));
    let line = d.err("node.duplicate", json!({ "id": "nope" }));
    assert_eq!(line.code, "editor.unknown_node");
}

#[test]
fn remove_reports_the_comments_that_went_with_the_node() {
    let mut d = Driver::open(&doc(COMMENTED));
    d.ok("select.set", json!({ "ids": ["card", "other"] }));
    let r = d.ok("node.remove", json!({ "ids": ["card"] }));
    assert_eq!(
        r["removed_comments"],
        json!(["// card: keep in sync with the brand kit"])
    );
    assert_eq!(r["selection"], json!(["other"]));
    assert!(d.session.text.contains("// the backdrop"));
    assert!(!d.session.text.contains("card"));
}

#[test]
fn gestures_keep_comments_in_the_text() {
    let mut d = Driver::open(&doc(COMMENTED));
    let r = d.ok(
        "gesture.commit",
        json!({ "node": "card", "dx": 5, "dy": 5 }),
    );
    assert_eq!(r["reformatted"], false);
    assert_eq!(r["removed_comments"], json!([]));
    for c in ["// the backdrop", "// card: keep in sync", "// inline note"] {
        assert!(d.session.text.contains(c), "{c} kept: {}", d.session.text);
    }
    let delta = &r["delta"];
    let start = delta["start"].as_u64().expect("start") as usize;
    let end = delta["end"].as_u64().expect("end") as usize;
    assert!(end - start <= 12, "minimal delta: {delta}");
}

#[test]
fn group_ungroup_and_reorder() {
    let mut d = Driver::open(&doc(COMMENTED));
    d.ok("select.set", json!({ "ids": ["card", "other"] }));
    let g = d.ok("node.group", json!({}));
    assert_eq!(g["selection"], json!(["group"]));
    assert!(d.session.text.contains("group id=\"group\""));
    let u = d.ok("node.ungroup", json!({}));
    assert_eq!(u["selection"], json!(["card", "other"]));
    assert!(!d.session.text.contains("group id="));
    let r = d.ok("node.reorder", json!({ "id": "bg", "to": "front" }));
    assert_eq!(r["ops"][0]["op"], "move_to_front");
    let bg = d.session.text.find("rect id=\"bg\"").expect("bg");
    let other = d.session.text.find("rect id=\"other\"").expect("other");
    assert!(bg > other);
    let e = d.err("node.reorder", json!({ "id": "bg", "to": "sideways" }));
    assert_eq!(e.code, "editor.invalid_params");
    d.ok("select.set", json!({ "ids": [] }));
    let none = d.err("node.remove", json!({}));
    assert_eq!(none.code, "editor.no_selection");
}

#[test]
fn format_rewrites_canonically_and_lists_dropped_comments() {
    let mut d = Driver::open(&doc(COMMENTED));
    let r = d.ok("doc.format", json!({}));
    assert_eq!(r["reformatted"], true);
    assert!(
        r["removed_comments"]
            .as_array()
            .expect("comments")
            .iter()
            .any(|c| c == "// the backdrop"),
        "{r}"
    );
    let again = d.ok("doc.format", json!({}));
    assert_eq!(again["changed"], false);
}
