//! `zenith_perceive`: the MCP form of `zenith perceive page`.

use std::process::Command;

use serde_json::{Value, json};
use zenith_cli::mcp::handle_message;

/// Two pages: one rect on page 1, page 2 empty, margins on page 1 only.
const DOC: &str = r##"zenith version=1 {
  project id="proj.p" name="P"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#f8fafc"
    token id="color.fg" type="color" value="#111111"
  }
  document id="doc.p" title="P" {
    page id="page.a" w=(px)100 h=(px)80 background=(token)"color.bg" margin-top=(px)8 margin-bottom=(px)8 margin-inner=(px)8 margin-outer=(px)8 {
      rect id="r.1" x=(px)10 y=(px)10 w=(px)50 h=(px)40 fill=(token)"color.fg"
    }
    page id="page.b" w=(px)100 h=(px)80 background=(token)"color.bg" {
    }
  }
}
"##;

fn tool_call(name: &str, args: Value) -> Value {
    let line = json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": name, "arguments": args }
    });
    handle_message(&line.to_string()).expect("request should produce a response")
}

fn write_doc(src: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("d.zen"), src).expect("write doc");
    dir
}

fn doc_path(dir: &tempfile::TempDir) -> String {
    dir.path().join("d.zen").to_string_lossy().into_owned()
}

#[test]
fn perceive_is_listed_with_a_doc_schema() {
    let resp = handle_message(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
        .expect("tools/list responds");
    let tools = resp["result"]["tools"].as_array().expect("tools");
    let tool = tools
        .iter()
        .find(|t| t["name"] == "zenith_perceive")
        .expect("zenith_perceive listed");
    assert_eq!(tool["inputSchema"]["required"], json!(["doc"]));
    assert!(
        tool["description"]
            .as_str()
            .unwrap_or("")
            .contains("not a judgement of design quality")
    );
}

#[test]
fn perceive_measures_page_one_by_default() {
    let dir = write_doc(DOC);
    let resp = tool_call("zenith_perceive", json!({ "doc": doc_path(&dir) }));
    assert_eq!(resp["result"]["isError"], false, "{resp}");
    let s = &resp["result"]["structuredContent"];
    assert_eq!(s["schema"], "zenith-perceive-page-v1");
    assert_eq!(s["page_count"], 2);
    let pages = s["pages"].as_array().expect("pages");
    assert_eq!(pages.len(), 1);
    let page = &pages[0];
    assert_eq!(page["id"], "page.a");
    assert_eq!(page["ink"]["pixel_count"], 2000);
    let metrics = &page["metrics"];
    for name in [
        "histogram",
        "value_zones",
        "density_map",
        "edge_map",
        "optical_balance",
        "clearspace",
    ] {
        assert_eq!(metrics[name]["status"], "measured", "{name}: {metrics}");
    }
    assert_eq!(metrics["clearspace"]["required"], 8.0);
    assert_eq!(metrics["clearspace"]["passes"], true);
    assert_eq!(metrics["grid_conformance"]["status"], "not_applicable");
}

#[test]
fn perceive_all_pages_reports_not_applicable_without_margins() {
    let dir = write_doc(DOC);
    let resp = tool_call(
        "zenith_perceive",
        json!({ "doc": doc_path(&dir), "all_pages": true }),
    );
    let pages = resp["result"]["structuredContent"]["pages"]
        .as_array()
        .expect("pages")
        .clone();
    assert_eq!(pages.len(), 2);
    let empty = &pages[1];
    assert_eq!(empty["ink"]["pixel_count"], 0);
    assert_eq!(
        empty["metrics"]["optical_balance"]["status"],
        "not_applicable"
    );
    assert!(
        empty["metrics"]["clearspace"]["reason"]
            .as_str()
            .unwrap_or("")
            .contains("margin")
    );
}

#[test]
fn perceive_rejects_a_bad_page() {
    let dir = write_doc(DOC);
    for page in [json!(0), json!(3), json!("one")] {
        let resp = tool_call(
            "zenith_perceive",
            json!({ "doc": doc_path(&dir), "page": page }),
        );
        assert!(
            resp.get("error").is_none(),
            "tool result, not protocol error"
        );
        assert_eq!(resp["result"]["isError"], true, "page {page}: {resp}");
    }
}

#[test]
fn perceive_is_blocked_by_hard_diagnostics() {
    let broken = DOC.replace(r#"fill=(token)"color.fg""#, r##"fill="#ff0000""##);
    let dir = write_doc(&broken);
    let resp = tool_call("zenith_perceive", json!({ "doc": doc_path(&dir) }));
    assert_eq!(resp["result"]["isError"], true, "{resp}");
    assert!(
        resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .contains("token.raw_visual_literal")
    );
}

#[test]
fn perceive_matches_the_cli_json() {
    let dir = write_doc(DOC);
    let path = doc_path(&dir);
    let resp = tool_call("zenith_perceive", json!({ "doc": path, "all_pages": true }));
    let output = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .args(["perceive", "page", &path, "--all-pages", "--json"])
        .output()
        .expect("run zenith perceive page");
    assert!(output.status.success(), "{output:?}");
    let cli: Value = serde_json::from_slice(&output.stdout).expect("cli json");
    assert_eq!(resp["result"]["structuredContent"], cli);
}
