//! `node.inspect`: attributes with units and token bindings, the resolved
//! box, the source span, lock state, and the style source.

use serde_json::{Value, json};

use crate::common::{Driver, doc};

fn attr<'a>(v: &'a Value, name: &str) -> &'a Value {
    v["attributes"]
        .as_array()
        .expect("attributes")
        .iter()
        .find(|a| a["name"] == name)
        .unwrap_or_else(|| panic!("no attribute {name}: {v}"))
}

const SRC: &str = r##"zenith version=1 {
  project id="proj.i" name="I"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#203040"
    token id="size.w" type="dimension" value=(px)80
  }
  styles {
    style id="card" {
      fill (token)"color.ink"
    }
  }
  document id="doc.i" title="I" {
    page id="pg" w=(px)400 h=(px)300 {
      frame id="f" x=(px)10 y=(px)10 w=(px)300 h=(px)200 layout="column" locked=#true {
        rect id="r" w=(token)"size.w" h=(pt)30 style="card"
      }
    }
  }
}
"##;

#[test]
fn inspect_reports_attributes_box_span_lock_and_style() {
    let mut d = Driver::open(SRC);
    let v = d.ok("node.inspect", json!({ "id": "r" }));
    assert_eq!(v["kind"], "rect");
    assert_eq!(v["parent"], "f");
    assert_eq!(v["page"], 1);
    let w = attr(&v, "w");
    assert_eq!(w["unit"], "token");
    assert_eq!(w["token"]["id"], "size.w");
    assert_eq!(w["token"]["value"], "(px)80");
    assert_eq!(w["px"], 80.0);
    let h = attr(&v, "h");
    assert_eq!(h["unit"], "pt");
    assert_eq!(h["px"], 40.0);
    assert_eq!(v["box"]["w"], 80.0);
    assert_eq!(v["box"]["h"], 40.0);
    let span = &v["span"];
    let start = span["start"].as_u64().expect("start") as usize;
    let end = span["end"].as_u64().expect("end") as usize;
    assert!(SRC[start..end].starts_with("rect id=\"r\""), "{span}");
    assert_eq!(span["line"], 15);
    assert_eq!(v["lock"]["locked"], false);
    assert_eq!(v["lock"]["locked_by"], "f");
    assert_eq!(v["lock"]["in_flow"]["frame"], "f");
    assert_eq!(v["lock"]["in_flow"]["mode"], "column");
    assert_eq!(v["style"]["source"], "node");
    assert_eq!(v["style"]["id"], "card");
    assert_eq!(v["style"]["properties"]["fill"]["resolved"], "#203040");
    assert!(
        v["style_provenance"]
            .as_str()
            .expect("note")
            .contains("not tracked")
    );
    assert_eq!(v["stale"], false);
}

#[test]
fn inspect_while_the_text_has_errors() {
    let text = doc(r#"      rect id="r" x=(px)1 y=(px)2 w=(px)3 h=(px)4"#);
    let mut d = Driver::open(&text);
    d.ok("buffer.set", json!({ "text": format!("{text}{{{{") }));
    let v = d.ok("node.inspect", json!({ "id": "r" }));
    assert_eq!(v["stale"], true);
    assert!(
        v.get("span").is_none(),
        "no span into a text that does not parse"
    );
    assert_eq!(attr(&v, "x")["px"], 1.0);
    let e = d.err("node.inspect", json!({ "id": "zz" }));
    assert_eq!(e.code, "editor.unknown_node");
}
