//! Live events: disk changes, conflicts, and edits from other clients.

use std::time::Duration;

use serde_json::json;

use crate::support::{DOC, apply_delta, read, start};

const WAIT: Duration = Duration::from_secs(10);

#[test]
fn clean_session_reloads_an_external_change() {
    let server = start();
    let mut events = server.events();
    let v = server.version();
    let changed = DOC.replace("w=(px)40", "w=(px)44");
    std::fs::write(&server.doc, &changed).expect("write");
    let e = events.wait_for("external_change", WAIT);
    assert_eq!(e["reloaded"], true, "{e}");
    assert_eq!(e["conflict"], false);
    assert_eq!(e["text"], changed.as_str());
    assert_eq!(e["base_version"], v);
    assert_eq!(e["version"], v + 1);
    assert_eq!(apply_delta(DOC, &e["delta"]), changed);
    let state = server.state();
    assert_eq!(state["text"], changed.as_str());
    assert_eq!(state["dirty"], false);
    // The reload is one undoable entry.
    let undo = server.ok(json!({ "command": "history.undo", "version": v + 1 }));
    assert_eq!(apply_delta(&changed, &undo["delta"]), DOC);
}

#[test]
fn dirty_session_turns_an_external_change_into_a_conflict() {
    let server = start();
    let mut events = server.events();
    let v = server.version();
    server.ok(json!({
        "command": "gesture.commit",
        "params": { "node": "box", "dx": 5, "dy": 0 },
        "version": v,
    }));
    events.wait_for("session", WAIT);
    let ours = server.state()["text"].as_str().expect("text").to_owned();
    let theirs = DOC.replace("w=(px)40", "w=(px)60");
    std::fs::write(&server.doc, &theirs).expect("write");
    let e = events.wait_for("external_change", WAIT);
    assert_eq!(e["conflict"], true, "{e}");
    assert_eq!(e["reloaded"], false);
    assert_eq!(e["text"], theirs.as_str());
    let state = server.state();
    assert_eq!(state["conflict"], true);
    assert_eq!(state["text"], ours.as_str(), "the session keeps its text");
    assert_eq!(state["disk_text"], theirs.as_str());

    // A plain save is refused with both ways out.
    let env = server.cmd(json!({ "command": "file.save" }));
    assert_eq!(env["error"]["code"], "edit.conflict", "{env}");
    let offers: Vec<&str> = env["error"]["offers"]
        .as_array()
        .expect("offers")
        .iter()
        .filter_map(|o| o["command"].as_str())
        .collect();
    assert_eq!(offers, ["file.reload", "file.save"]);
    assert_eq!(read(&server.doc), theirs, "the disk text is untouched");

    // Overwrite wins for the session.
    let saved = server.ok(json!({ "command": "file.save", "params": { "overwrite": true } }));
    assert_eq!(saved["saved"], true);
    let on_disk = read(&server.doc);
    assert!(on_disk.contains("x=(px)25"), "{on_disk}");
    assert_eq!(server.state()["conflict"], false);

    // Again, and this time reload wins for the disk.
    let v = server.version();
    server.ok(json!({
        "command": "gesture.commit",
        "params": { "node": "box", "dx": 5, "dy": 0 },
        "version": v,
    }));
    let theirs = on_disk.replace("h=(px)30", "h=(px)33");
    std::fs::write(&server.doc, &theirs).expect("write");
    let e = events.wait_for("external_change", WAIT);
    assert_eq!(e["conflict"], true, "{e}");
    let v = server.version();
    let reloaded = server.ok(json!({ "command": "file.reload", "version": v }));
    assert_eq!(reloaded["changed"], true);
    let state = server.state();
    assert_eq!(state["text"], theirs.as_str());
    assert_eq!(state["dirty"], false);
    assert_eq!(state["conflict"], false);
}

#[test]
fn two_clients_see_each_others_edits() {
    let server = start();
    let mut a_events = server.events();
    let mut b_events = server.events();
    let v = server.version();
    let mut a_text = DOC.to_owned();
    let mut b_text = DOC.to_owned();

    // B edits. A applies B's delta.
    let env = server.cmd_as(
        "b",
        json!({
            "command": "gesture.commit",
            "params": { "node": "box", "dx": 3, "dy": 4 },
            "version": v,
        }),
    );
    assert_eq!(env["ok"], true, "{env}");
    let e = a_events.wait_for("session", WAIT);
    assert_eq!(e["client"], "b");
    assert_eq!(e["command"], "gesture.commit");
    assert_eq!(e["base_version"], v);
    a_text = apply_delta(&a_text, &e["delta"]);
    b_text = apply_delta(&b_text, &env["result"]["delta"]);
    assert_eq!(a_text, b_text);
    let own = b_events.wait_for("session", WAIT);
    assert_eq!(own["client"], "b", "B sees its own edit tagged as its own");

    // A edits. B applies A's delta.
    let env = server.cmd_as(
        "a",
        json!({
            "command": "gesture.commit",
            "params": { "node": "box", "dx": -1, "dy": 0 },
            "version": v + 1,
        }),
    );
    assert_eq!(env["ok"], true, "{env}");
    let e = b_events.wait_for("session", WAIT);
    assert_eq!(e["client"], "a");
    b_text = apply_delta(&b_text, &e["delta"]);
    a_text = apply_delta(&a_text, &env["result"]["delta"]);
    assert_eq!(a_text, b_text);
    assert_eq!(server.state()["text"], a_text.as_str());

    // A save reaches both.
    server.cmd_as("a", json!({ "command": "file.save" }));
    let saved = b_events.wait_for("saved", WAIT);
    assert_eq!(saved["client"], "a");
}

#[test]
fn deleted_file_is_reported_and_a_save_writes_it_again() {
    let server = start();
    let mut events = server.events();
    std::fs::remove_file(&server.doc).expect("remove");
    let e = events.wait_for("external_change", WAIT);
    assert_eq!(e["deleted"], true, "{e}");
    assert_eq!(server.state()["missing"], true);
    server.ok(json!({ "command": "file.save" }));
    assert!(read(&server.doc).contains("// The only page."));
}
