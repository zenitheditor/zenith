//! Every defense of the editor server, proven over real sockets.

use serde_json::json;

use crate::support::{raw, start, status_only};

#[test]
fn wrong_or_missing_token_is_unauthorized() {
    let server = start();
    let host = server.host();
    for auth in [
        "",
        "Authorization: Bearer nope\r\n",
        "Authorization: Basic abc\r\n",
    ] {
        let r = raw(
            server.port,
            format!("GET /api/state HTTP/1.1\r\nHost: {host}\r\n{auth}\r\n").as_bytes(),
        );
        assert_eq!(r.status, 401, "{auth:?}");
        assert_eq!(r.json()["error"]["code"], "edit.unauthorized");
    }
    let body = r#"{"command":"doc.outline"}"#;
    let r = raw(
        server.port,
        format!(
            "POST /api/cmd HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .as_bytes(),
    );
    assert_eq!(r.status, 401);
    // Static files need the token too.
    let r = raw(
        server.port,
        format!("GET / HTTP/1.1\r\nHost: {host}\r\n\r\n").as_bytes(),
    );
    assert_eq!(r.status, 401);
}

#[test]
fn page_load_token_sets_a_strict_http_only_cookie() {
    let server = start();
    let host = server.host();
    let r = raw(
        server.port,
        format!(
            "GET /?token={} HTTP/1.1\r\nHost: {host}\r\n\r\n",
            server.token
        )
        .as_bytes(),
    );
    assert_eq!(r.status, 200);
    let cookie = r.header("Set-Cookie").expect("cookie");
    assert!(cookie.contains("HttpOnly"), "{cookie}");
    assert!(cookie.contains("SameSite=Strict"), "{cookie}");
    assert!(
        r.header("Content-Security-Policy")
            .is_some_and(|c| c.contains("frame-ancestors 'none'"))
    );
    let pair = cookie.split(';').next().expect("pair");
    let r = raw(
        server.port,
        format!("GET /api/state HTTP/1.1\r\nHost: {host}\r\nCookie: {pair}\r\n\r\n").as_bytes(),
    );
    assert_eq!(r.status, 200);
    // The query token works only for the page, never for the API.
    let r = raw(
        server.port,
        format!(
            "GET /api/state?token={} HTTP/1.1\r\nHost: {host}\r\n\r\n",
            server.token
        )
        .as_bytes(),
    );
    assert_eq!(r.status, 401);
}

#[test]
fn foreign_host_is_refused() {
    let server = start();
    for host in [
        format!("evil.example:{}", server.port),
        "127.0.0.1:1".to_owned(),
        String::new(),
    ] {
        let header = if host.is_empty() {
            String::new()
        } else {
            format!("Host: {host}\r\n")
        };
        let r = raw(
            server.port,
            format!(
                "GET /api/state HTTP/1.1\r\n{header}Authorization: Bearer {}\r\n\r\n",
                server.token
            )
            .as_bytes(),
        );
        assert_eq!(r.status, 403, "{host}");
        assert_eq!(r.json()["error"]["code"], "edit.bad_host");
    }
    let r = server.request("GET", "/api/state", &[("Host", "localhost")], None);
    assert_eq!(r.status, 400, "a second Host header is malformed");
}

#[test]
fn cross_origin_requests_are_refused_and_no_cors_is_sent() {
    let server = start();
    let body = json!({ "command": "doc.outline" }).to_string();
    let r = server.request(
        "POST",
        "/api/cmd",
        &[("Origin", "http://evil.example")],
        Some(&body),
    );
    assert_eq!(r.status, 403);
    assert_eq!(r.json()["error"]["code"], "edit.bad_origin");
    let r = server.request("POST", "/api/cmd", &[("Origin", "null")], Some(&body));
    assert_eq!(r.status, 403);
    let r = server.request(
        "GET",
        "/api/state",
        &[("Sec-Fetch-Site", "cross-site")],
        None,
    );
    assert_eq!(r.status, 403);
    let own = format!("http://{}", server.host());
    let r = server.request("POST", "/api/cmd", &[("Origin", &own)], Some(&body));
    assert_eq!(r.status, 200);
    assert!(r.header("Access-Control-Allow-Origin").is_none());
    // A form post (no JSON content type) never runs a command.
    let r = raw(
        server.port,
        format!(
            "POST /api/cmd HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n\
             Content-Type: text/plain\r\nContent-Length: {}\r\n\r\n{body}",
            server.host(),
            server.token,
            body.len()
        )
        .as_bytes(),
    );
    assert_eq!(r.status, 415);
}

#[test]
fn static_routes_never_reach_the_disk() {
    let server = start();
    std::fs::write(server.dir.path().join("secret.txt"), "TOP-SECRET-CONTENT").expect("write");
    for path in [
        "/../Cargo.toml",
        "/%2e%2e/%2e%2e/etc/passwd",
        "/secret.txt",
        "/doc.zen",
        "/assets/",
        "//etc/passwd",
    ] {
        let r = server.request("GET", path, &[], None);
        assert_eq!(r.status, 404, "{path}");
        assert!(!String::from_utf8_lossy(&r.body).contains("TOP-SECRET-CONTENT"));
    }
    let r = server.request("GET", "/", &[], None);
    assert_eq!(r.status, 200);
    assert!(
        r.header("Content-Type")
            .is_some_and(|t| t.starts_with("text/html"))
    );
}

#[test]
fn page_modules_serve_with_types_and_entity_tags() {
    let server = start();
    for (path, ty) in [
        ("/js/main.js", "text/javascript"),
        ("/vendor/codemirror.js", "text/javascript"),
        ("/css/tokens.css", "text/css"),
        ("/vendor/LICENSES.txt", "text/plain"),
    ] {
        let r = server.request("GET", path, &[], None);
        assert_eq!(r.status, 200, "{path}");
        assert!(
            r.header("Content-Type").is_some_and(|t| t.starts_with(ty)),
            "{path}"
        );
        assert_eq!(
            r.header("Cache-Control").as_deref(),
            Some("no-cache"),
            "{path}"
        );
        let etag = r.header("ETag").expect("ETag");
        let again = server.request("GET", path, &[("If-None-Match", &etag)], None);
        assert_eq!(again.status, 304, "{path}");
        assert!(again.body.is_empty(), "{path}: a 304 has no body");
        let stale = server.request("GET", path, &[("If-None-Match", "\"0-0\"")], None);
        assert_eq!(stale.status, 200, "{path}");
    }
    let page = server.request("GET", "/", &[], None);
    assert!(page.header("Content-Security-Policy").is_some());
    assert!(String::from_utf8_lossy(&page.body).contains("js/main.js"));
}

#[test]
fn oversize_and_malformed_requests_get_json_errors() {
    let server = start();
    let host = server.host();
    let r = raw(
        server.port,
        format!(
            "POST /api/cmd HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            server.token,
            17 * 1024 * 1024
        )
        .as_bytes(),
    );
    assert_eq!(r.status, 413);
    assert_eq!(r.json()["error"]["code"], "http.body_too_large");
    let big = format!(
        "GET /api/state HTTP/1.1\r\nHost: {host}\r\nX-Pad: {}\r\n\r\n",
        "a".repeat(20 * 1024)
    );
    assert_eq!(raw(server.port, big.as_bytes()).status, 431);
    for body in [
        "{",
        "[]",
        r#"{"command": 5}"#,
        r#"{"command":"x","extra":1}"#,
    ] {
        let r = server.request("POST", "/api/cmd", &[], Some(body));
        assert_eq!(r.status, 400, "{body}");
        assert_eq!(r.json()["error"]["code"], "edit.bad_json");
    }
    let r = raw(server.port, b"\x00\x01garbage\r\n\r\n");
    assert_eq!(r.status, 400);
    assert_eq!(r.json()["ok"], false);
    // The server still answers.
    assert_eq!(server.state()["valid"], true);
}

#[cfg(unix)]
#[test]
fn symlink_out_of_the_root_reads_as_an_error() {
    use crate::support::{DOC, start_on};

    let outside = tempfile::tempdir().expect("outside");
    std::fs::write(outside.path().join("secret.zen"), DOC).expect("write");
    let dir = tempfile::tempdir().expect("dir");
    std::os::unix::fs::symlink(
        outside.path().join("secret.zen"),
        dir.path().join("lib.zen"),
    )
    .expect("symlink");
    let doc_text = DOC.replace(
        "  tokens format=",
        "  imports {\n    import id=\"lib\" kind=\"zen\" src=\"lib.zen\"\n  }\n  tokens format=",
    );
    let doc = dir.path().join("doc.zen");
    std::fs::write(&doc, &doc_text).expect("write");
    let server = start_on(dir, doc, &[]);
    let diag = server.ok(json!({ "command": "doc.diagnose" }));
    let text = diag.to_string();
    assert!(
        text.contains("outside the editor root"),
        "the import must not be read: {text}"
    );
}

#[test]
fn event_streams_are_capped() {
    let server = start();
    let streams: Vec<_> = (0..8).map(|_| server.events()).collect();
    let r = raw(
        server.port,
        format!(
            "GET /api/events HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n\r\n",
            server.host(),
            server.token
        )
        .as_bytes(),
    );
    assert_eq!(r.status, 503);
    assert_eq!(r.json()["error"]["code"], "edit.too_many_streams");
    // Closed streams are dropped at the next write, freeing slots.
    drop(streams);
    let v = server.version();
    let mut freed = false;
    for i in 0..20 {
        let ids = if i % 2 == 0 {
            json!(["box"])
        } else {
            json!([])
        };
        server.ok(json!({ "command": "select.set", "params": { "ids": ids } }));
        let probe = status_only(
            server.port,
            &format!(
                "GET /api/events HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n\r\n",
                server.host(),
                server.token
            ),
        );
        if probe == 200 {
            freed = true;
            break;
        }
    }
    assert!(freed, "dead streams were never dropped");
    assert_eq!(server.version(), v);
}
