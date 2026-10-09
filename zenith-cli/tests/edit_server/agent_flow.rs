//! An agent edits a document end to end with no browser: hit, drag, render,
//! undo, redo, save.

use serde_json::json;

use crate::support::{DOC, apply_delta, read, start};

#[test]
fn agent_drags_a_node_renders_and_saves_with_comments() {
    let server = start();
    let state = server.state();
    assert_eq!(state["dirty"], false);
    assert_eq!(state["valid"], true);
    assert_eq!(state["text"], DOC);
    let v0 = state["version"].as_u64().expect("version");

    // Hit the rect.
    let hit = server.ok(json!({ "command": "select.hit", "params": { "x": 30, "y": 30 } }));
    assert_eq!(hit["hits"][0]["id"], "box", "{hit}");
    assert_eq!(hit["selection"], json!(["box"]));

    // Drag it 10 px right, 5 px down.
    let env = server.cmd(json!({
        "command": "gesture.commit",
        "params": { "node": "box", "dx": 10, "dy": 5 },
        "version": v0,
        "diff": true,
    }));
    assert_eq!(env["ok"], true, "{env}");
    let result = &env["result"];
    assert_eq!(result["changed"], true);
    assert_eq!(result["reformatted"], false);
    assert_eq!(env["version"], v0 + 1);
    assert_eq!(env["dirty"], true);
    let moved = apply_delta(DOC, &result["delta"]);
    assert!(moved.contains("x=(px)30 y=(px)25"), "{moved}");
    assert_eq!(moved, server.state()["text"]);
    let diff = env["diff"].as_str().expect("diff");
    assert!(
        diff.contains("-      rect id=\"box\" x=(px)20 y=(px)20"),
        "{diff}"
    );
    assert!(
        diff.contains("+      rect id=\"box\" x=(px)30 y=(px)25"),
        "{diff}"
    );

    // Render: the PNG comes from the image route.
    let env = server.cmd(json!({ "command": "doc.render", "params": { "scale": 1 } }));
    assert_eq!(env["ok"], true, "{env}");
    let url = env["image"]["url"].as_str().expect("url");
    assert_eq!(env["image"]["width"], 200);
    let png = server.request("GET", url, &[], None);
    assert_eq!(png.status, 200);
    assert_eq!(png.header("Content-Type").as_deref(), Some("image/png"));
    assert!(png.body.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(
        png.header("Cache-Control")
            .is_some_and(|c| c.contains("immutable"))
    );

    // Undo, then redo, through the shared history.
    let v1 = v0 + 1;
    let undo = server.ok(json!({ "command": "history.undo", "version": v1 }));
    assert_eq!(apply_delta(&moved, &undo["delta"]), DOC);
    let redo = server.ok(json!({ "command": "history.redo", "version": v1 + 1 }));
    assert_eq!(apply_delta(DOC, &redo["delta"]), moved);

    // Save: comments survive, history stamps a doc-id.
    let saved = server.ok(json!({ "command": "file.save" }));
    assert_eq!(saved["saved"], true);
    assert_eq!(saved["stamped"], true, "{saved}");
    assert_eq!(saved["dirty"], false);
    let on_disk = read(&server.doc);
    for comment in [
        "// Palette.",
        "// The only page.",
        "// The box an agent drags.",
        "// trailing",
    ] {
        assert!(on_disk.contains(comment), "{comment} lost: {on_disk}");
    }
    assert!(on_disk.contains("x=(px)30 y=(px)25"), "{on_disk}");
    assert!(on_disk.contains("doc-id="), "{on_disk}");
    let state = server.state();
    assert_eq!(state["text"], on_disk.as_str());
    assert_eq!(state["dirty"], false);
    assert_eq!(state["version"], saved["version"]);

    // The CLI history recorded the save.
    assert!(
        std::fs::read_dir(server.data.path())
            .expect("data dir")
            .next()
            .is_some(),
        "history store is empty"
    );
    server.shutdown(false);
}

#[test]
fn rejected_and_stale_commands_keep_the_session() {
    let server = start();
    let v = server.version();
    let env = server.cmd(json!({
        "command": "gesture.commit",
        "params": { "node": "box", "dx": 1, "dy": 1 },
        "version": v + 7,
    }));
    assert_eq!(env["ok"], false);
    assert_eq!(env["error"]["code"], "editor.stale_version");
    let env = server.cmd(json!({ "command": "no.such.command" }));
    assert_eq!(env["error"]["code"], "editor.unknown_command");
    assert_eq!(server.version(), v);
    let list = server.ok(json!({ "command": "commands.list" }));
    let ids: Vec<&str> = list["commands"]
        .as_array()
        .expect("commands")
        .iter()
        .filter_map(|c| c["id"].as_str())
        .collect();
    assert!(ids.contains(&"file.save") && ids.contains(&"gesture.commit"));
    // /api/save is the route form of file.save.
    let r = server.request("POST", "/api/save", &[], Some("{}"));
    assert_eq!(r.status, 200);
    assert_eq!(r.json()["result"]["saved"], true);
}
