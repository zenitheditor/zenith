//! The `zenith_editor_*` MCP tools: open a document in an editor session,
//! run editor commands on it, render it, list sessions, and attach to a
//! running `zenith edit`.
//!
//! Every argument is decoded and checked before any file is read or any
//! request is sent.

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Value, json};
use zenith_core::fix::unified_diff;
use zenith_editor::Request;

use super::registry::{Entry, registry};
use crate::edit::client::Remote;
use crate::edit::doc::{DocState, Ran, Target, image_meta};
use crate::mcp::protocol::ToolResult;
use crate::mcp::serialize::compact;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenArgs {
    path: String,
    #[serde(default)]
    root: Option<String>,
    #[serde(default)]
    discard: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandArgs {
    #[serde(default)]
    session: Option<String>,
    #[serde(default)]
    path: Option<String>,
    command: String,
    #[serde(default)]
    params: Value,
    #[serde(default)]
    version: Option<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenderArgs {
    #[serde(default)]
    session: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    page: Option<u64>,
    #[serde(default)]
    scale: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AttachArgs {
    url: String,
    #[serde(default)]
    token: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoArgs {}

/// `zenith_editor_open {path, root?, discard?}`.
pub(crate) fn open(args: &Value) -> ToolResult {
    let a: OpenArgs = match decode(args, "zenith_editor_open") {
        Ok(a) => a,
        Err(r) => return r,
    };
    let target = match Target::resolve(
        &PathBuf::from(&a.path),
        a.root.as_deref().map(PathBuf::from).as_deref(),
    ) {
        Ok(t) => t,
        Err(e) => return ToolResult::err(format!("{}: {}", e.code, e.message)),
    };
    let mut reg = registry();
    let existing = reg.local_id(&target.path);
    if let Some(id) = &existing
        && let Some(Entry::Local(doc)) = reg.sessions.get(id)
        && doc.dirty()
        && !a.discard
    {
        return ToolResult::err(format!(
            "session {id} for '{}' has unsaved edits; save them with zenith_editor_command \
             file.save, or pass discard=true to reopen from disk",
            target.path.display()
        ));
    }
    let (doc, reply) = match DocState::open(target) {
        Ok(opened) => opened,
        Err(e) => return ToolResult::err(format!("{}: {}", e.code, e.message)),
    };
    let id = existing.unwrap_or_else(|| reg.next_id('e'));
    let out = json!({
        "session": id,
        "path": doc.path(),
        "version": doc.session().version,
        "valid": reply.get("valid").cloned().unwrap_or(Value::Null),
        "stale": reply.get("stale").cloned().unwrap_or(Value::Null),
        "page_count": reply.get("page_count").cloned().unwrap_or(Value::Null),
        "diagnostics": reply.get("diagnostics").cloned().unwrap_or(json!([])),
        "commands": "send command commands.list for every command and its params",
    });
    reg.sessions.insert(id, Entry::Local(Box::new(doc)));
    let text = compact(&out);
    ToolResult::ok(out, text)
}

/// `zenith_editor_command {session|path, command, params?, version?}`.
pub(crate) fn command(args: &Value) -> ToolResult {
    let a: CommandArgs = match decode(args, "zenith_editor_command") {
        Ok(a) => a,
        Err(r) => return r,
    };
    if a.command.is_empty() {
        return ToolResult::err(
            "missing 'command'; send commands.list to see every command".to_owned(),
        );
    }
    if !matches!(a.params, Value::Null | Value::Object(_)) {
        return ToolResult::err("'params' must be an object".to_owned());
    }
    let request = Request {
        command: a.command,
        params: a.params,
        version: a.version,
    };
    run(a.session.as_deref(), a.path.as_deref(), &request)
}

/// `zenith_editor_render {session|path, page?, scale?}`.
pub(crate) fn render(args: &Value) -> ToolResult {
    let a: RenderArgs = match decode(args, "zenith_editor_render") {
        Ok(a) => a,
        Err(r) => return r,
    };
    if a.page == Some(0) {
        return ToolResult::err("'page' is 1-based; pass 1 or more".to_owned());
    }
    if let Some(s) = a.scale
        && !(s.is_finite() && s > 0.0 && s <= 4.0)
    {
        return ToolResult::err(format!("'scale' {s} must be in 0 < scale <= 4"));
    }
    let mut params = serde_json::Map::new();
    if let Some(p) = a.page {
        params.insert("page".into(), json!(p));
    }
    if let Some(s) = a.scale {
        params.insert("scale".into(), json!(s));
    }
    let request = Request::new("doc.render", Value::Object(params));
    run(a.session.as_deref(), a.path.as_deref(), &request)
}

/// One session as listed, before attached servers are asked for state.
enum Listed {
    Local(Value),
    Attached {
        id: String,
        remote: Remote,
        path: String,
    },
}

/// `zenith_editor_sessions {}`.
pub(crate) fn sessions(args: &Value) -> ToolResult {
    if let Err(r) = decode::<NoArgs>(args, "zenith_editor_sessions") {
        return r;
    }
    let listed: Vec<Listed> = registry()
        .sessions
        .iter()
        .map(|(id, entry)| match entry {
            Entry::Local(doc) => Listed::Local(json!({
                "session": id,
                "kind": "local",
                "path": doc.path(),
                "version": doc.session().version,
                "dirty": doc.dirty(),
                "valid": doc.session().valid,
                "conflict": doc.conflict(),
            })),
            Entry::Remote { remote, path } => Listed::Attached {
                id: id.clone(),
                remote: remote.clone(),
                path: path.clone(),
            },
        })
        .collect();
    // Remote state is read after the registry lock is released.
    let list: Vec<Value> = listed
        .into_iter()
        .map(|entry| match entry {
            Listed::Local(v) => v,
            Listed::Attached { id, remote, path } => match remote.get_json("/api/state") {
                Ok(state) => json!({
                    "session": id,
                    "kind": "attached",
                    "server": remote.addr().to_string(),
                    "reachable": true,
                    "path": state["path"],
                    "version": state["version"],
                    "dirty": state["dirty"],
                    "valid": state["valid"],
                    "conflict": state["conflict"],
                }),
                Err(e) => json!({
                    "session": id,
                    "kind": "attached",
                    "server": remote.addr().to_string(),
                    "reachable": false,
                    "path": path,
                    "error": e,
                }),
            },
        })
        .collect();
    let out = json!({ "sessions": list });
    let text = compact(&out);
    ToolResult::ok(out, text)
}

/// `zenith_editor_attach {url, token?}`.
pub(crate) fn attach(args: &Value) -> ToolResult {
    let a: AttachArgs = match decode(args, "zenith_editor_attach") {
        Ok(a) => a,
        Err(r) => return r,
    };
    let remote = match Remote::parse(&a.url, a.token.as_deref()) {
        Ok(r) => r,
        Err(e) => return ToolResult::err(e),
    };
    let state = match remote.get_json("/api/state") {
        Ok(s) => s,
        Err(e) => return ToolResult::err(e),
    };
    let path = state["path"].as_str().unwrap_or_default().to_owned();
    let mut reg = registry();
    let id = reg.remote_id(&remote).unwrap_or_else(|| reg.next_id('r'));
    reg.sessions.insert(
        id.clone(),
        Entry::Remote {
            remote: remote.clone(),
            path: path.clone(),
        },
    );
    let out = json!({
        "session": id,
        "server": remote.addr().to_string(),
        "path": path,
        "version": state["version"],
        "dirty": state["dirty"],
        "valid": state["valid"],
        "conflict": state["conflict"],
    });
    let text = compact(&out);
    ToolResult::ok(out, text)
}

/// Run `request` on the session named by `session` or `path`. A `path`
/// with no session opens one.
fn run(session: Option<&str>, path: Option<&str>, request: &Request) -> ToolResult {
    let target = match (session, path) {
        (Some(_), Some(_)) | (None, None) => {
            return ToolResult::err("pass exactly one of 'session' and 'path'".to_owned());
        }
        (Some(_), None) => None,
        (None, Some(p)) => match Target::resolve(&PathBuf::from(p), None) {
            Ok(t) => Some(t),
            Err(e) => return ToolResult::err(format!("{}: {}", e.code, e.message)),
        },
    };
    let mut reg = registry();
    let id = match (session, target) {
        (Some(id), _) => {
            if !reg.sessions.contains_key(id) {
                return ToolResult::err(format!(
                    "no editor session '{id}'; zenith_editor_sessions lists them, \
                     zenith_editor_open starts one"
                ));
            }
            id.to_owned()
        }
        (None, Some(target)) => match reg.local_id(&target.path) {
            Some(id) => id,
            None => match DocState::open(target) {
                Ok((doc, _)) => {
                    let id = reg.next_id('e');
                    reg.sessions.insert(id.clone(), Entry::Local(Box::new(doc)));
                    id
                }
                Err(e) => return ToolResult::err(format!("{}: {}", e.code, e.message)),
            },
        },
        (None, None) => return ToolResult::err("pass 'session' or 'path'".to_owned()),
    };
    match reg.sessions.get_mut(&id) {
        Some(Entry::Local(doc)) => run_local(&id, doc, request),
        Some(Entry::Remote { remote, .. }) => {
            let remote = remote.clone();
            drop(reg);
            run_remote(&id, &remote, request)
        }
        None => ToolResult::err(format!("no editor session '{id}'")),
    }
}

/// Run `request` on a local session. A disk change since the last call is
/// taken first (reload when clean, conflict when dirty) and reported under
/// `external`.
fn run_local(id: &str, doc: &mut DocState, request: &Request) -> ToolResult {
    let external: Vec<Value> = doc
        .poll_disk(false)
        .into_iter()
        .map(|e| json!({ "event": e.name, "data": e.data }))
        .collect();
    let Ran {
        result,
        image,
        work,
        change,
        ..
    } = doc.execute(request, None);
    let ok = result.is_ok();
    let mut out = json!({
        "session": id,
        "ok": ok,
        "command": request.command,
        "version": doc.session().version,
        "dirty": doc.dirty(),
        "conflict": doc.conflict(),
        "work": work,
    });
    if let Some(obj) = out.as_object_mut() {
        match result {
            Ok(v) => {
                obj.insert("result".into(), v);
            }
            Err(e) => {
                obj.insert("error".into(), json!(e));
            }
        }
        if let Some(img) = &image {
            obj.insert("image".into(), image_meta(img));
        }
        if let Some(change) = change {
            let label = doc.path().file_name().map_or_else(
                || "document.zen".into(),
                |n| n.to_string_lossy().into_owned(),
            );
            obj.insert(
                "diff".into(),
                Value::String(unified_diff(&label, &change.before, &change.after)),
            );
        }
        if !external.is_empty() {
            obj.insert("external".into(), Value::Array(external));
        }
    }
    let text = compact(&out);
    let result = ToolResult::report(out, text, !ok);
    match image {
        Some(img) => result.with_png(&img.png),
        None => result,
    }
}

/// Forward `request` to an attached `zenith edit` server. The PNG of a
/// render is fetched from its image route and inlined.
fn run_remote(id: &str, remote: &Remote, request: &Request) -> ToolResult {
    let body = json!({
        "command": request.command,
        "params": request.params,
        "version": request.version,
        "diff": true,
    });
    let mut reply = match remote.post_json("/api/cmd", &body) {
        Ok(r) => r,
        Err(e) => return ToolResult::err(e),
    };
    let png = match reply["image"]["url"].as_str() {
        Some(url) => match remote.get_bytes(url) {
            Ok(bytes) => Some(bytes),
            Err(e) => return ToolResult::err(e),
        },
        None => None,
    };
    let ok = reply["ok"].as_bool().unwrap_or(false);
    if let Some(obj) = reply.as_object_mut() {
        obj.insert("session".into(), Value::String(id.to_owned()));
    }
    let text = compact(&reply);
    let result = ToolResult::report(reply, text, !ok);
    match png {
        Some(png) => result.with_png(&png),
        None => result,
    }
}

/// Decode `args` into `T`, or the tool error naming the bad field.
fn decode<T: for<'de> Deserialize<'de>>(args: &Value, tool: &str) -> Result<T, ToolResult> {
    let args = match args {
        Value::Null => json!({}),
        other => other.clone(),
    };
    serde_json::from_value(args)
        .map_err(|e| ToolResult::err(format!("invalid arguments for {tool}: {e}")))
}
