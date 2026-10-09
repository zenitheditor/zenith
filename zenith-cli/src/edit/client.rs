//! [`Remote`]: a minimal client for a running `zenith edit` server on this
//! machine. The MCP `zenith_editor_attach` tool forwards commands through
//! it, so a human watching the page sees an agent's edits live.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

use serde_json::Value;

/// Most response bytes read.
const MAX_RESPONSE: u64 = 64 * 1024 * 1024;
/// Time to connect.
const CONNECT: Duration = Duration::from_secs(5);
/// Time one exchange may take (a render can be slow).
const EXCHANGE: Duration = Duration::from_secs(120);

/// A `zenith edit` server on a loopback address, with its token.
#[derive(Clone)]
pub(crate) struct Remote {
    addr: SocketAddr,
    token: String,
}

impl std::fmt::Debug for Remote {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Remote").field("addr", &self.addr).finish()
    }
}

impl Remote {
    /// The server at `url` (`http://127.0.0.1:<port>/…`). `token` defaults
    /// to the `token=` query of `url`.
    ///
    /// # Errors
    ///
    /// A message when the URL is not `http://` on a loopback host with a
    /// port, or no token is given.
    pub(crate) fn parse(url: &str, token: Option<&str>) -> Result<Remote, String> {
        let rest = url.strip_prefix("http://").ok_or_else(|| {
            format!("url '{url}' must start with http:// (the URL `zenith edit` printed)")
        })?;
        let (authority, tail) = rest.split_once('/').unwrap_or((rest, ""));
        let (host, port) = authority.rsplit_once(':').ok_or_else(|| {
            format!("url '{url}' has no port; pass the URL `zenith edit` printed")
        })?;
        let port: u16 = port
            .parse()
            .map_err(|_| format!("url '{url}' has a bad port '{port}'"))?;
        let host = host.trim_start_matches('[').trim_end_matches(']');
        let ip = if host.eq_ignore_ascii_case("localhost") {
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        } else {
            host.parse::<IpAddr>()
                .map_err(|_| format!("url host '{host}' is not localhost or an IP address"))?
        };
        if !ip.is_loopback() {
            return Err(format!(
                "url host '{host}' is not a loopback address; attach only reaches a `zenith edit` \
                 on this machine"
            ));
        }
        let query_token = tail
            .split_once('?')
            .map_or("", |(_, q)| q)
            .split('&')
            .find_map(|pair| pair.strip_prefix("token="));
        let token = token
            .or(query_token)
            .filter(|t| !t.is_empty())
            .ok_or("no token: pass `token`, or the URL with ?token=")?;
        Ok(Remote {
            addr: SocketAddr::new(ip, port),
            token: token.to_owned(),
        })
    }

    /// The server address.
    pub(crate) fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// `GET path` as JSON.
    ///
    /// # Errors
    ///
    /// A transport error, or the server's error message for a non-200
    /// status.
    pub(crate) fn get_json(&self, path: &str) -> Result<Value, String> {
        let body = self.exchange("GET", path, None)?;
        parse_json(&body)
    }

    /// `POST path` with a JSON body, as JSON.
    ///
    /// # Errors
    ///
    /// As [`Remote::get_json`].
    pub(crate) fn post_json(&self, path: &str, body: &Value) -> Result<Value, String> {
        let bytes = body.to_string().into_bytes();
        let body = self.exchange("POST", path, Some(&bytes))?;
        parse_json(&body)
    }

    /// `GET path` as bytes.
    ///
    /// # Errors
    ///
    /// As [`Remote::get_json`].
    pub(crate) fn get_bytes(&self, path: &str) -> Result<Vec<u8>, String> {
        self.exchange("GET", path, None)
    }

    fn exchange(&self, method: &str, path: &str, body: Option<&[u8]>) -> Result<Vec<u8>, String> {
        let fail = |e: std::io::Error| format!("zenith edit at {} did not answer: {e}", self.addr);
        let mut stream = TcpStream::connect_timeout(&self.addr, CONNECT).map_err(fail)?;
        stream.set_read_timeout(Some(EXCHANGE)).map_err(fail)?;
        stream.set_write_timeout(Some(EXCHANGE)).map_err(fail)?;
        let mut head = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n\
             X-Zenith-Client: mcp\r\nConnection: close\r\n",
            self.addr, self.token
        );
        if let Some(body) = body {
            head.push_str(&format!(
                "Content-Type: application/json\r\nContent-Length: {}\r\n",
                body.len()
            ));
        }
        head.push_str("\r\n");
        stream.write_all(head.as_bytes()).map_err(fail)?;
        if let Some(body) = body {
            stream.write_all(body).map_err(fail)?;
        }
        stream.flush().map_err(fail)?;
        let mut raw = Vec::new();
        stream
            .take(MAX_RESPONSE)
            .read_to_end(&mut raw)
            .map_err(fail)?;
        let split = raw
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .ok_or_else(|| format!("zenith edit at {} sent no response head", self.addr))?;
        let head = String::from_utf8_lossy(raw.get(..split).unwrap_or_default()).into_owned();
        let body = raw.get(split + 4..).unwrap_or_default().to_vec();
        let status = head
            .split(' ')
            .nth(1)
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);
        if status == 200 {
            return Ok(body);
        }
        let message = serde_json::from_slice::<Value>(&body)
            .ok()
            .and_then(|v| {
                let code = v["error"]["code"].as_str()?.to_owned();
                let message = v["error"]["message"].as_str()?.to_owned();
                Some(format!("{code}: {message}"))
            })
            .unwrap_or_else(|| String::from_utf8_lossy(&body).into_owned());
        Err(format!(
            "zenith edit at {} answered {status}: {message}",
            self.addr
        ))
    }
}

fn parse_json(body: &[u8]) -> Result<Value, String> {
    serde_json::from_slice(body).map_err(|e| format!("zenith edit sent bad JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_urls_with_a_token_parse() {
        let r = Remote::parse("http://127.0.0.1:4000/?token=abc", None).expect("parse");
        assert_eq!(r.addr(), "127.0.0.1:4000".parse().expect("addr"));
        assert_eq!(r.token, "abc");
        let r = Remote::parse("http://localhost:4000", Some("t")).expect("parse");
        assert_eq!(r.token, "t");
        assert!(Remote::parse("http://[::1]:4000", Some("t")).is_ok());
        for bad in [
            "https://127.0.0.1:4000",
            "http://10.0.0.1:4000",
            "http://example.com:4000",
            "http://127.0.0.1",
        ] {
            assert!(Remote::parse(bad, Some("t")).is_err(), "{bad}");
        }
        assert!(Remote::parse("http://127.0.0.1:4000/", None).is_err());
    }
}
