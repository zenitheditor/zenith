use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

const DOC: &str = r##"zenith version=1 {
  project id="proj.mcp_svg" name="SVG"
  tokens format="zenith-token-v1" {
    token id="color.red" type="color" value="#ff0000"
    token id="color.blue" type="color" value="#0000ff"
  }
  styles {}
  document id="doc.mcp_svg" title="SVG" {
    page id="page.a" w=(px)100 h=(px)80 {
      rect id="rect.a" x=(px)10 y=(px)10 w=(px)30 h=(px)20 fill=(token)"color.red"
    }
    page id="page.b" w=(px)120 h=(px)90 {
      rect id="rect.b" x=(px)20 y=(px)20 w=(px)40 h=(px)30 fill=(token)"color.blue"
    }
  }
}
"##;

fn session(store: &Path, requests: &[Value]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .arg("mcp")
        .env("ZENITH_DATA_DIR", store)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn MCP");
    {
        let mut stdin = child.stdin.take().expect("stdin");
        for request in requests {
            writeln!(stdin, "{request}").expect("write request");
        }
    }
    let output = child.wait_with_output().expect("wait MCP");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("UTF-8 stdout")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("response JSON"))
        .collect()
}

fn render(id: u64, arguments: Value) -> Value {
    json!({"jsonrpc":"2.0", "id":id, "method":"tools/call", "params":{"name":"zenith_render", "arguments":arguments}})
}

#[test]
fn svg_resource_round_trip_returns_utf8_text_and_matches_output_file() {
    let dir = tempfile::tempdir().expect("document directory");
    let store = tempfile::tempdir().expect("store directory");
    let doc = dir.path().join("doc.zen");
    let out = dir.path().join("out.svg");
    fs::write(&doc, DOC).expect("write document");
    let responses = session(
        store.path(),
        &[render(
            1,
            json!({"doc":doc, "format":"svg", "page":2, "out":out}),
        )],
    );
    let result = &responses[0]["result"];
    assert_eq!(result["isError"], false, "{responses:?}");
    let structured = &result["structuredContent"];
    assert_eq!(structured["format"], "svg");
    assert_eq!(structured["blocked"], false);
    assert_eq!(structured["resource"]["mimeType"], "image/svg+xml");
    let uri = structured["resource"]["uri"]
        .as_str()
        .expect("resource URI");
    assert!(uri.starts_with("zenith://doc/"));
    let read = session(
        store.path(),
        &[json!({"jsonrpc":"2.0", "id":2, "method":"resources/read", "params":{"uri":uri}})],
    );
    let content = &read[0]["result"]["contents"][0];
    assert_eq!(content["mimeType"], "image/svg+xml");
    assert!(
        content.get("blob").is_none(),
        "SVG resources use text: {content}"
    );
    let text = content["text"].as_str().expect("SVG text");
    assert!(text.contains("<svg") && text.contains("<path"));
    assert!(!text.contains("data:image/png"));
    assert_eq!(text, fs::read_to_string(&out).expect("SVG file"));
    assert!(
        text.contains("viewBox=\"0 0 120 90\""),
        "selected page: {text}"
    );
}

#[test]
fn svg_rejects_png_only_arguments_without_writes() {
    let dir = tempfile::tempdir().expect("document directory");
    let store = tempfile::tempdir().expect("store directory");
    let doc = dir.path().join("doc.zen");
    fs::write(&doc, DOC).expect("write document");
    for extra in [json!({"scale":0.5}), json!({"contact_sheet":true})] {
        let out = dir.path().join("blocked.svg");
        let mut arguments = json!({"doc":doc, "format":"svg", "out":out});
        arguments
            .as_object_mut()
            .expect("object")
            .extend(extra.as_object().expect("object").clone());
        let responses = session(store.path(), &[render(1, arguments)]);
        let result = &responses[0]["result"];
        assert_eq!(result["isError"], true, "{responses:?}");
        assert!(
            result["content"][0]["text"]
                .as_str()
                .expect("error text")
                .contains("cli.invalid_argument")
        );
        assert!(!out.exists());
    }
}

#[test]
fn svg_mcp_fallback_policy_blocks_resource_and_output_creation() {
    let dir = tempfile::tempdir().expect("document directory");
    let store = tempfile::tempdir().expect("store directory");
    let doc = dir.path().join("doc.zen");
    let shadow =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/shadow.zen"))
            .expect("shadow fixture");
    fs::write(&doc, shadow).expect("write document");
    fs::write(
        dir.path().join(".zenith.kdl"),
        "diagnostics { deny \"render.svg_rasterized\"; }\n",
    )
    .expect("write policy");
    let out = dir.path().join("blocked.svg");
    let responses = session(
        store.path(),
        &[render(1, json!({"doc":doc, "format":"svg", "out":out}))],
    );
    let result = &responses[0]["result"];
    assert_eq!(result["isError"], true, "{responses:?}");
    assert!(!out.exists());
    assert!(result["structuredContent"].get("resource").is_none());
    assert!(
        result["content"]
            .as_array()
            .expect("content")
            .iter()
            .all(|entry| entry["type"] != "resource_link")
    );
}
