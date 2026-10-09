//! `node.set` beyond geometry: raw values bound through tokens (existing
//! or created), stroke width, corner radius, font, alignment,
//! per-span text, and visibility and lock; and the `edit` fields
//! `node.inspect` lists for them.

use serde_json::json;

use crate::common::Driver;
use crate::common::doc;

fn line_of<'t>(text: &'t str, id: &str) -> &'t str {
    text.lines()
        .find(|l| l.contains(&format!("id=\"{id}\"")))
        .unwrap_or_else(|| panic!("no line for {id}"))
}

const STYLED: &str = r##"zenith version=1 {
  project id="proj.p" name="P"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#203040"
    token id="font.sans" type="fontFamily" value="Noto Sans"
    token id="size.body" type="dimension" value=(px)14
  }
  styles {
    style id="body" {
      font-size (token)"size.body"
    }
  }
  document id="doc.p" title="P" {
    page id="pg" w=(px)400 h=(px)300 {
      text id="t" x=(px)10 y=(px)10 w=(px)300 h=(px)60 style="body" fill=(token)"color.ink" font-family=(token)"font.sans" {
        span "Hello "
        span "world" fill=(token)"color.ink"
      }
      text id="bare" x=(px)10 y=(px)100 w=(px)300 h=(px)30 fill=(token)"color.ink" { span "Bare" }
    }
  }
}
"##;

#[test]
fn raw_colors_bind_an_existing_token_or_create_one() {
    let text =
        doc(r#"      rect id="r" x=(px)10 y=(px)10 w=(px)80 h=(px)20 fill=(token)"color.bg""#);
    let mut d = Driver::open(&text);
    // #203040 is color.ink: no new token.
    d.ok(
        "node.set",
        json!({ "id": "r", "fill": { "value": "#203040" } }),
    );
    assert!(line_of(&d.session.text, "r").contains(r#"fill=(token)"color.ink""#));
    assert!(!d.session.text.contains("color.custom"));
    // A new color with alpha: one token, created in the same transaction.
    let undo = d.session.history.undo.len();
    let v = d.ok(
        "node.set",
        json!({ "id": "r", "fill": { "value": "#FF000080" }, "stroke": { "value": "#ff000080" } }),
    );
    assert_eq!(d.session.history.undo.len(), undo + 1);
    let text = &d.session.text;
    assert!(
        text.contains(r##"token id="color.custom.ff000080" type="color" value="#ff000080""##),
        "{text}"
    );
    assert_eq!(
        text.matches("color.custom.ff000080\" type").count(),
        1,
        "{text}"
    );
    assert!(line_of(text, "r").contains(r#"fill=(token)"color.custom.ff000080""#));
    assert!(line_of(text, "r").contains(r#"stroke=(token)"color.custom.ff000080""#));
    assert_eq!(v["valid"], true);
    let inspect = d.ok("node.inspect", json!({ "id": "r" }));
    assert_eq!(inspect["edit"]["fill"]["token"], "color.custom.ff000080");
    assert_eq!(inspect["edit"]["fill"]["resolved"], "#ff000080");
    // Bad values name the field.
    let e = d.err("node.set", json!({ "id": "r", "fill": { "value": "red" } }));
    assert_eq!(e.code, "editor.invalid_params");
    assert!(e.message.contains("fill"), "{}", e.message);
}

#[test]
fn stroke_width_and_radius_take_px_and_null_removes_radius() {
    let mut d = Driver::open(&doc(
        r#"      rect id="r" x=(px)10 y=(px)10 w=(px)80 h=(px)20 fill=(token)"color.bg"
      rect id="s" x=(px)10 y=(px)50 w=(px)80 h=(px)20 fill=(token)"color.bg""#,
    ));
    d.ok(
        "node.set",
        json!({ "id": "r", "stroke": "color.ink", "stroke_width": { "value": 3 }, "radius": { "value": 4.5 } }),
    );
    let text = d.session.text.clone();
    assert!(
        text.contains(r#"token id="size.stroke-width.3" type="dimension" value=(px)3"#),
        "{text}"
    );
    assert!(
        text.contains(r#"token id="size.radius.4-5" type="dimension" value=(px)4.5"#),
        "{text}"
    );
    let r = line_of(&text, "r");
    assert!(
        r.contains(r#"stroke-width=(token)"size.stroke-width.3""#),
        "{r}"
    );
    assert!(r.contains(r#"radius=(token)"size.radius.4-5""#), "{r}");
    // The same px again binds the token now there, on another node.
    d.ok("node.set", json!({ "id": "s", "radius": { "value": 4.5 } }));
    assert_eq!(d.session.text.matches("token id=\"size.radius").count(), 1);
    d.ok("node.set", json!({ "id": "r", "radius": null }));
    assert!(!line_of(&d.session.text, "r").contains("radius="));
    let e = d.err("node.set", json!({ "id": "r", "radius": { "value": -1 } }));
    assert_eq!(e.code, "editor.invalid_params");
}

#[test]
fn font_fields_align_and_spans() {
    let mut d = Driver::open(STYLED);
    let e = d.ok("node.inspect", json!({ "id": "t" }))["edit"].clone();
    let fields: Vec<&str> = e["fields"]
        .as_array()
        .expect("fields")
        .iter()
        .filter_map(|f| f.as_str())
        .collect();
    for f in ["font_family", "font_size", "font_weight", "align", "spans"] {
        assert!(fields.contains(&f), "{f} in {fields:?}");
    }
    assert_eq!(e["font_family"]["token"], "font.sans");
    assert_eq!(e["font_size"]["style"]["token"], "size.body");
    assert!(
        e.get("line_height").is_none(),
        "no layout reads line-height"
    );
    assert_eq!(e["align"], "start");
    assert_eq!(e["spans"][1], json!({ "text": "world", "styled": true }));
    d.ok(
        "node.set",
        json!({
            "id": "t",
            "font_size": { "value": 18 },
            "font_weight": { "value": 700 },
            "font_family": "font.sans",
            "align": "center",
            "spans": [{ "index": 1, "text": "there" }],
        }),
    );
    let text = d.session.text.clone();
    let t = line_of(&text, "t");
    assert!(t.contains(r#"font-size=(token)"size.font-size.18""#), "{t}");
    assert!(t.contains(r#"font-weight=(token)"weight.700""#), "{t}");
    assert!(t.contains(r#"align="center""#), "{t}");
    assert!(
        !text.contains(r#"span "world""#)
            && text.contains(r#"span "there" fill=(token)"color.ink""#),
        "{text}"
    );
    let e = d.err("node.set", json!({ "id": "bare", "line_height": 1.4 }));
    assert_eq!(
        e.code, "editor.invalid_params",
        "line_height is not a node.set field"
    );
    // null drops the node's own value: the style applies again.
    d.ok("node.set", json!({ "id": "t", "font_size": null }));
    assert!(!line_of(&d.session.text, "t").contains("font-size="));
    let e = d.err(
        "node.set",
        json!({ "id": "t", "font_weight": { "value": 950 } }),
    );
    assert_eq!(e.code, "editor.invalid_params");
    let e = d.err(
        "node.set",
        json!({ "id": "t", "spans": [{ "index": 9, "text": "x" }] }),
    );
    assert_eq!(e.code, "editor.rejected");
}

#[test]
fn visibility_and_lock_toggle_even_on_a_locked_node() {
    let mut d = Driver::open(&doc(
        r#"      rect id="r" x=(px)10 y=(px)10 w=(px)80 h=(px)20 fill=(token)"color.bg" locked=#true"#,
    ));
    let e = d.err("node.set", json!({ "id": "r", "x": 20 }));
    assert_eq!(e.code, "editor.locked");
    d.ok("node.set", json!({ "id": "r", "locked": false }));
    assert!(line_of(&d.session.text, "r").contains("locked=#false"));
    d.ok("node.set", json!({ "id": "r", "visible": false }));
    assert!(line_of(&d.session.text, "r").contains("visible=#false"));
    let hidden = d.ok("node.inspect", json!({ "id": "r" }));
    assert_eq!(hidden["edit"]["visible"], false);
    d.ok("node.set", json!({ "id": "r", "visible": true }));
    assert!(line_of(&d.session.text, "r").contains("visible=#true"));
    let same = d.ok("node.set", json!({ "id": "r", "locked": false }));
    assert_eq!(same["changed"], false);
}
