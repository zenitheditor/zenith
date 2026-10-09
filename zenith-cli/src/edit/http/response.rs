//! Write one HTTP/1.1 response. Every response closes the connection.

use std::io::Write;

use serde_json::{Value, json};

use super::HttpError;

/// The Content-Security-Policy of HTML pages: same-origin scripts, styles,
/// workers, and requests. Images may also be `blob:` and `data:` URLs.
pub(crate) const PAGE_CSP: &str = "default-src 'self'; img-src 'self' blob: data:; \
style-src 'self' 'unsafe-inline'; worker-src 'self' blob:; connect-src 'self'; \
object-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'";

/// One response: status, extra headers, body.
#[derive(Debug, Clone)]
pub(crate) struct Response {
    pub(crate) status: u16,
    pub(crate) headers: Vec<(&'static str, String)>,
    pub(crate) body: Vec<u8>,
}

impl Response {
    /// A response with `content_type` and `body`, not cacheable.
    pub(crate) fn new(status: u16, content_type: &str, body: Vec<u8>) -> Self {
        Self {
            status,
            headers: vec![
                ("Content-Type", content_type.to_owned()),
                ("Cache-Control", "no-store".to_owned()),
            ],
            body,
        }
    }

    /// A JSON response.
    pub(crate) fn json(status: u16, value: &Value) -> Self {
        Self::new(
            status,
            "application/json; charset=utf-8",
            value.to_string().into_bytes(),
        )
    }

    /// The JSON error response of `error`: `{ok: false, error: {code,
    /// message}}`.
    pub(crate) fn error(error: &HttpError) -> Self {
        let mut r = Self::json(
            error.status,
            &json!({
                "ok": false,
                "error": { "code": error.code, "message": error.message },
            }),
        );
        if error.status == 405 {
            r.headers.push(("Allow", "GET, POST".to_owned()));
        }
        r
    }

    /// Replace the value of header `name`, or add it.
    pub(crate) fn with_header(mut self, name: &'static str, value: impl Into<String>) -> Self {
        let value = value.into();
        match self.headers.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = value,
            None => self.headers.push((name, value)),
        }
        self
    }

    /// Write the status line, the headers, and the body.
    ///
    /// # Errors
    ///
    /// The socket write error.
    pub(crate) fn write_to(&self, out: &mut impl Write) -> std::io::Result<()> {
        let mut head = format!(
            "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
            self.status,
            reason(self.status),
            self.body.len()
        );
        let extra = self.headers.iter().map(|(n, v)| (*n, v.as_str()));
        for (name, value) in extra.chain(SECURITY_HEADERS.iter().copied()) {
            head.push_str(name);
            head.push_str(": ");
            head.push_str(value);
            head.push_str("\r\n");
        }
        head.push_str("\r\n");
        out.write_all(head.as_bytes())?;
        out.write_all(&self.body)?;
        out.flush()
    }
}

/// Headers every response carries. No CORS header is ever sent.
const SECURITY_HEADERS: &[(&str, &str)] = &[
    ("X-Content-Type-Options", "nosniff"),
    ("Referrer-Policy", "no-referrer"),
    ("X-Frame-Options", "DENY"),
    ("Cross-Origin-Resource-Policy", "same-origin"),
];

/// The reason phrase of `status`.
pub(crate) fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        421 => "Misdirected Request",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        505 => "HTTP Version Not Supported",
        _ => "Status",
    }
}
