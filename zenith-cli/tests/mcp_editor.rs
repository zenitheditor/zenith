//! The `zenith_editor_*` MCP tools: a full agent flow on an in-process
//! session, argument checks, and attaching to a running `zenith edit`.
//!
//! Each test drives a real `zenith mcp` subprocess line by line, with its
//! own history store.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

const DOC: &str = r##"zenith version=1 {
  // Palette.
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#112233"
  }
  document id="doc.t" {
    // The only page.
    page id="p" w=(px)200 h=(px)120 {
      rect id="box" x=(px)20 y=(px)20 w=(px)40 h=(px)30 fill=(token)"color.ink" // trailing
    }
  }
}
"##;

/// The base64 of the PNG signature.
const PNG_B64: &str = "iVBORw0KGgo";

struct Mcp {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Mcp {
    fn start(data: &Path) -> Mcp {
        let mut child = Command::new(env!("CARGO_BIN_EXE_zenith"))
            .arg("mcp")
            .env("ZENITH_DATA_DIR", data)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn zenith mcp");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        Mcp {
            child,
            stdin,
            stdout,
            next: 0,
        }
    }

    /// Call `tool` and return the `tools/call` result.
    fn call(&mut self, tool: &str, args: Value) -> Value {
        self.next += 1;
        let req = json!({
            "jsonrpc": "2.0", "id": self.next, "method": "tools/call",
            "params": { "name": tool, "arguments": args }
        });
        writeln!(self.stdin, "{req}").expect("write");
        self.stdin.flush().expect("flush");
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read");
        let resp: Value = serde_json::from_str(&line).expect("json");
        resp["result"].clone()
    }

    /// Call `tool` and return its structured content, which must not be an
    /// error.
    fn ok(&mut self, tool: &str, args: Value) -> Value {
        let r = self.call(tool, args);
        assert_eq!(r["isError"], false, "{tool}: {r}");
        r["structuredContent"].clone()
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn images(result: &Value) -> Vec<&str> {
    result["content"]
        .as_array()
        .expect("content")
        .iter()
        .filter(|c| c["type"] == "image")
        .map(|c| {
            assert_eq!(c["mimeType"], "image/png");
            c["data"].as_str().expect("data")
        })
        .collect()
}

#[test]
fn agent_edits_through_an_mcp_editor_session() {
    let dir = tempfile::tempdir().expect("dir");
    let data = tempfile::tempdir().expect("data");
    let doc = dir.path().join("d.zen");
    std::fs::write(&doc, DOC).expect("write");
    let mut mcp = Mcp::start(data.path());

    let opened = mcp.ok("zenith_editor_open", json!({ "path": doc }));
    let session = opened["session"].as_str().expect("session").to_owned();
    assert_eq!(opened["valid"], true);
    let v = opened["version"].as_u64().expect("version");

    let hit = mcp.ok(
        "zenith_editor_command",
        json!({ "session": session, "command": "select.hit", "params": { "x": 30, "y": 30 } }),
    );
    assert_eq!(hit["result"]["hits"][0]["id"], "box");

    let moved = mcp.ok(
        "zenith_editor_command",
        json!({
            "session": session, "command": "gesture.commit",
            "params": { "node": "box", "dx": 10, "dy": 5 }, "version": v,
        }),
    );
    assert_eq!(moved["result"]["changed"], true);
    assert_eq!(moved["dirty"], true);
    let diff = moved["diff"].as_str().expect("diff");
    assert!(
        diff.contains("+      rect id=\"box\" x=(px)30 y=(px)25"),
        "{diff}"
    );

    let render = mcp.call(
        "zenith_editor_render",
        json!({ "session": session, "scale": 0.5 }),
    );
    assert_eq!(render["isError"], false, "{render}");
    let pngs = images(&render);
    assert_eq!(pngs.len(), 1);
    assert!(pngs[0].starts_with(PNG_B64));
    assert_eq!(render["structuredContent"]["result"]["width"], 100);

    // A stale edit is an error result with the engine code.
    let stale = mcp.call(
        "zenith_editor_command",
        json!({
            "session": session, "command": "gesture.commit",
            "params": { "node": "box", "dx": 1, "dy": 1 }, "version": v,
        }),
    );
    assert_eq!(stale["isError"], true);
    assert_eq!(
        stale["structuredContent"]["error"]["code"],
        "editor.stale_version"
    );

    let listed = mcp.ok("zenith_editor_sessions", json!({}));
    assert_eq!(listed["sessions"][0]["session"], session.as_str());
    assert_eq!(listed["sessions"][0]["dirty"], true);

    // Reopening a dirty session needs discard.
    let reopen = mcp.call("zenith_editor_open", json!({ "path": doc }));
    assert_eq!(reopen["isError"], true);

    let saved = mcp.ok(
        "zenith_editor_command",
        json!({ "session": session, "command": "file.save" }),
    );
    assert_eq!(saved["result"]["saved"], true);
    let on_disk = std::fs::read_to_string(&doc).expect("read");
    assert!(on_disk.contains("// Palette.") && on_disk.contains("// trailing"));
    assert!(on_disk.contains("x=(px)30 y=(px)25"));

    // A path addresses the same session.
    let state = mcp.ok(
        "zenith_editor_command",
        json!({ "path": doc, "command": "file.state" }),
    );
    assert_eq!(state["session"], session.as_str());
    assert_eq!(state["result"]["dirty"], false);

    // A disk change made between calls is taken before the next command.
    let changed = on_disk.replace("w=(px)40", "w=(px)41");
    std::fs::write(&doc, &changed).expect("write");
    let state = mcp.ok(
        "zenith_editor_command",
        json!({ "session": session, "command": "file.state" }),
    );
    assert_eq!(state["external"][0]["event"], "external_change");
    assert_eq!(state["external"][0]["data"]["reloaded"], true);
    assert_eq!(state["result"]["dirty"], false);
}

#[test]
fn editor_arguments_are_checked_before_side_effects() {
    let dir = tempfile::tempdir().expect("dir");
    let data = tempfile::tempdir().expect("data");
    let doc = dir.path().join("d.zen");
    std::fs::write(&doc, DOC).expect("write");
    let mut mcp = Mcp::start(data.path());
    for (tool, args) in [
        ("zenith_editor_render", json!({ "path": doc, "scale": 9 })),
        ("zenith_editor_render", json!({ "path": doc, "page": 0 })),
        (
            "zenith_editor_command",
            json!({ "path": doc, "command": "doc.outline", "params": [1] }),
        ),
        (
            "zenith_editor_command",
            json!({ "path": doc, "session": "e1", "command": "doc.outline" }),
        ),
        (
            "zenith_editor_command",
            json!({ "path": doc, "command": "doc.outline", "bogus": 1 }),
        ),
        ("zenith_editor_open", json!({})),
        (
            "zenith_editor_attach",
            json!({ "url": "http://10.0.0.1:80/#token=x" }),
        ),
    ] {
        let r = mcp.call(tool, args.clone());
        assert_eq!(r["isError"], true, "{tool} {args}");
    }
    // No session was opened by the refused calls.
    let listed = mcp.ok("zenith_editor_sessions", json!({}));
    assert_eq!(listed["sessions"], json!([]));
    let missing = mcp.call(
        "zenith_editor_open",
        json!({ "path": dir.path().join("nope.zen") }),
    );
    let text = missing["content"][0]["text"].as_str().expect("text");
    assert!(
        text.contains("edit.missing_file") && text.contains("nope.zen"),
        "{text}"
    );
}

#[test]
fn editor_tools_open_only_zen_files_and_close_sessions() {
    let dir = tempfile::tempdir().expect("dir");
    let data = tempfile::tempdir().expect("data");
    let doc = dir.path().join("d.zen");
    std::fs::write(&doc, DOC).expect("write");
    let rc = dir.path().join(".bashrc");
    std::fs::write(&rc, "echo hi\n").expect("write");
    let mut mcp = Mcp::start(data.path());
    for (tool, args) in [
        ("zenith_editor_open", json!({ "path": rc })),
        (
            "zenith_editor_command",
            json!({ "path": rc, "command": "doc.open", "params": { "text": "pwned" } }),
        ),
    ] {
        let r = mcp.call(tool, args);
        assert_eq!(r["isError"], true, "{r}");
        let text = r["content"][0]["text"].as_str().expect("text");
        assert!(text.contains("not a .zen document"), "{text}");
    }
    assert_eq!(std::fs::read_to_string(&rc).expect("read"), "echo hi\n");
    #[cfg(unix)]
    {
        let r = mcp.call("zenith_editor_open", json!({ "path": doc, "root": "/" }));
        assert_eq!(r["isError"], true, "{r}");
        let text = r["content"][0]["text"].as_str().expect("text");
        assert!(text.contains("whole filesystem"), "{text}");
    }
    let opened = mcp.ok("zenith_editor_open", json!({ "path": doc }));
    let session = opened["session"].as_str().expect("session").to_owned();
    let v = opened["version"].as_u64().expect("version");
    mcp.ok(
        "zenith_editor_command",
        json!({
            "session": session, "command": "gesture.commit",
            "params": { "node": "box", "dx": 1, "dy": 0 }, "version": v,
        }),
    );
    let refused = mcp.call("zenith_editor_close", json!({ "session": session }));
    assert_eq!(refused["isError"], true, "unsaved edits need discard");
    let closed = mcp.ok(
        "zenith_editor_close",
        json!({ "session": session, "discard": true }),
    );
    assert_eq!(closed["closed"], session.as_str());
    assert_eq!(closed["kind"], "local");
    let listed = mcp.ok("zenith_editor_sessions", json!({}));
    assert_eq!(listed["sessions"], json!([]));
    let again = mcp.call("zenith_editor_close", json!({ "session": session }));
    assert_eq!(again["isError"], true, "a closed session is gone");
    assert_eq!(std::fs::read_to_string(&doc).expect("read"), DOC);
}

#[test]
fn attach_forwards_to_a_running_zenith_edit() {
    let dir = tempfile::tempdir().expect("dir");
    let data = tempfile::tempdir().expect("data");
    let doc = dir.path().join("d.zen");
    std::fs::write(&doc, DOC).expect("write");
    let mut server = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .arg("edit")
        .arg(&doc)
        .args(["--no-open", "--json"])
        .env("ZENITH_DATA_DIR", data.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn zenith edit");
    let mut line = String::new();
    BufReader::new(server.stdout.take().expect("stdout"))
        .read_line(&mut line)
        .expect("start line");
    let start: Value = serde_json::from_str(&line).expect("json");
    let url = start["url"].as_str().expect("url").to_owned();

    let mut mcp = Mcp::start(data.path());
    let bad = mcp.call(
        "zenith_editor_attach",
        json!({ "url": format!("http://127.0.0.1:{}/", start["port"]), "token": "nope" }),
    );
    assert_eq!(bad["isError"], true, "a wrong token is refused");

    let attached = mcp.ok("zenith_editor_attach", json!({ "url": url }));
    let session = attached["session"].as_str().expect("session").to_owned();
    assert!(session.starts_with('r'));
    let v = attached["version"].as_u64().expect("version");
    let moved = mcp.ok(
        "zenith_editor_command",
        json!({
            "session": session, "command": "gesture.commit",
            "params": { "node": "box", "dx": 10, "dy": 5 }, "version": v,
        }),
    );
    assert_eq!(moved["ok"], true);
    assert!(moved["diff"].as_str().expect("diff").contains("x=(px)30"));
    let render = mcp.call("zenith_editor_render", json!({ "session": session }));
    assert_eq!(render["isError"], false, "{render}");
    assert!(images(&render)[0].starts_with(PNG_B64));
    let listed = mcp.ok("zenith_editor_sessions", json!({}));
    assert_eq!(listed["sessions"][0]["kind"], "attached");
    assert_eq!(listed["sessions"][0]["version"], v + 1);
    assert_eq!(
        listed["sessions"][0]["dirty"], true,
        "the server holds the edit"
    );
    let _ = server.kill();
    let _ = server.wait();
}
