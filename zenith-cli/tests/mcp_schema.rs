//! `zenith_schema`: every surface `zenith schema` has, over MCP.

use serde_json::{Value, json};
use zenith_cli::mcp::handle_message;

fn tool_call(name: &str, args: Value) -> Value {
    let line = json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": name, "arguments": args }
    });
    handle_message(&line.to_string()).expect("request should produce a response")
}

/// The structured result of a `tools/call` response.
fn structured(resp: &Value) -> &Value {
    &resp["result"]["structuredContent"]
}

#[test]
fn schema_surfaces_match_the_cli() {
    for surface in [
        "write",
        "ports",
        "brand",
        "block",
        "style",
        "defaults",
        "page",
        "asset",
        "document",
        "variant",
        "diagnostics",
        "tokens",
        "nodes",
        "ops",
        "overview",
    ] {
        let resp = tool_call("zenith_schema", json!({ "surface": surface }));
        assert_eq!(resp["result"]["isError"], false, "{surface}: {resp}");
    }
    let write = tool_call("zenith_schema", json!({ "surface": "write" }));
    assert!(
        structured(&write)["example"]
            .as_str()
            .is_some_and(|e| e.contains("zenith version=1")),
        "{write}"
    );
}
