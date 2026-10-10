//! One connection: read the head, run the checks, read the body, route it,
//! write the response.
//!
//! The body of an API request is read only after the `Host`, `Origin`, and
//! token checks pass, so a client without the token never makes the server
//! wait for or hold a body.

use std::io::Read;
use std::net::{Shutdown, TcpStream};
use std::time::Duration;

use super::api;
use super::assets;
use super::shared::Shared;
use crate::edit::http::{HttpError, HttpRequest, Method, PAGE_CSP, Response, read_body, read_head};

/// Time one response write may take.
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
/// After an error reply, unread request bytes are drained for at most this
/// long, so closing the socket does not reset the connection before the
/// client reads the reply.
const LINGER: Duration = Duration::from_millis(500);
/// Most request bytes drained after an error reply.
const LINGER_BYTES: u64 = 64 * 1024;

/// Serve one connection to the end.
pub(super) fn serve(shared: &Shared, mut stream: TcpStream) {
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
    let _ = stream.set_nodelay(true);
    let (mut req, leftover) = match read_head(&mut stream, shared.limits) {
        Ok(head) => head,
        Err(e) => return refuse(stream, &e),
    };
    if let Err(e) = shared.guard.check_site(&req) {
        return refuse(stream, &e);
    }
    if !req.path.starts_with("/api/") {
        let response = page(&req).unwrap_or_else(|e| Response::error(&e));
        let _ = response.write_to(&mut stream);
        return;
    }
    if let Err(e) = shared.guard.check_token(&req) {
        return refuse(stream, &e);
    }
    shared.authenticated();
    if let Err(e) = read_body(&mut stream, &mut req, leftover, shared.limits) {
        return refuse(stream, &e);
    }
    if req.method == Method::Get && req.path == "/api/events" {
        if let Err((stream, e)) = api::events(shared, stream) {
            refuse(stream, &e);
        }
        return;
    }
    let stop = req.method == Method::Post && req.path == "/api/shutdown";
    let response = route(shared, &req).unwrap_or_else(|e| Response::error(&e));
    let accepted = response.status == 200;
    let _ = response.write_to(&mut stream);
    drop(stream);
    if stop && accepted {
        shared.request_stop();
    }
}

/// Write the error reply, then close without resetting the connection.
fn refuse(mut stream: TcpStream, error: &HttpError) {
    if Response::error(error).write_to(&mut stream).is_err() {
        return;
    }
    let _ = stream.shutdown(Shutdown::Write);
    let _ = stream.set_read_timeout(Some(LINGER));
    let _ = std::io::copy(&mut (&stream).take(LINGER_BYTES), &mut std::io::sink());
}

/// Route an API request that passed every check.
fn route(shared: &Shared, req: &HttpRequest) -> Result<Response, HttpError> {
    let path = req.path.as_str();
    match (&req.method, path) {
        (Method::Post, "/api/cmd") => api::cmd(shared, req),
        (Method::Post, "/api/save") => api::save(shared, req),
        (Method::Post, "/api/shutdown") => api::shutdown(shared, req),
        (Method::Get, "/api/state") => Ok(api::state(shared, req)),
        (Method::Get, p) if p.starts_with("/api/image/") => api::image(shared, p),
        (
            Method::Get | Method::Post | Method::Head | Method::Other(_),
            "/api/cmd" | "/api/save" | "/api/shutdown" | "/api/state" | "/api/events",
        ) => Err(method_not_allowed(path)),
        (Method::Get | Method::Post | Method::Head | Method::Other(_), _) => {
            Err(api::not_found(path))
        }
    }
}

/// A page or static asset: `GET`, or `HEAD` for the headers alone. These
/// are the public files of the editor page, so they need no token: the page
/// reads the token from the URL fragment and sends it on every API call. A
/// matching `If-None-Match` gets 304 with no body.
fn page(req: &HttpRequest) -> Result<Response, HttpError> {
    let head = match &req.method {
        Method::Get => false,
        Method::Head => true,
        Method::Post | Method::Other(_) => return Err(method_not_allowed(&req.path)),
    };
    let asset = assets::lookup(&req.path).ok_or_else(|| api::not_found(&req.path))?;
    let fresh = req
        .header("if-none-match")?
        .is_some_and(|tags| assets::matches_etag(tags, asset.etag));
    let mut response = if fresh {
        Response::new(304, asset.content_type, Vec::new())
    } else {
        Response::new(200, asset.content_type, asset.bytes.to_vec())
    }
    .with_header("ETag", asset.etag);
    if assets::is_page(&req.path) {
        // The page is never stored: a stored copy outlives the run.
        response = response.with_header("Cache-Control", "no-store");
    } else {
        // `no-cache`: the browser keeps a copy but checks it on every load,
        // so a rebuilt binary never serves an old module and an unchanged
        // file costs a 304 with no body.
        response = response.with_header("Cache-Control", "no-cache");
    }
    if asset.content_type.starts_with("text/html") {
        response = response.with_header("Content-Security-Policy", PAGE_CSP);
    }
    Ok(if head { response.for_head() } else { response })
}

fn method_not_allowed(path: &str) -> HttpError {
    HttpError::new(
        405,
        "edit.method_not_allowed",
        format!("'{path}' does not take this method; see the route table in `zenith edit --help`"),
    )
}
