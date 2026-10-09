//! Open, type with errors and fix, versions, stale rendering, and the
//! registry.

use serde_json::json;
use zenith_editor::{Request, commands, hex_sha256};

use crate::common::{Driver, doc};

const RECT: &str =
    r#"      rect id="r" x=(px)40 y=(px)50 w=(px)100 h=(px)60 fill=(token)"color.ink""#;

#[test]
fn open_validates_and_reports_pages() {
    let mut d = Driver::example("rect.zen");
    assert!(d.session.valid);
    assert_eq!(d.session.version, 2, "doc.open bumps the version");
    let reply = d.ok("doc.diagnose", json!({}));
    assert_eq!(reply["valid"], true);
    assert_eq!(reply["exit_code"], 0);
    let open = d.ok("doc.open", json!({ "text": "not kdl {{{" }));
    assert_eq!(open["valid"], false);
    assert_eq!(open["page_count"], serde_json::Value::Null);
    assert!(d.session.history.undo.is_empty());
}

#[test]
fn typing_an_error_keeps_the_last_valid_render_until_fixed() {
    let text = doc(RECT);
    let mut d = Driver::open(&text);
    let valid_render = d.outcome("doc.render", json!({}));
    let first = valid_render.image.expect("png");
    let broken = text.replace("(token)\"color.ink\"", "(token)\"color.missing\"");
    let set = d.ok("buffer.set", json!({ "text": broken }));
    assert_eq!(set["valid"], false);
    assert_eq!(set["stale"], true);
    assert!(
        set["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|x| x["severity"] == "error"),
        "{set}"
    );
    assert_eq!(d.session.last_valid_text.as_deref(), Some(text.as_str()));
    // Rendering uses the last valid text and says so.
    let stale = d.outcome("doc.render", json!({}));
    let reply = stale.result.expect("stale render");
    assert_eq!(reply["stale"], true);
    assert_eq!(stale.image.expect("png").png, first.png);
    // Gestures are disabled while the text has errors.
    let e = d.err("gesture.commit", json!({ "node": "r", "dx": 5 }));
    assert_eq!(e.code, "editor.buffer_invalid");
    // A parse error keeps the same last valid text.
    d.ok("buffer.set", json!({ "text": "zenith {" }));
    assert_eq!(d.session.last_valid_text.as_deref(), Some(text.as_str()));
    // Fixing it renders the new text.
    let fixed = text.replace("w=(px)100", "w=(px)120");
    let set = d.ok("buffer.set", json!({ "text": fixed }));
    assert_eq!(set["valid"], true);
    assert_eq!(d.session.last_valid_text, None);
    let fresh = d.outcome("doc.render", json!({}));
    assert_eq!(fresh.result.expect("render")["stale"], false);
    assert_ne!(fresh.image.expect("png").png, first.png);
}

#[test]
fn stale_and_missing_versions_are_rejected() {
    let mut d = Driver::open(&doc(RECT));
    let old = d.session.version;
    d.ok("gesture.commit", json!({ "node": "r", "dx": 5 }));
    let stale = Request::new("buffer.set", json!({ "text": "x" })).at(old);
    let before = d.session.clone();
    let out = d.send(&stale);
    let e = out.result.expect_err("stale");
    assert_eq!(e.code, "editor.stale_version");
    assert_eq!(out.session, before);
    let missing = d.send(&Request::new("buffer.set", json!({ "text": "x" })));
    assert_eq!(
        missing.result.expect_err("missing").code,
        "editor.missing_version"
    );
    // A read command with an old version is rejected too.
    let read = d.send(&Request::new("doc.outline", json!({})).at(old));
    assert_eq!(read.result.expect_err("stale").code, "editor.stale_version");
    let unknown = d.send(&Request::new("doc.explode", json!({})));
    assert_eq!(
        unknown.result.expect_err("x").code,
        "editor.unknown_command"
    );
    let bad = d.send(&Request::new("select.hit", json!({ "x": "left" })));
    assert_eq!(bad.result.expect_err("x").code, "editor.invalid_params");
}

#[test]
fn render_reports_size_hash_and_page() {
    let mut d = Driver::example("multipage.zen");
    let out = d.outcome("doc.render", json!({ "page": 2, "scale": 0.5 }));
    let reply = out.result.expect("render");
    let image = out.image.expect("png");
    assert_eq!(reply["page"], 2);
    assert_eq!(reply["sha256"], hex_sha256(&image.png));
    assert_eq!(reply["width"], image.width);
    assert_eq!(d.session.page, 2);
    let e = d.err("doc.render", json!({ "page": 99 }));
    assert_eq!(e.code, "render.page_out_of_range");
    let e = d.err("doc.render", json!({ "scale": 9 }));
    assert_eq!(e.code, "render.invalid_scale");
}

#[test]
fn never_valid_session_has_nothing_to_render() {
    let mut d = Driver::open("zenith {");
    let e = d.err("doc.render", json!({}));
    assert_eq!(e.code, "editor.no_valid_render");
}

#[test]
fn commands_list_reports_enabled_state() {
    let mut d = Driver::open(&doc(RECT));
    let list = d.ok("commands.list", json!({}));
    let entries = list["commands"].as_array().expect("commands");
    assert_eq!(entries.len(), commands().len());
    let undo = entries
        .iter()
        .find(|c| c["id"] == "history.undo")
        .expect("undo");
    assert_eq!(undo["enabled"], false);
    assert_eq!(undo["disabled"]["code"], "editor.nothing_to_undo");
    for required in [
        "doc.open",
        "buffer.set",
        "doc.diagnose",
        "doc.render",
        "doc.outline",
        "node.inspect",
        "select.hit",
        "select.set",
        "node.handles",
        "gesture.preview",
        "gesture.commit",
        "tx.apply",
        "node.remove",
        "node.duplicate",
        "node.reorder",
        "node.group",
        "node.ungroup",
        "history.undo",
        "history.redo",
        "doc.format",
        "fonts.required",
        "commands.list",
    ] {
        assert!(entries.iter().any(|c| c["id"] == required), "{required}");
    }
}

#[test]
fn outline_lists_pages_layers_and_flags() {
    let mut d = Driver::open(&doc(r#"      group id="g" name="Card" {
        rect id="a" x=(px)0 y=(px)0 w=(px)10 h=(px)10 locked=#true
        rect id="b" x=(px)0 y=(px)0 w=(px)10 h=(px)10 visible=#false
      }
      rect id="guide" x=(px)0 y=(px)0 w=(px)1 h=(px)300 role="guide""#));
    let o = d.ok("doc.outline", json!({}));
    let page = &o["pages"][0];
    assert_eq!(page["id"], "pg");
    assert_eq!(page["w"], 400.0);
    let g = &page["children"][0];
    assert_eq!(g["name"], "Card");
    assert_eq!(g["children"][0]["locked"], true);
    assert_eq!(g["children"][1]["visible"], false);
    assert_eq!(page["children"][1]["guide"], true);
}

#[test]
fn fonts_required_and_view_set() {
    let mut d = Driver::open(&doc(RECT));
    let f = d.ok("fonts.required", json!({}));
    assert_eq!(f["complete"], true);
    assert!(f["embedded"].as_array().expect("embedded").len() >= 2);
    let v = d.ok("view.set", json!({ "zoom": 2.0, "pan_x": 10.0 }));
    assert_eq!(v["viewport"]["zoom"], 2.0);
    let out = d.outcome("doc.render", json!({}));
    assert_eq!(
        out.image.expect("png").width,
        800,
        "zoom is the default scale"
    );
    assert_eq!(
        d.err("view.set", json!({ "page": 3 })).code,
        "render.page_out_of_range"
    );
    assert_eq!(
        d.err("view.set", json!({ "zoom": 0 })).code,
        "editor.invalid_params"
    );
}
