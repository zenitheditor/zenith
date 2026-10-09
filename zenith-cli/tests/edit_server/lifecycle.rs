//! Start, port reporting, start errors, and graceful shutdown.

use std::process::Command;
use std::time::Duration;

use serde_json::json;

use crate::support::{DOC, start, start_with};

#[test]
fn start_line_reports_port_url_and_token() {
    let server = start();
    assert_ne!(server.port, 0);
    assert_eq!(server.token.len(), 64);
    assert!(server.token.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(
        server.url,
        format!("http://127.0.0.1:{}/?token={}", server.port, server.token)
    );
    let other = start();
    assert_ne!(other.token, server.token, "each run has its own token");
    server.shutdown(false);
}

#[test]
fn shutdown_refuses_unsaved_edits_unless_forced() {
    let server = start();
    let mut events = server.events();
    let v = server.version();
    server.ok(json!({
        "command": "gesture.commit",
        "params": { "node": "box", "dx": 1, "dy": 0 },
        "version": v,
    }));
    let r = server.request("POST", "/api/shutdown", &[], Some("{}"));
    assert_eq!(r.status, 409);
    assert_eq!(r.json()["error"]["code"], "edit.unsaved");
    assert_eq!(server.state()["dirty"], true, "still running");
    server.shutdown(true);
    assert_eq!(
        events.wait_for("shutdown", Duration::from_secs(10)),
        json!({})
    );
}

#[test]
fn explicit_port_is_used() {
    let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("probe");
    let port = probe.local_addr().expect("addr").port();
    drop(probe);
    let server = start_with(DOC, &["--port", &port.to_string()]);
    assert_eq!(server.port, port);
}

#[test]
fn missing_file_fails_with_the_path_and_next_step() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("nope.zen");
    let out = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .arg("edit")
        .arg(&missing)
        .arg("--no-open")
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("error[edit.missing_file]"), "{err}");
    assert!(err.contains(&missing.display().to_string()), "{err}");
    assert!(err.contains("zenith new"), "{err}");
    assert!(!missing.exists(), "nothing is created");

    let out = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .arg("edit")
        .arg(&missing)
        .args(["--no-open", "--json"])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(2));
    let envelope: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("json error envelope");
    assert_eq!(envelope["diagnostics"][0]["code"], "edit.missing_file");
}

#[test]
fn non_loopback_host_needs_allow_remote() {
    let dir = tempfile::tempdir().expect("tempdir");
    let doc = dir.path().join("d.zen");
    std::fs::write(&doc, DOC).expect("write");
    let out = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .arg("edit")
        .arg(&doc)
        .args(["--no-open", "--host", "0.0.0.0"])
        .output()
        .expect("run");
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("edit.remote_refused"), "{err}");
    assert!(err.contains("--allow-remote"), "{err}");
}
