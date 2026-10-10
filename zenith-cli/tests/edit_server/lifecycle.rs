//! Start, port reporting, start errors, and graceful shutdown.

use std::process::Command;
use std::time::Duration;

use serde_json::json;

use crate::support::{DOC, read, start, start_on, start_with};

#[test]
fn start_line_reports_port_url_and_token() {
    let server = start();
    assert_ne!(server.port, 0);
    assert_eq!(server.token.len(), 64);
    assert!(server.token.bytes().all(|b| b.is_ascii_hexdigit()));
    assert_eq!(
        server.url,
        format!("http://127.0.0.1:{}/#token={}", server.port, server.token)
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

#[test]
fn a_read_only_document_warns_at_start_and_save_names_the_fix() {
    let dir = tempfile::tempdir().expect("tempdir");
    let doc = dir.path().join("doc.zen");
    std::fs::write(&doc, DOC).expect("write");
    let original = std::fs::metadata(&doc).expect("meta").permissions();
    let mut perms = original.clone();
    perms.set_readonly(true);
    std::fs::set_permissions(&doc, perms).expect("chmod");
    let mut server = start_on(dir, doc.clone(), &[]);
    let state = server.state();
    assert_eq!(state["readonly"], true, "{state}");
    let v = server.version();
    server.ok(json!({
        "command": "gesture.commit",
        "params": { "node": "box", "dx": 1, "dy": 0 },
        "version": v,
    }));
    let env = server.cmd(json!({ "command": "file.save" }));
    assert_eq!(env["ok"], false, "{env}");
    assert_eq!(env["error"]["code"], "edit.readonly", "{env}");
    let message = env["error"]["message"].as_str().expect("message");
    assert!(message.contains("is read-only"), "{message}");
    assert!(
        message.contains("write a copy under another name"),
        "{message}"
    );
    assert!(!message.contains("directory exists"), "{message}");
    assert!(!message.starts_with("error["), "{message}");
    assert_eq!(read(&doc), DOC, "nothing was written");
    let (_, stderr) = server.stop_with_signal_and_collect("KILL");
    assert!(stderr.contains("is read-only"), "{stderr}");
    std::fs::set_permissions(&doc, original).expect("chmod");
}

#[cfg(unix)]
mod signals {
    use std::time::Duration;

    use serde_json::json;

    use crate::support::{DOC, read, start};

    const WAIT: Duration = Duration::from_secs(10);

    #[test]
    fn ctrl_c_on_a_clean_session_stops_quietly() {
        for signal in ["INT", "TERM"] {
            let mut server = start();
            let (status, stderr) = server.stop_with_signal_and_collect(signal);
            assert_eq!(status.code(), Some(0), "SIG{signal}: {status} {stderr}");
            assert!(stderr.contains("zenith edit: stopped"), "{stderr}");
            assert!(!stderr.contains("unsaved"), "{stderr}");
        }
    }

    #[test]
    fn ctrl_c_with_unsaved_edits_warns_then_a_save_keeps_them() {
        let mut server = start();
        let doc = server.doc.clone();
        let mut events = server.events();
        let v = server.version();
        server.ok(json!({
            "command": "gesture.commit",
            "params": { "node": "box", "dx": 3, "dy": 0 },
            "version": v,
        }));
        server.signal("INT");
        let e = events.wait_for("stopping", WAIT);
        assert_eq!(e["dirty"], true, "{e}");
        assert_eq!(e["reason"], "signal");
        assert!(server.alive(), "the first signal keeps the server running");
        assert_eq!(read(&server.doc), DOC, "nothing is written by the signal");
        let saved = server.ok(json!({ "command": "file.save" }));
        assert_eq!(saved["saved"], true);
        // Clean again: the next signal stops at once, with no discard.
        let (status, stderr) = server.stop_with_signal_and_collect("INT");
        assert_eq!(status.code(), Some(0), "{status} {stderr}");
        assert!(stderr.contains("has unsaved edits"), "{stderr}");
        assert!(stderr.contains("press Ctrl-C again to discard"), "{stderr}");
        assert!(read(&doc).contains("x=(px)23"), "the saved edit is on disk");
    }

    #[test]
    fn a_second_ctrl_c_discards_unsaved_edits() {
        let mut server = start();
        let doc = server.doc.clone();
        let mut events = server.events();
        let v = server.version();
        server.ok(json!({
            "command": "gesture.commit",
            "params": { "node": "box", "dx": 3, "dy": 0 },
            "version": v,
        }));
        server.signal("INT");
        events.wait_for("stopping", WAIT);
        let (status, stderr) = server.stop_with_signal_and_collect("INT");
        assert_eq!(status.code(), Some(130), "{status} {stderr}");
        assert!(stderr.contains("unsaved edits were discarded"), "{stderr}");
        assert_eq!(read(&doc), DOC, "the discarded edit never reached the disk");
    }
}

/// The opener gets the path of a private redirect file, never the URL with
/// the token. The file goes after the first authenticated request.
#[cfg(target_os = "linux")]
#[test]
fn the_browser_opener_never_sees_the_token() {
    use std::io::{BufRead, BufReader};
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::process::Stdio;
    use std::time::Instant;

    use crate::support::raw;

    let bin = tempfile::tempdir().expect("bin");
    let record = bin.path().join("argv.txt");
    let script = bin.path().join("xdg-open");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\ncp \"$1\" '{0}.copy'\nstat -c %a \"$1\" > '{0}.mode'\nprintf '%s\\n' \"$@\" \
             > '{0}.tmp'\nmv '{0}.tmp' '{0}'\n",
            record.display()
        ),
    )
    .expect("script");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let runtime = tempfile::tempdir().expect("runtime");
    let dir = tempfile::tempdir().expect("dir");
    let doc = dir.path().join("doc.zen");
    std::fs::write(&doc, DOC).expect("write");
    let data = tempfile::tempdir().expect("data");
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .arg("edit")
        .arg(&doc)
        .arg("--json")
        .env("PATH", path)
        .env("XDG_RUNTIME_DIR", runtime.path())
        .env("ZENITH_DATA_DIR", data.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn");
    let mut line = String::new();
    BufReader::new(child.stdout.take().expect("stdout"))
        .read_line(&mut line)
        .expect("start line");
    let start: serde_json::Value = serde_json::from_str(&line).expect("json");
    let token = start["token"].as_str().expect("token").to_owned();
    let port = u16::try_from(start["port"].as_u64().expect("port")).expect("u16");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !record.exists() {
        assert!(Instant::now() < deadline, "the opener never ran");
        std::thread::sleep(Duration::from_millis(20));
    }
    let argv = std::fs::read_to_string(&record).expect("argv");
    assert!(
        !argv.contains(&token),
        "the token is on the command line: {argv}"
    );
    let file = PathBuf::from(argv.trim());
    assert!(file.starts_with(runtime.path()), "{}", file.display());
    let copy = std::fs::read_to_string(format!("{}.copy", record.display())).expect("copy");
    assert!(copy.contains(&format!("/#token={token}")), "{copy}");
    let mode = std::fs::read_to_string(format!("{}.mode", record.display())).expect("mode");
    assert_eq!(mode.trim(), "600");
    assert!(file.exists(), "the file waits for the browser");
    let r = raw(
        port,
        format!(
            "GET /api/state HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {token}\r\n\r\n"
        )
        .as_bytes(),
    );
    assert_eq!(r.status, 200);
    let deadline = Instant::now() + Duration::from_secs(5);
    while file.exists() {
        assert!(
            Instant::now() < deadline,
            "the file outlived the first authenticated request"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// The URL prints before the first validation, and the first request waits
/// for it: the first state is the opened, validated document.
#[test]
fn the_first_request_sees_the_opened_document() {
    let server = start();
    let state = server.state();
    assert_eq!(state["valid"], true, "{state}");
    assert_eq!(state["dirty"], false, "{state}");
    assert_eq!(state["text"], DOC, "{state}");
    server.shutdown(false);
}
