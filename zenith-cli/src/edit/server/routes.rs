//! One connection: read the request, run the checks, route it, write the
//! response.

use std::net::TcpStream;
use std::time::Duration;

use super::api;
use super::assets;
use super::guard::Auth;
use super::shared::Shared;
use crate::edit::http::{HttpError, HttpRequest, Method, PAGE_CSP, Response, read_request};

/// Time one response write may take.
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// Serve one connection to the end.
pub(super) fn serve(shared: &Shared, mut stream: TcpStream) {
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
    let _ = stream.set_nodelay(true);
    let req = match read_request(&mut stream, shared.limits) {
        Ok(req) => req,
        Err(e) => {
            let _ = Response::error(&e).write_to(&mut stream);
            return;
        }
    };
    if let Err(e) = shared.guard.check_site(&req) {
        let _ = Response::error(&e).write_to(&mut stream);
        return;
    }
    if req.method == Method::Get && req.path == "/api/events" {
        if let Err(e) = shared.guard.check_token(&req, false) {
            let _ = Response::error(&e).write_to(&mut stream);
            return;
        }
        if let Err((mut stream, e)) = api::events(shared, stream) {
            let _ = Response::error(&e).write_to(&mut stream);
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

/// Route an API or page request that passed the site checks.
fn route(shared: &Shared, req: &HttpRequest) -> Result<Response, HttpError> {
    let path = req.path.as_str();
    if path.starts_with("/api/") {
        shared.guard.check_token(req, false)?;
        return match (&req.method, path) {
            (Method::Post, "/api/cmd") => api::cmd(shared, req),
            (Method::Post, "/api/save") => api::save(shared, req),
            (Method::Post, "/api/shutdown") => api::shutdown(shared, req),
            (Method::Get, "/api/state") => Ok(api::state(shared, req)),
            (Method::Get, p) if p.starts_with("/api/image/") => api::image(shared, p),
            (
                Method::Get | Method::Post | Method::Other(_),
                "/api/cmd" | "/api/save" | "/api/shutdown" | "/api/state" | "/api/events",
            ) => Err(method_not_allowed(path)),
            (Method::Get | Method::Post | Method::Other(_), _) => Err(api::not_found(path)),
        };
    }
    page(shared, req)
}

/// A page or static asset. Only `GET`. `/?token=` sets the page cookie.
/// A matching `If-None-Match` gets 304 with no body.
fn page(shared: &Shared, req: &HttpRequest) -> Result<Response, HttpError> {
    match &req.method {
        Method::Get => {}
        Method::Post | Method::Other(_) => return Err(method_not_allowed(&req.path)),
    }
    let auth = shared
        .guard
        .check_token(req, assets::is_page(&req.path))
        .map_err(|_| {
            HttpError::new(
                401,
                "edit.unauthorized",
                "open the URL `zenith edit` printed: it carries the token",
            )
        })?;
    let asset = assets::lookup(&req.path).ok_or_else(|| api::not_found(&req.path))?;
    let fresh = req
        .header("if-none-match")?
        .is_some_and(|tags| assets::matches_etag(tags, asset.etag));
    // `no-cache`: the browser keeps a copy but checks it on every load, so a
    // rebuilt binary never serves an old page and an unchanged file costs a
    // 304 with no body.
    let mut response = if fresh {
        Response::new(304, asset.content_type, Vec::new())
    } else {
        Response::new(200, asset.content_type, asset.bytes.to_vec())
    }
    .with_header("Cache-Control", "no-cache")
    .with_header("ETag", asset.etag);
    if asset.content_type.starts_with("text/html") {
        response = response.with_header("Content-Security-Policy", PAGE_CSP);
    }
    match auth {
        Auth::Query => Ok(response.with_header("Set-Cookie", shared.guard.set_cookie())),
        Auth::Bearer | Auth::Cookie => Ok(response),
    }
}

fn method_not_allowed(path: &str) -> HttpError {
    HttpError::new(
        405,
        "edit.method_not_allowed",
        format!("'{path}' does not take this method; see the route table in `zenith edit --help`"),
    )
}
