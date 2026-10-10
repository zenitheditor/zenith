//! The `/api/*` handlers.

use std::io::Write;
use std::net::TcpStream;

use serde::Deserialize;
use serde_json::{Value, json};
use zenith_core::fix::unified_diff;
use zenith_editor::Request;

use super::shared::Shared;
use crate::edit::doc::{DocState, Event, Ran, image_meta};
use crate::edit::http::{HttpError, HttpRequest, Response, reason};

/// Body of `POST /api/cmd`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CmdBody {
    command: String,
    #[serde(default)]
    params: Value,
    #[serde(default)]
    version: Option<u64>,
    /// Add a unified diff of the text change to the reply.
    #[serde(default)]
    diff: bool,
}

/// Body of `POST /api/save`.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct SaveBody {
    #[serde(default)]
    version: Option<u64>,
    #[serde(default)]
    overwrite: bool,
    #[serde(default)]
    diff: bool,
}

/// Body of `POST /api/shutdown`.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct ShutdownBody {
    #[serde(default)]
    force: bool,
}

/// `POST /api/cmd`: run one command.
pub(super) fn cmd(shared: &Shared, req: &HttpRequest) -> Result<Response, HttpError> {
    let body: CmdBody = json_body(req)?;
    let client = client_id(req)?;
    let request = Request {
        command: body.command,
        params: body.params,
        version: body.version,
    };
    Ok(run(shared, &request, client.as_deref(), body.diff))
}

/// `POST /api/save`: `file.save` with `{version?, overwrite?}`.
pub(super) fn save(shared: &Shared, req: &HttpRequest) -> Result<Response, HttpError> {
    let body: SaveBody = if req.body.is_empty() {
        SaveBody::default()
    } else {
        json_body(req)?
    };
    let client = client_id(req)?;
    let request = Request {
        command: "file.save".into(),
        params: json!({ "overwrite": body.overwrite }),
        version: body.version,
    };
    Ok(run(shared, &request, client.as_deref(), body.diff))
}

/// `GET /api/state[?text=1]`: the session summary.
pub(super) fn state(shared: &Shared, req: &HttpRequest) -> Response {
    let with_text = matches!(req.query_param("text"), Some("1" | "true"));
    let doc = shared.doc();
    let mut summary = doc.summary(with_text);
    if let Some(obj) = summary.as_object_mut() {
        obj.insert("ok".into(), Value::Bool(true));
    }
    Response::json(200, &summary)
}

/// `GET /api/image/<sha256>.png`: a render from this run. The URL names
/// the bytes, so the response is cacheable for good.
pub(super) fn image(shared: &Shared, path: &str) -> Result<Response, HttpError> {
    let sha = path
        .strip_prefix("/api/image/")
        .and_then(|p| p.strip_suffix(".png"))
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| not_found(path))?;
    let png = shared.images().get(sha).ok_or_else(|| {
        HttpError::new(
            404,
            "edit.image_expired",
            format!(
                "image {sha} is not in the cache (it keeps the last 32 renders); send doc.render again"
            ),
        )
    })?;
    Ok(Response::new(200, "image/png", png.as_ref().clone())
        .with_header("Cache-Control", "private, max-age=31536000, immutable"))
}

/// `POST /api/shutdown {force?}`: stop the server. Refused with 409
/// `edit.unsaved` while the session is dirty, unless `force`.
pub(super) fn shutdown(shared: &Shared, req: &HttpRequest) -> Result<Response, HttpError> {
    let body: ShutdownBody = if req.body.is_empty() {
        ShutdownBody::default()
    } else {
        json_body(req)?
    };
    let doc = shared.doc();
    if doc.dirty() && !body.force {
        return Err(HttpError::new(
            409,
            "edit.unsaved",
            "the session has unsaved edits; send file.save first, or shutdown with \
             {\"force\": true} to drop them",
        ));
    }
    Ok(Response::json(
        200,
        &json!({ "ok": true, "stopping": true, "dirty": doc.dirty() }),
    ))
}

/// `GET /api/events`: hand the socket to the event thread.
///
/// # Errors
///
/// 503 `edit.too_many_streams` when the stream cap is reached.
pub(super) fn events(shared: &Shared, mut stream: TcpStream) -> Result<(), (TcpStream, HttpError)> {
    if !shared.hub.reserve() {
        return Err((
            stream,
            HttpError::new(
                503,
                "edit.too_many_streams",
                "too many open event streams; close another editor tab and retry",
            ),
        ));
    }
    let head = format!(
        "HTTP/1.1 200 {}\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\n\
         Connection: close\r\nX-Content-Type-Options: nosniff\r\nX-Accel-Buffering: no\r\n\r\n",
        reason(200)
    );
    if stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.flush())
        .is_err()
    {
        shared.hub.release();
        return Ok(());
    }
    // Join under the document lock, so the first event is in order with
    // every event after it.
    let doc = shared.doc();
    shared
        .hub
        .join(stream, Event::new("state", doc.summary(false)));
    Ok(())
}

/// Run `request` and shape the reply envelope. Events go to the hub while
/// the document lock is held, so they stay in version order.
fn run(shared: &Shared, request: &Request, client: Option<&str>, diff: bool) -> Response {
    let mut doc = shared.doc();
    let ran = doc.execute(request, client);
    let Ran {
        result,
        image,
        work,
        events,
        change,
    } = ran;
    shared.note_clean(doc.dirty());
    shared.hub.send(events);
    let mut out = json!({
        "ok": result.is_ok(),
        "command": request.command,
        "version": doc.session().version,
        "dirty": doc.dirty(),
        "work": work,
    });
    if let Some(obj) = out.as_object_mut() {
        match result {
            Ok(value) => {
                obj.insert("result".into(), value);
            }
            Err(error) => {
                obj.insert("error".into(), json!(error));
            }
        }
        if let Some(image) = image {
            let sha = image.sha256();
            let mut meta = image_meta(&image);
            if let Some(m) = meta.as_object_mut() {
                m.insert("url".into(), json!(format!("/api/image/{sha}.png")));
            }
            obj.insert("image".into(), meta);
            shared.images().insert(sha, image.png);
        }
        if diff && let Some(change) = &change {
            obj.insert("diff".into(), Value::String(diff_text(&doc, change)));
        }
    }
    Response::json(200, &out)
}

fn diff_text(doc: &DocState, change: &crate::edit::doc::TextChange) -> String {
    let label = doc.path().file_name().map_or_else(
        || "document.zen".into(),
        |n| n.to_string_lossy().into_owned(),
    );
    unified_diff(&label, &change.before, &change.after)
}

/// Decode the JSON body. The request must say `application/json`, which a
/// cross-origin page cannot send without a CORS preflight this server
/// never answers.
fn json_body<T: for<'de> Deserialize<'de>>(req: &HttpRequest) -> Result<T, HttpError> {
    let ty = req.header("content-type")?.unwrap_or("");
    if !ty
        .split(';')
        .next()
        .is_some_and(|t| t.trim().eq_ignore_ascii_case("application/json"))
    {
        return Err(HttpError::new(
            415,
            "edit.bad_content_type",
            format!("Content-Type '{ty}' is not application/json; send a JSON body"),
        ));
    }
    serde_json::from_slice(&req.body).map_err(|e| {
        HttpError::new(
            400,
            "edit.bad_json",
            format!("the body is not the expected JSON: {e}"),
        )
    })
}

/// The `X-Zenith-Client` header: 1 to 64 of `[A-Za-z0-9_-]`.
fn client_id(req: &HttpRequest) -> Result<Option<String>, HttpError> {
    match req.header("x-zenith-client")? {
        None => Ok(None),
        Some(id)
            if !id.is_empty()
                && id.len() <= 64
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') =>
        {
            Ok(Some(id.to_owned()))
        }
        Some(id) => Err(HttpError::new(
            400,
            "edit.bad_client",
            format!("X-Zenith-Client '{id}' must be 1 to 64 letters, digits, '-' or '_'"),
        )),
    }
}

/// 404 for an unknown path.
pub(super) fn not_found(path: &str) -> HttpError {
    HttpError::new(
        404,
        "edit.not_found",
        format!("no route '{path}'; the API is /api/cmd, /api/state, /api/save, /api/events"),
    )
}
