//! Native Streamable-HTTP transport for the MCP server (`zenith mcp --http`).
//!
//! Gated behind the `http` Cargo feature. It runs on the bounded HTTP/1.1
//! layer of `zenith edit` (`std::net`, no extra dependency) and drives the
//! very same [`super::handle_message`] seam as stdio. No async runtime, no
//! C dependencies.
//!
//! The client POSTs one JSON-RPC message to `/mcp` and receives the JSON-RPC
//! response as `application/json`. Notifications (no `id`) get `202
//! Accepted` with no body. SSE streaming is not used (every Zenith tool is a
//! simple request/response), so a `GET` is answered `405`.
//!
//! # Security
//!
//! The tools read and write files, so every request passes these checks
//! before its body is read:
//!
//! - **Bind.** Loopback only, unless `--allow-remote`.
//! - **Token.** `Authorization: Bearer <token>`, compared in constant time.
//!   The token comes from `ZENITH_MCP_TOKEN` (at least 32 visible ASCII
//!   characters), else 32 random bytes made at start and printed to stderr.
//! - **Host.** `localhost`, an IP literal, the `--http` host, or an
//!   `--allow-host` name, with the bound port. A DNS-rebinding page carries
//!   its own name and gets 403.
//! - **Origin.** When present, `http://<Host>` or `https://<Host>`. A
//!   browser page on another site gets 403.
//! - **Content-Type.** `application/json`, else 415.
//! - **Limits.** Head 16 KiB within 5 s, body at most 8 MiB (413). A client
//!   silent for 5 s is dropped. One thread per connection, at most 32 open.
//!   Past the cap a client gets 503 from a non-blocking write.
//!
//! Tool calls run one at a time, as over stdio. The file policy of
//! `super::policy` applies on top: the HTTP transport confines every tool
//! path to `--root`, by default the working directory.

use std::io::Write;
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use crate::edit::{
    HttpError, HttpRequest, Limits, Method, Response, Token, read_body, read_head, split_host,
};

/// Most request body bytes.
const MAX_BODY: usize = 8 * 1024 * 1024;
/// Shortest configured token.
const MIN_TOKEN: usize = 32;
/// The one endpoint.
const ENDPOINT: &str = "/mcp";
/// Open connections.
const MAX_CONNECTIONS: usize = 32;
/// Time one response write may take.
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// How to serve MCP over HTTP.
#[derive(Debug, Clone, Default)]
pub struct HttpOptions {
    /// `host:port` to bind.
    pub addr: String,
    /// Allow a non-loopback bind address.
    pub allow_remote: bool,
    /// Extra names accepted in `Host` (for a reverse proxy).
    pub allow_hosts: Vec<String>,
}

/// The checks every request passes before its body is read.
struct Gate {
    token: Token,
    port: u16,
    /// Accepted `Host` names besides `localhost` and IP literals, lowercase.
    names: Vec<String>,
}

impl Gate {
    fn check(&self, req: &HttpRequest) -> Result<(), HttpError> {
        let host = req
            .header("host")?
            .ok_or_else(|| HttpError::new(403, "mcp.bad_host", "the request has no Host header"))?;
        if !self.host_allowed(host) {
            return Err(HttpError::new(
                403,
                "mcp.bad_host",
                format!(
                    "Host '{host}' is not this server; use localhost or an IP address with port \
                     {}, or start the server with --allow-host <name>",
                    self.port
                ),
            ));
        }
        if let Some(origin) = req.header("origin")? {
            let ok = [format!("http://{host}"), format!("https://{host}")]
                .iter()
                .any(|o| o.eq_ignore_ascii_case(origin));
            if !ok {
                return Err(HttpError::new(
                    403,
                    "mcp.bad_origin",
                    format!(
                        "Origin '{origin}' is not this server; send requests from an MCP client, \
                         without Origin"
                    ),
                ));
            }
        }
        let auth = req.header("authorization")?.unwrap_or("");
        let given = match auth.split_once(' ') {
            Some((scheme, rest)) if scheme.eq_ignore_ascii_case("bearer") => rest.trim(),
            Some(_) | None => "",
        };
        if !self.token.matches(given) {
            return Err(HttpError::new(
                401,
                "mcp.unauthorized",
                "missing or wrong token; send 'Authorization: Bearer <token>' with the token \
                 `zenith mcp --http` printed (or ZENITH_MCP_TOKEN)",
            ));
        }
        if req.path != ENDPOINT {
            return Err(HttpError::new(
                404,
                "mcp.not_found",
                format!("no endpoint '{}'; POST JSON-RPC to {ENDPOINT}", req.path),
            ));
        }
        if req.method != Method::Post {
            return Err(HttpError::new(
                405,
                "mcp.method_not_allowed",
                format!("{ENDPOINT} takes POST with one JSON-RPC message"),
            ));
        }
        let ty = req.header("content-type")?.unwrap_or("");
        if !ty
            .split(';')
            .next()
            .is_some_and(|t| t.trim().eq_ignore_ascii_case("application/json"))
        {
            return Err(HttpError::new(
                415,
                "mcp.bad_content_type",
                format!("Content-Type '{ty}' is not application/json; send a JSON body"),
            ));
        }
        Ok(())
    }

    fn host_allowed(&self, host: &str) -> bool {
        let (name, port) = split_host(host);
        if port != Some(self.port) && !(port.is_none() && self.port == 80) {
            return false;
        }
        let name = name.to_ascii_lowercase();
        let ip = name
            .strip_prefix('[')
            .and_then(|n| n.strip_suffix(']'))
            .unwrap_or(&name);
        name == "localhost" || ip.parse::<IpAddr>().is_ok() || self.names.contains(&name)
    }
}

/// The token: `ZENITH_MCP_TOKEN`, else a fresh one. `true` when fresh.
fn token() -> Result<(Token, bool), String> {
    match std::env::var("ZENITH_MCP_TOKEN") {
        Ok(value) => {
            if value.len() < MIN_TOKEN || !value.bytes().all(|b| b.is_ascii_graphic()) {
                return Err(format!(
                    "ZENITH_MCP_TOKEN must be at least {MIN_TOKEN} visible ASCII characters; \
                     set a longer one, or unset it to get a random token"
                ));
            }
            Ok((Token::from_secret(value), false))
        }
        Err(_) => Token::generate()
            .map(|t| (t, true))
            .map_err(|e| format!("cannot make a token: {e}")),
    }
}

/// Everything a connection thread reaches.
struct Served {
    gate: Gate,
    limits: Limits,
    open: AtomicUsize,
    /// Tool calls run one at a time, as over stdio.
    calls: Mutex<()>,
}

/// Serve the MCP protocol over HTTP until the process is killed.
///
/// Returns a non-zero exit code when the address is refused or cannot be
/// bound.
pub fn serve(options: &HttpOptions) -> u8 {
    let addr: SocketAddr = match options.addr.parse() {
        Ok(a) => a,
        Err(_) => {
            eprintln!(
                "zenith mcp: --http '{}' is not IP:PORT; pass e.g. 127.0.0.1:8080",
                options.addr
            );
            return 2;
        }
    };
    if !addr.ip().is_loopback() && !options.allow_remote {
        eprintln!(
            "zenith mcp: --http '{addr}' is not a loopback address; anyone who reaches it can \
             try the token. Add --allow-remote to serve on it anyway (behind a TLS proxy)"
        );
        return 2;
    }
    let (token, fresh) = match token() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("zenith mcp: {e}");
            return 2;
        }
    };
    let listener = match TcpListener::bind(addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("zenith mcp: cannot bind '{addr}': {e}");
            return 1;
        }
    };
    let bound = listener.local_addr().unwrap_or(addr);
    let mut names: Vec<String> = options
        .allow_hosts
        .iter()
        .map(|h| h.to_ascii_lowercase())
        .collect();
    names.push(addr.ip().to_string());
    if !addr.ip().is_loopback() {
        eprintln!(
            "zenith mcp: serving on non-loopback {bound} over plain HTTP; the token crosses the \
             network unencrypted. Put a TLS reverse proxy in front, or use an SSH tunnel"
        );
    }
    eprintln!("zenith mcp: HTTP transport listening on http://{bound}{ENDPOINT}");
    if fresh {
        eprintln!("zenith mcp: token {}", token.as_str());
    } else {
        eprintln!("zenith mcp: token from ZENITH_MCP_TOKEN");
    }
    eprintln!("zenith mcp: send 'Authorization: Bearer <token>' with every request");
    let served = Arc::new(Served {
        gate: Gate {
            token,
            port: bound.port(),
            names,
        },
        limits: Limits {
            body: MAX_BODY,
            ..Limits::default()
        },
        open: AtomicUsize::new(0),
        calls: Mutex::new(()),
    });
    for stream in listener.incoming() {
        let Ok(stream) = stream else {
            // Out of descriptors, say: pause instead of spinning.
            std::thread::sleep(Duration::from_millis(20));
            continue;
        };
        if served.open.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
            served.open.fetch_sub(1, Ordering::SeqCst);
            busy(&stream);
            continue;
        }
        let conn = Arc::clone(&served);
        let spawned = std::thread::Builder::new()
            .name("zenith-mcp-conn".into())
            .spawn(move || {
                handle(&conn, stream);
                conn.open.fetch_sub(1, Ordering::SeqCst);
            });
        // A failed spawn dropped the closure and the socket: give the slot
        // back here.
        if spawned.is_err() {
            served.open.fetch_sub(1, Ordering::SeqCst);
        }
    }
    0
}

/// Answer 503 with one non-blocking write, then close.
fn busy(stream: &TcpStream) {
    let error = HttpError::new(
        503,
        "mcp.busy",
        "the server has too many open connections; retry in a moment",
    );
    let mut bytes = Vec::new();
    if Response::error(&error).write_to(&mut bytes).is_ok() && stream.set_nonblocking(true).is_ok()
    {
        let mut writer = stream;
        let _ = writer.write(&bytes);
    }
}

fn handle(served: &Served, mut stream: TcpStream) {
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
    let response = respond(served, &mut stream).unwrap_or_else(|e| Response::error(&e));
    let _ = response.write_to(&mut stream);
}

fn respond(served: &Served, stream: &mut TcpStream) -> Result<Response, HttpError> {
    let (mut req, leftover) = read_head(stream, served.limits)?;
    served.gate.check(&req)?;
    read_body(stream, &mut req, leftover, served.limits)?;
    let body = std::str::from_utf8(&req.body).map_err(|_| {
        HttpError::new(
            400,
            "mcp.bad_body",
            "the body is not UTF-8 JSON; send one JSON-RPC message",
        )
    })?;
    let reply = {
        let _one = served.calls.lock().unwrap_or_else(PoisonError::into_inner);
        super::handle_message(body)
    };
    Ok(match reply {
        Some(value) => Response::json(200, &value),
        // A notification produced no reply.
        None => Response::new(202, "application/json", Vec::new()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate() -> Gate {
        Gate {
            token: Token::from_secret("t".repeat(40)),
            port: 8080,
            names: vec!["127.0.0.1".into(), "mcp.example".into()],
        }
    }

    fn req(method: Method, path: &str, headers: &[(&str, String)]) -> HttpRequest {
        HttpRequest {
            method,
            path: path.into(),
            query: String::new(),
            headers: headers
                .iter()
                .map(|(n, v)| (n.to_ascii_lowercase(), v.clone()))
                .collect(),
            length: 0,
            body: Vec::new(),
        }
    }

    fn good() -> Vec<(&'static str, String)> {
        vec![
            ("Host", "127.0.0.1:8080".into()),
            ("Authorization", format!("Bearer {}", "t".repeat(40))),
            ("Content-Type", "application/json".into()),
        ]
    }

    fn with(extra: &[(&'static str, &str)], drop: &str) -> Vec<(&'static str, String)> {
        let mut h: Vec<_> = good().into_iter().filter(|(n, _)| *n != drop).collect();
        h.extend(extra.iter().map(|(n, v)| (*n, (*v).to_owned())));
        h
    }

    fn run(headers: &[(&'static str, String)]) -> Result<(), HttpError> {
        gate().check(&req(Method::Post, "/mcp", headers))
    }

    #[test]
    fn a_complete_request_passes() {
        assert_eq!(run(&good()), Ok(()));
        assert_eq!(
            run(&with(&[("Origin", "http://127.0.0.1:8080")], "")),
            Ok(())
        );
        assert_eq!(run(&with(&[("Host", "mcp.example:8080")], "Host")), Ok(()));
    }

    #[test]
    fn each_missing_proof_is_refused() {
        let code = |h: Vec<(&'static str, String)>| run(&h).expect_err("refused").code;
        assert_eq!(code(with(&[], "Authorization")), "mcp.unauthorized");
        assert_eq!(
            code(with(&[("Authorization", "Bearer nope")], "Authorization")),
            "mcp.unauthorized"
        );
        assert_eq!(
            code(with(&[("Host", "evil.example:8080")], "Host")),
            "mcp.bad_host"
        );
        assert_eq!(
            code(with(&[("Host", "127.0.0.1:1")], "Host")),
            "mcp.bad_host"
        );
        assert_eq!(code(with(&[], "Host")), "mcp.bad_host");
        assert_eq!(
            code(with(&[("Origin", "http://evil.example")], "")),
            "mcp.bad_origin"
        );
        assert_eq!(
            code(with(&[("Content-Type", "text/plain")], "Content-Type")),
            "mcp.bad_content_type"
        );
        let g = gate();
        let get = g.check(&req(Method::Get, "/mcp", &good()));
        assert_eq!(get.expect_err("get").status, 405);
        let path = g.check(&req(Method::Post, "/other", &good()));
        assert_eq!(path.expect_err("path").status, 404);
    }

    #[test]
    fn the_token_is_checked_before_the_path_and_method() {
        let anon = req(Method::Get, "/other", &[("Host", "127.0.0.1:8080".into())]);
        assert_eq!(
            gate().check(&anon).expect_err("anon").status,
            401,
            "an anonymous client learns nothing else"
        );
    }
}
