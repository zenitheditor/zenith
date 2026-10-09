//! `node.set` (the inspector's write), the `edit` part of `node.inspect`,
//! and `doc.tokens`.

use serde_json::json;
use zenith_editor::Offer;

use crate::common::{Driver, codes, doc, offer_ids};

fn offer(e: &zenith_editor::EditorError, id: &str) -> Offer {
    e.offers
        .iter()
        .find(|o| o.id == id)
        .cloned()
        .unwrap_or_else(|| panic!("no offer {id}: {e:?}"))
}

/// The lines of `after` that differ from `before` (same line count).
fn changed_lines(before: &str, after: &str) -> Vec<String> {
    let a: Vec<&str> = before.lines().collect();
    let b: Vec<&str> = after.lines().collect();
    assert_eq!(a.len(), b.len(), "line count changed:\n{after}");
    a.iter()
        .zip(b.iter())
        .filter(|(x, y)| x != y)
        .map(|(_, y)| (*y).to_owned())
        .collect()
}

#[test]
fn inspect_lists_the_fields_node_set_takes() {
    let mut d = Driver::open(&doc(r#"      // the card
      rect id="r" x=(px)10 y=(pt)30 w=(token)"size.w" h=(px)20 fill=(token)"color.ink" opacity=0.5
      text id="t" x=(px)0 y=(px)100 w=(px)200 h=(px)30 fill=(token)"color.ink" { span "Hello" }
      text id="rich" x=(px)0 y=(px)150 w=(px)200 h=(px)30 { span "a"; span "b" fill=(token)"color.bg" }
      rect id="below" anchor-sibling="r" anchor-edge="below" anchor-gap=(px)5 w=(px)50 h=(px)10
      line id="l" x1=(px)0 y1=(px)0 x2=(px)50 y2=(px)50 stroke=(token)"color.ink""#));
    let r = d.ok("node.inspect", json!({ "id": "r" }));
    let e = &r["edit"];
    assert_eq!(
        e["fields"],
        json!([
            "x",
            "y",
            "w",
            "h",
            "rotate",
            "opacity",
            "fill",
            "stroke",
            "stroke_width",
            "radius",
            "visible",
            "locked"
        ])
    );
    assert_eq!(e["visible"], true);
    assert_eq!(e["locked"], false);
    assert_eq!(e["x"], 10.0);
    assert_eq!(e["y"], 40.0);
    assert_eq!(e["w"], 80.0);
    assert_eq!(e["axes"]["w"], "token");
    assert_eq!(e["axes"]["y"], "length");
    assert_eq!(e["rotate"], 0.0);
    assert_eq!(e["opacity"], 0.5);
    assert_eq!(e["fill"]["token"], "color.ink");
    assert_eq!(e["stroke"], json!({}));
    let t = d.ok("node.inspect", json!({ "id": "t" }));
    assert_eq!(t["edit"]["text"], "Hello");
    assert!(
        !t["edit"]["fields"]
            .as_array()
            .expect("fields")
            .contains(&json!("stroke"))
    );
    let rich = d.ok("node.inspect", json!({ "id": "rich" }));
    assert!(rich["edit"].get("text").is_none(), "{rich}");
    assert_eq!(rich["edit"]["axes"]["h"], "length");
    let below = d.ok("node.inspect", json!({ "id": "below" }));
    assert_eq!(below["edit"]["axes"]["y"], "anchor");
    assert_eq!(below["edit"]["y"], 65.0);
    let l = d.ok("node.inspect", json!({ "id": "l" }));
    assert!(l["edit"].get("x").is_none(), "a line has no box fields");
    assert_eq!(l["edit"]["stroke"]["token"], "color.ink");
}

#[test]
fn node_set_writes_only_the_node_and_keeps_units_and_comments() {
    let text = doc(r#"      // the card
      rect id="r" x=(px)10 y=(pt)30 w=(px)80 h=(px)20 fill=(token)"color.ink"
      // other
      rect id="o" x=(px)200 y=(px)200 w=(px)10 h=(px)10"#);
    let mut d = Driver::open(&text);
    let v = d.ok(
        "node.set",
        json!({ "id": "r", "x": 25, "y": 52, "w": 90, "opacity": 0.25 }),
    );
    assert_eq!(v["changed"], true);
    assert_eq!(v["selection"], json!(["r"]));
    let changed = changed_lines(&text, &d.session.text);
    assert_eq!(changed.len(), 1, "{changed:?}");
    let line = &changed[0];
    assert!(
        line.contains(r#"rect id="r" x=(px)25 y=(pt)39 w=(px)90"#),
        "{line}"
    );
    assert!(line.contains("opacity=0.25"), "{line}");
    // Same values again: nothing changes.
    let same = d.ok("node.set", json!({ "id": "r", "x": 25, "w": 90 }));
    assert_eq!(same["changed"], false);
    let r = d.ok(
        "node.set",
        json!({ "id": "r", "rotate": 30, "fill": "color.bg" }),
    );
    assert_eq!(r["changed"], true);
    assert!(
        d.session.text.contains("rotate=(deg)30"),
        "{}",
        d.session.text
    );
    assert!(d.session.text.contains(r#"fill=(token)"color.bg""#));
    let back = d.ok("node.set", json!({ "id": "r", "rotate": 0 }));
    assert_eq!(back["changed"], true);
    assert!(!d.session.text.contains("rotate="), "rotate 0 removes it");
}

#[test]
fn node_set_rejections_offer_node_set_with_the_flag() {
    let text = doc(
        r#"      rect id="r" x=(token)"size.x" y=(px)40 w=(px)80 h=(px)20
      rect id="c" anchor="bottom-right" w=(px)40 h=(px)20
      frame id="f" x=(px)0 y=(px)200 w=(px)300 h=(px)60 layout="row" {
        rect id="k" w="fill" h=(px)20
      }"#,
    );
    let mut d = Driver::open(&text);
    let e = d.err("node.set", json!({ "id": "r", "x": 50 }));
    assert_eq!(codes(&e), vec!["tx.token_bound"]);
    let detach = offer(&e, "detach");
    assert_eq!(detach.command, "node.set");
    d.ok(&detach.command, detach.params.clone());
    assert!(
        d.session.text.contains(r#"rect id="r" x=(px)50"#),
        "{}",
        d.session.text
    );

    let e = d.err("node.set", json!({ "id": "c", "x": 5 }));
    assert_eq!(codes(&e), vec!["tx.anchored"]);
    assert_eq!(offer_ids(&e), vec!["detach_anchor"]);

    let e = d.err("node.set", json!({ "id": "k", "w": 30 }));
    assert_eq!(codes(&e), vec!["tx.computed_size"]);
    let set = offer(&e, "set_size");
    d.ok(&set.command, set.params.clone());
    assert!(
        d.session.text.contains(r#"rect id="k" w=(px)30"#),
        "{}",
        d.session.text
    );

    let e = d.err("node.set", json!({ "id": "k", "x": 40 }));
    assert_eq!(codes(&e), vec!["tx.layout_managed"]);
    assert_eq!(offer_ids(&e), vec!["absolute"], "node.set has no reorder");
}

#[test]
fn node_set_text_and_parameter_errors() {
    let text = doc(
        r#"      text id="t" x=(px)0 y=(px)0 w=(px)200 h=(px)30 { span "Hello" }
      text id="rich" x=(px)0 y=(px)50 w=(px)200 h=(px)30 { span "a"; span "b" italic=#true }
      rect id="r" x=(px)0 y=(px)100 w=(px)10 h=(px)10"#,
    );
    let mut d = Driver::open(&text);
    d.ok("node.set", json!({ "id": "t", "text": "Hi there" }));
    assert!(
        d.session.text.contains(r#"span "Hi there""#),
        "{}",
        d.session.text
    );
    assert_eq!(
        d.err("node.set", json!({ "id": "rich", "text": "x" })).code,
        "editor.unsupported"
    );
    assert_eq!(
        d.err("node.set", json!({ "id": "r", "opacity": 2 })).code,
        "editor.invalid_params"
    );
    assert_eq!(
        d.err("node.set", json!({ "id": "r", "w": -1 })).code,
        "editor.invalid_params"
    );
    assert_eq!(
        d.err("node.set", json!({ "id": "r", "reorder": true }))
            .code,
        "editor.invalid_params"
    );
}

#[test]
fn doc_tokens_lists_resolved_tokens_by_type() {
    let mut d = Driver::open(&doc(
        r#"      rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10"#,
    ));
    let all = d.ok("doc.tokens", json!({}));
    assert_eq!(all["tokens"].as_array().expect("tokens").len(), 4);
    let colors = d.ok("doc.tokens", json!({ "type": "color" }));
    assert_eq!(
        colors["tokens"],
        json!([
            { "id": "color.bg", "type": "color", "value": "#f0f0f0" },
            { "id": "color.ink", "type": "color", "value": "#203040" },
        ])
    );
    assert_eq!(colors["stale"], false);
}
