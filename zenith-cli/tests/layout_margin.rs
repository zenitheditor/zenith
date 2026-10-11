//! Per-child margin through the CLI: `set_layout margin_top` moves one
//! button 40px lower in a column, and the compiled-box delta that
//! `zenith tx` prints names only that button and the siblings after it.
//! `zenith schema` lists the margin attributes and op fields.

mod common;

use common::{CARDS, Env, INTO_ROW, json, stdout};

/// A fixed-size column form: a title, a button frame with an icon, and a
/// footer.
const FORM: &str = r##"zenith version=1 {
  project id="proj.m" name="M"
  tokens format="zenith-token-v1" {
    token id="color.k" type="color" value="#000000"
    token id="color.w" type="color" value="#ffffff"
  }
  styles { }
  document id="doc.m" title="M" {
    page id="pg" w=(px)400 h=(px)400 {
      frame id="form" x=(px)20 y=(px)20 w=(px)300 h=(px)300 layout="column" gap=(px)12 align="start" fill=(token)"color.w" {
        rect id="title" w=(px)200 h=(px)30 fill=(token)"color.k"
        frame id="button" w=(px)120 h=(px)40 fill=(token)"color.k" {
          rect id="button.icon" x=(px)8 y=(px)8 w=(px)24 h=(px)24 fill=(token)"color.w"
        }
        rect id="footer" w=(px)200 h=(px)20 fill=(token)"color.k"
      }
    }
  }
}
"##;

const MOVE: &str = r#"{"ops":[{"op":"set_layout","node":"button","margin_top":40}]}"#;

fn num(v: &serde_json::Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

#[test]
fn margin_top_moves_only_the_button_and_later_siblings() {
    let env = Env::with_doc(FORM);
    let out = env.tx(MOVE, &["--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    let v = json(&out);
    assert_eq!(v["status"], "accepted", "{v:#}");
    let boxes = v["boxes"].as_array().expect("boxes");
    let ids: Vec<&str> = boxes.iter().filter_map(|b| b["id"].as_str()).collect();
    assert_eq!(ids, ["button", "footer"], "{v:#}");
    for b in boxes {
        let (before, after) = (&b["before"], &b["after"]);
        assert_eq!(num(&after["y"]) - num(&before["y"]), 40.0, "{b}");
        for k in ["x", "w", "h"] {
            assert_eq!(after[k], before[k], "{k} of {b}");
        }
    }
    let button = &boxes[0];
    assert_eq!(button["descendants"], 1, "the icon moves with the button");
    assert_eq!(num(&button["before"]["y"]), 62.0);
}

#[test]
fn row_margin_left_moves_only_later_cards() {
    let env = Env::with_doc(CARDS);
    let into = env.tx(INTO_ROW, &["--apply"]);
    assert_eq!(into.status.code(), Some(0), "{}", stdout(&into));
    let v = json(&env.tx(
        r#"{"ops":[{"op":"set_layout","node":"c1","margin_left":24}]}"#,
        &["--json"],
    ));
    assert_eq!(v["status"], "accepted", "{v:#}");
    let boxes = v["boxes"].as_array().expect("boxes");
    let ids: Vec<&str> = boxes.iter().filter_map(|b| b["id"].as_str()).collect();
    assert_eq!(ids, ["c1", "c2"], "card.3 sits before c1 and stays: {v:#}");
    for b in boxes {
        assert_eq!(num(&b["after"]["x"]) - num(&b["before"]["x"]), 24.0, "{b}");
        assert_eq!(b["after"]["y"], b["before"]["y"], "{b}");
    }
}

#[test]
fn applied_margin_validates_clean() {
    let env = Env::with_doc(FORM);
    let out = env.tx(MOVE, &["--apply"]);
    assert_eq!(out.status.code(), Some(0), "{}", stdout(&out));
    assert!(env.read().contains("margin-top=(px)40"), "{}", env.read());
    let check = env.zenith(&["validate", "t.zen", "--json"]);
    let v = json(&check);
    let diags = v["diagnostics"].as_array().expect("diagnostics");
    assert!(diags.is_empty(), "{v:#}");
}

#[test]
fn nudging_an_in_flow_child_points_to_margin() {
    let env = Env::with_doc(FORM);
    let out = env.tx(
        r#"{"ops":[{"op":"nudge_geometry","node":"button","dy":40}]}"#,
        &["--json"],
    );
    let v = json(&out);
    assert_eq!(v["status"], "rejected", "{v:#}");
    let message = v["diagnostics"]
        .as_array()
        .and_then(|d| d.iter().find(|d| d["code"] == "tx.layout_managed"))
        .and_then(|d| d["message"].as_str())
        .unwrap_or_else(|| panic!("no tx.layout_managed: {v:#}"));
    assert!(message.contains("set_layout with margin_top"), "{message}");
}

#[test]
fn schema_lists_margin_attributes_and_fields() {
    let env = Env::with_doc(FORM);
    let node = stdout(&env.zenith(&["schema", "node", "rect"]));
    for name in ["margin", "margin-x", "margin-top", "margin-left"] {
        assert!(node.contains(name), "rect lists {name}:\n{node}");
    }
    assert!(node.contains("(default: margin-y)"), "{node}");
    let frame = stdout(&env.zenith(&["schema", "node", "frame"]));
    assert!(frame.contains("margin-top moves one child"), "{frame}");
    let op = stdout(&env.zenith(&["schema", "op", "set_layout"]));
    for name in [
        "margin_x",
        "margin_y",
        "margin_top",
        "margin_right",
        "margin_bottom",
        "margin_left",
    ] {
        assert!(op.contains(name), "set_layout lists {name}:\n{op}");
    }
}
