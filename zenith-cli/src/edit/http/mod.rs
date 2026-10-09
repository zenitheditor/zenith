//! A minimal HTTP/1.1 layer over `std::net`: one request per connection,
//! `Content-Length` bodies only, hard size and time limits. Wiring and the
//! shared error type.
//!
//! - `request` — read and parse one request.
//! - `response` — write one response.

mod request;
mod response;

pub(crate) use request::{HttpRequest, Limits, Method, read_request};
pub(crate) use response::{PAGE_CSP, Response, reason};

/// An HTTP-level error: status, stable code, and a message naming the next
/// action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HttpError {
    pub(crate) status: u16,
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

impl HttpError {
    /// An error with `status`, `code`, and `message`.
    pub(crate) fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
}
