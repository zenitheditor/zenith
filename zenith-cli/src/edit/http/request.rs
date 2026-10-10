//! Read one HTTP/1.1 request from a socket, within size and time limits.

use std::io::{ErrorKind, Read};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use super::HttpError;

/// Size and time limits for one request.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
    /// Bytes of request line plus headers.
    pub(crate) head: usize,
    /// Header lines.
    pub(crate) headers: usize,
    /// Body bytes.
    pub(crate) body: usize,
    /// Time to receive the request line and the headers.
    pub(crate) head_deadline: Duration,
    /// Time to receive the body, counted from the end of the head.
    pub(crate) body_deadline: Duration,
    /// Longest wait for the next bytes. A client that sends nothing for
    /// this long is dropped.
    pub(crate) idle: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            head: 16 * 1024,
            headers: 64,
            body: 16 * 1024 * 1024,
            head_deadline: Duration::from_secs(5),
            body_deadline: Duration::from_secs(60),
            idle: Duration::from_secs(5),
        }
    }
}

/// The request method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Method {
    /// `GET`.
    Get,
    /// `POST`.
    Post,
    /// `HEAD`: a `GET` without the body. Static routes answer it.
    Head,
    /// Any other method. Every route answers it with 405.
    Other(String),
}

/// One parsed request.
#[derive(Debug, Clone)]
pub(crate) struct HttpRequest {
    pub(crate) method: Method,
    /// The path part of the target, starting with `/`. Not decoded.
    pub(crate) path: String,
    /// The query part of the target, without `?`. Not decoded.
    pub(crate) query: String,
    /// Header names in lowercase, with their trimmed values, in order.
    pub(crate) headers: Vec<(String, String)>,
    /// The `Content-Length` value. `0` without the header.
    pub(crate) length: usize,
    /// The body. Empty until [`read_body`] ran.
    pub(crate) body: Vec<u8>,
}

/// Body bytes that arrived with the head, kept for [`read_body`].
#[derive(Debug, Default)]
pub(crate) struct Leftover(Vec<u8>);

impl HttpRequest {
    /// The value of the single header `name` (lowercase). `None` when it is
    /// absent.
    ///
    /// # Errors
    ///
    /// 400 when the header appears more than once.
    pub(crate) fn header(&self, name: &str) -> Result<Option<&str>, HttpError> {
        let mut found = None;
        for (n, v) in &self.headers {
            if n == name {
                if found.is_some() {
                    return Err(HttpError::new(
                        400,
                        "http.duplicate_header",
                        format!("the request has more than one '{name}' header; send one"),
                    ));
                }
                found = Some(v.as_str());
            }
        }
        Ok(found)
    }

    /// The value of query parameter `key`, undecoded.
    pub(crate) fn query_param(&self, key: &str) -> Option<&str> {
        self.query
            .split('&')
            .filter_map(|pair| pair.split_once('=').or(Some((pair, ""))))
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v)
    }
}

/// Read the request line and the headers from `stream`. The body stays on
/// the socket until [`read_body`], so a caller can check the request first.
///
/// # Errors
///
/// 408 past the head deadline or the idle wait, 431 for an oversize head,
/// 501 for `Transfer-Encoding`, 505 for a non-1.x version, and 400 for
/// anything malformed.
pub(crate) fn read_head(
    stream: &mut TcpStream,
    limits: Limits,
) -> Result<(HttpRequest, Leftover), HttpError> {
    let start = Instant::now();
    let mut buf: Vec<u8> = Vec::with_capacity(2048);
    let head_end = loop {
        if let Some(end) = find_head_end(&buf) {
            break end;
        }
        if buf.len() > limits.head {
            return Err(head_too_large(limits));
        }
        read_some(stream, &mut buf, start, limits.head_deadline, limits.idle)?;
    };
    if head_end > limits.head {
        return Err(head_too_large(limits));
    }
    let head_bytes = buf.get(..head_end).unwrap_or_default();
    let head = std::str::from_utf8(head_bytes)
        .map_err(|_| bad("http.bad_head", "the request head is not UTF-8"))?;
    let mut lines = head.split("\r\n");
    let (method, path, query) = request_line(lines.next().unwrap_or_default())?;
    let headers = header_lines(lines, limits)?;
    let mut request = HttpRequest {
        method,
        path,
        query,
        headers,
        length: 0,
        body: Vec::new(),
    };
    if request.header("transfer-encoding")?.is_some() {
        return Err(HttpError::new(
            501,
            "http.chunked_unsupported",
            "chunked bodies are not supported; send the body with Content-Length",
        ));
    }
    request.length = match request.header("content-length")? {
        None => 0,
        Some(v) => v.parse::<usize>().map_err(|_| {
            bad(
                "http.bad_length",
                format!("Content-Length '{v}' is not a byte count"),
            )
        })?,
    };
    let rest = buf
        .get(head_end + 4..)
        .map(<[u8]>::to_vec)
        .unwrap_or_default();
    Ok((request, Leftover(rest)))
}

/// Read the body of `request` (its `Content-Length` bytes) into
/// `request.body`.
///
/// # Errors
///
/// 413 for an oversize body, 408 past the body deadline or the idle wait,
/// 400 when the client closes early.
pub(crate) fn read_body(
    stream: &mut TcpStream,
    request: &mut HttpRequest,
    leftover: Leftover,
    limits: Limits,
) -> Result<(), HttpError> {
    let length = request.length;
    if length > limits.body {
        return Err(HttpError::new(
            413,
            "http.body_too_large",
            format!(
                "the body is {length} bytes; the limit is {} bytes",
                limits.body
            ),
        ));
    }
    let start = Instant::now();
    let mut body = leftover.0;
    while body.len() < length {
        read_some(stream, &mut body, start, limits.body_deadline, limits.idle)?;
    }
    body.truncate(length);
    request.body = body;
    Ok(())
}

/// Read more bytes into `buf` before the deadline.
fn read_some(
    stream: &mut TcpStream,
    buf: &mut Vec<u8>,
    start: Instant,
    deadline: Duration,
    idle: Duration,
) -> Result<(), HttpError> {
    let remaining = deadline
        .checked_sub(start.elapsed())
        .filter(|d| !d.is_zero())
        .ok_or_else(timeout)?;
    stream
        .set_read_timeout(Some(remaining.min(idle).max(Duration::from_millis(1))))
        .map_err(|e| bad("http.socket", e.to_string()))?;
    let mut chunk = [0u8; 8192];
    match stream.read(&mut chunk) {
        Ok(0) => Err(bad(
            "http.closed",
            "the client closed the connection mid-request",
        )),
        Ok(n) => {
            buf.extend_from_slice(chunk.get(..n).unwrap_or_default());
            Ok(())
        }
        Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => Err(timeout()),
        Err(e) if e.kind() == ErrorKind::Interrupted => Ok(()),
        Err(e) => Err(bad("http.read_failed", e.to_string())),
    }
}

fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn request_line(line: &str) -> Result<(Method, String, String), HttpError> {
    let mut parts = line.split(' ');
    let (Some(method), Some(target), Some(version), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(bad(
            "http.bad_request_line",
            format!("'{line}' is not 'METHOD /path HTTP/1.1'"),
        ));
    };
    if !version.starts_with("HTTP/1.") {
        return Err(HttpError::new(
            505,
            "http.bad_version",
            format!("'{version}' is not supported; send HTTP/1.1"),
        ));
    }
    if !target.starts_with('/') {
        return Err(bad(
            "http.bad_target",
            format!("the target '{target}' must start with '/'"),
        ));
    }
    let method = match method {
        "GET" => Method::Get,
        "POST" => Method::Post,
        "HEAD" => Method::Head,
        other => Method::Other(other.to_owned()),
    };
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    Ok((method, path.to_owned(), query.to_owned()))
}

fn header_lines<'a>(
    lines: impl Iterator<Item = &'a str>,
    limits: Limits,
) -> Result<Vec<(String, String)>, HttpError> {
    let mut headers = Vec::new();
    for line in lines {
        if headers.len() >= limits.headers {
            return Err(HttpError::new(
                431,
                "http.too_many_headers",
                format!("the request has more than {} headers", limits.headers),
            ));
        }
        if line.starts_with([' ', '\t']) {
            return Err(bad(
                "http.bad_header",
                "folded header lines are not supported",
            ));
        }
        let (name, value) = line.split_once(':').ok_or_else(|| {
            bad(
                "http.bad_header",
                format!("header line '{line}' has no ':'"),
            )
        })?;
        if name.is_empty() || !name.bytes().all(is_token_byte) {
            return Err(bad(
                "http.bad_header",
                format!("header name '{name}' is not a token"),
            ));
        }
        headers.push((name.to_ascii_lowercase(), value.trim().to_owned()));
    }
    Ok(headers)
}

fn is_token_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)
}

fn head_too_large(limits: Limits) -> HttpError {
    HttpError::new(
        431,
        "http.head_too_large",
        format!("the request head exceeds {} bytes", limits.head),
    )
}

fn timeout() -> HttpError {
    HttpError::new(
        408,
        "http.timeout",
        "the request did not arrive in time; send it in one go",
    )
}

fn bad(code: &'static str, message: impl Into<String>) -> HttpError {
    HttpError::new(400, code, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

    fn parse(raw: &[u8], limits: Limits) -> Result<HttpRequest, HttpError> {
        parse_with(raw, limits, Duration::from_millis(200))
    }

    /// Parse `raw`, keeping the client socket open for `hold`.
    fn parse_with(raw: &[u8], limits: Limits, hold: Duration) -> Result<HttpRequest, HttpError> {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let raw = raw.to_vec();
        let writer = std::thread::spawn(move || {
            let mut c = TcpStream::connect(addr).expect("connect");
            let _ = c.write_all(&raw);
            std::thread::sleep(hold);
        });
        let (mut s, _) = listener.accept().expect("accept");
        let out = read_head(&mut s, limits).and_then(|(mut req, rest)| {
            read_body(&mut s, &mut req, rest, limits)?;
            Ok(req)
        });
        writer.join().expect("join");
        out
    }

    #[test]
    fn parses_a_post_with_a_body() {
        let r = parse(
            b"POST /api/cmd?x=1&y HTTP/1.1\r\nHost: a\r\nContent-Length: 5\r\n\r\nhello",
            Limits::default(),
        )
        .expect("parse");
        assert_eq!(r.method, Method::Post);
        assert_eq!(r.path, "/api/cmd");
        assert_eq!(r.query_param("x"), Some("1"));
        assert_eq!(r.query_param("y"), Some(""));
        assert_eq!(r.header("host").expect("one"), Some("a"));
        assert_eq!(r.body, b"hello");
    }

    #[test]
    fn rejects_oversize_heads_bodies_and_chunked() {
        let small = Limits {
            head: 64,
            ..Limits::default()
        };
        let long = format!("GET / HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(200));
        assert_eq!(parse(long.as_bytes(), small).expect_err("head").status, 431);
        let body = Limits {
            body: 4,
            ..Limits::default()
        };
        let err =
            parse(b"POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\nhello", body).expect_err("body");
        assert_eq!(err.status, 413);
        let err = parse(
            b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n",
            Limits::default(),
        )
        .expect_err("chunked");
        assert_eq!(err.status, 501);
    }

    #[test]
    fn rejects_malformed_input_and_slow_clients() {
        for raw in [
            &b"GARBAGE\r\n\r\n"[..],
            b"GET nopath HTTP/1.1\r\n\r\n",
            b"GET / HTTP/1.1\r\nno colon\r\n\r\n",
            b"POST / HTTP/1.1\r\nContent-Length: x\r\n\r\n",
            b"POST / HTTP/1.1\r\nContent-Length: 1\r\nContent-Length: 1\r\n\r\na",
        ] {
            assert_eq!(parse(raw, Limits::default()).expect_err("bad").status, 400);
        }
        assert_eq!(
            parse(b"GET / HTTP/2\r\n\r\n", Limits::default())
                .expect_err("version")
                .status,
            505
        );
        let quick = Limits {
            head_deadline: Duration::from_millis(50),
            ..Limits::default()
        };
        assert_eq!(
            parse(b"GET / HTTP/1.1\r\n", quick)
                .expect_err("slow")
                .status,
            408
        );
    }

    #[test]
    fn idle_clients_time_out_before_the_deadline() {
        let idle = Limits {
            head_deadline: Duration::from_secs(30),
            body_deadline: Duration::from_secs(30),
            idle: Duration::from_millis(100),
            ..Limits::default()
        };
        // The client stays silent for 2 s, then closes. Without the idle
        // wait the read would end with the close (400), not with 408.
        let err =
            parse_with(b"GET / HTTP/1.1\r\n", idle, Duration::from_secs(2)).expect_err("idle head");
        assert_eq!(err.status, 408);
        let err = parse_with(
            b"POST / HTTP/1.1\r\nContent-Length: 9\r\n\r\nabc",
            idle,
            Duration::from_secs(2),
        )
        .expect_err("idle body");
        assert_eq!(err.status, 408);
    }

    #[test]
    fn the_body_waits_for_read_body() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let writer = std::thread::spawn(move || {
            let mut c = TcpStream::connect(addr).expect("connect");
            let _ = c.write_all(b"POST / HTTP/1.1\r\nContent-Length: 20000000\r\n\r\n");
            std::thread::sleep(Duration::from_millis(200));
        });
        let (mut s, _) = listener.accept().expect("accept");
        let (req, rest) = read_head(&mut s, Limits::default()).expect("head");
        assert_eq!(req.length, 20_000_000, "the head alone is read");
        assert!(req.body.is_empty());
        let mut req = req;
        let err = read_body(&mut s, &mut req, rest, Limits::default()).expect_err("too large");
        assert_eq!(err.status, 413);
        writer.join().expect("join");
    }
}
