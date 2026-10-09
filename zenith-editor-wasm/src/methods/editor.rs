//! `editor {session?, command, path?, files?, fonts?, global_config?, data?,
//! allow?, warn?, deny?}`: one `zenith-editor` command.
//!
//! `command` is the engine request `{command, params?, version?}` (see
//! `zenith_editor` for the command table). `session` is the session the
//! last reply returned; absent starts an empty one (send `doc.open` first).
//! The project fields work as for `render`.
//!
//! Result: `{session, result, work, png_base64?}`. `png_base64` is the page
//! raster of `doc.render` and `gesture.preview`, or only the window of a
//! `viewport` render (`result.rect` names it in device px). For
//! `commands.batch` it is the PNG of step `result.image_step`. A batch
//! decodes the session and `files` once for all its steps. A failed command
//! is the usual error envelope, with the engine's `code`, `message`,
//! `diagnostics`, and `offers`; the page keeps its session.

use std::collections::BTreeMap;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use zenith_editor::{DEFAULT_DOCUMENT_PATH, MemProject, Request, Session};
use zenith_pipeline::PolicyFlags;

use super::font_supply::decode_font_bytes;
use crate::protocol::ErrorBody;

/// The `params` of `editor`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EditorParams {
    /// The session from the last reply. Absent: an empty session.
    #[serde(default)]
    pub(crate) session: Option<Session>,
    /// The engine request.
    pub(crate) command: Request,
    /// The document's path in `files`. Defaults to `document.zen`.
    #[serde(default)]
    pub(crate) path: Option<String>,
    /// Project files: path → base64 bytes.
    #[serde(default)]
    pub(crate) files: BTreeMap<String, String>,
    /// Bundled font files the module does not embed: file name → base64.
    #[serde(default)]
    pub(crate) fonts: BTreeMap<String, String>,
    /// The path in `files` of the global config.
    #[serde(default)]
    pub(crate) global_config: Option<String>,
    /// The path in `files` of a JSON or CSV data file for `(data)` refs.
    #[serde(default)]
    pub(crate) data: Option<String>,
    /// Diagnostic codes to suppress.
    #[serde(default)]
    pub(crate) allow: Vec<String>,
    /// Diagnostic codes to force to Warning.
    #[serde(default)]
    pub(crate) warn: Vec<String>,
    /// Diagnostic codes to elevate to Error.
    #[serde(default)]
    pub(crate) deny: Vec<String>,
}

/// Run one engine command over the request's project.
pub(crate) fn run(params: EditorParams) -> Result<Value, ErrorBody> {
    let mut files = BTreeMap::new();
    for (path, b64) in &params.files {
        let bytes = STANDARD.decode(b64).map_err(|e| {
            ErrorBody::new(
                "request.invalid_params",
                format!(
                    "files['{path}'] is not valid base64: {e}; send each file as standard base64"
                ),
            )
        })?;
        files.insert(path.clone(), bytes);
    }
    let invalid = |e: zenith_editor::EditorError| ErrorBody {
        code: "request.invalid_params".to_owned(),
        ..ErrorBody::from(e)
    };
    let mut project = MemProject::new(params.path.as_deref().unwrap_or(DEFAULT_DOCUMENT_PATH))
        .with_files(files)
        .with_fonts(decode_font_bytes(&params.fonts)?)
        .map_err(invalid)?
        .with_flags(PolicyFlags {
            allow: params.allow,
            warn: params.warn,
            deny: params.deny,
        });
    if let Some(global) = params.global_config {
        project = project.with_global_config(global);
    }
    if let Some(data) = params.data.as_deref() {
        project = project.with_data_file(data).map_err(ErrorBody::from)?;
    }
    let session = params.session.unwrap_or_else(|| Session::new(""));
    let outcome = project.execute(session, &params.command);
    let result = outcome.result.map_err(ErrorBody::from)?;
    let mut reply = Map::new();
    reply.insert("session".to_owned(), json!(outcome.session));
    reply.insert("result".to_owned(), result);
    reply.insert("work".to_owned(), json!(outcome.work));
    if let Some(image) = outcome.image {
        reply.insert("png_base64".to_owned(), json!(STANDARD.encode(&image.png)));
    }
    Ok(Value::Object(reply))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLOWCHART: &str = include_str!("../../../examples/flowchart.zen");

    fn params(session: Option<Session>, command: Value) -> EditorParams {
        serde_json::from_value(json!({ "session": session, "command": command }))
            .expect("params decode")
    }

    #[test]
    fn open_render_commit_undo_round_trip() {
        let open = run(params(
            None,
            json!({ "command": "doc.open", "params": { "text": FLOWCHART } }),
        ))
        .expect("open");
        assert_eq!(open["result"]["valid"], true, "{open}");
        let session: Session = serde_json::from_value(open["session"].clone()).expect("session");
        let render = run(params(
            Some(session.clone()),
            json!({ "command": "doc.render", "params": { "scale": 0.5 } }),
        ))
        .expect("render");
        let png = STANDARD
            .decode(render["png_base64"].as_str().expect("png"))
            .expect("base64");
        assert!(png.starts_with(b"\x89PNG"));
        let hit = run(params(
            Some(session),
            json!({ "command": "select.hit", "params": { "x": 180, "y": 60 } }),
        ))
        .expect("hit");
        let session: Session = serde_json::from_value(hit["session"].clone()).expect("session");
        assert_eq!(session.selection.len(), 1, "{hit}");
        let version = session.version;
        let commit = run(params(
            Some(session),
            json!({ "command": "gesture.commit", "params": { "dx": 4, "dy": 2 }, "version": version }),
        ))
        .expect("commit");
        assert_eq!(commit["result"]["changed"], true, "{commit}");
        let session: Session = serde_json::from_value(commit["session"].clone()).expect("session");
        let undo = run(params(
            Some(session.clone()),
            json!({ "command": "history.undo", "version": session.version }),
        ))
        .expect("undo");
        assert_eq!(undo["session"]["text"], FLOWCHART);
    }

    #[test]
    fn batch_returns_every_step_and_the_render_png() {
        let open = run(params(
            None,
            json!({ "command": "doc.open", "params": { "text": FLOWCHART } }),
        ))
        .expect("open");
        let session: Session = serde_json::from_value(open["session"].clone()).expect("session");
        let text = format!("{FLOWCHART}\n");
        let steps = json!([
            { "command": "buffer.set", "params": { "text": text }, "version": session.version },
            { "command": "doc.render", "params": { "scale": 0.5 } },
            { "command": "doc.outline" },
        ]);
        let out = run(params(
            Some(session),
            json!({ "command": "commands.batch", "params": { "steps": steps } }),
        ))
        .expect("batch");
        assert_eq!(out["result"]["image_step"], 1, "{out}");
        let ok: Vec<&Value> = out["result"]["steps"]
            .as_array()
            .expect("steps")
            .iter()
            .map(|s| &s["ok"])
            .collect();
        assert_eq!(ok, [&json!(true), &json!(true), &json!(true)]);
        assert_eq!(out["session"]["text"], text.as_str());
        assert_eq!(out["work"]["parses"], 1);
        let png = STANDARD
            .decode(out["png_base64"].as_str().expect("png"))
            .expect("base64");
        assert!(png.starts_with(b"\x89PNG"));
    }

    #[test]
    fn engine_errors_use_the_error_envelope_with_offers() {
        let text = r##"zenith version=1 {
  project id="p" name="P"
  tokens format="zenith-token-v1" {
    token id="c" type="color" value="#102030"
    token id="s" type="dimension" value=(px)20
  }
  styles {}
  document id="d" title="D" {
    page id="pg" w=(px)100 h=(px)100 {
      rect id="r" x=(token)"s" y=(px)10 w=(px)20 h=(px)20 fill=(token)"c"
    }
  }
}
"##;
        let open = run(params(
            None,
            json!({ "command": "doc.open", "params": { "text": text } }),
        ))
        .expect("open");
        let session: Session = serde_json::from_value(open["session"].clone()).expect("session");
        let version = session.version;
        let err = run(params(
            Some(session),
            json!({ "command": "gesture.commit", "params": { "node": "r", "dx": 5 }, "version": version }),
        ))
        .expect_err("token bound");
        assert_eq!(err.code, "editor.rejected");
        assert_eq!(err.offers.first().map(|o| o.id.as_str()), Some("detach"));
        let stale = run(params(
            None,
            json!({ "command": "buffer.set", "params": { "text": "x" }, "version": 99 }),
        ))
        .expect_err("stale");
        assert_eq!(stale.code, "editor.stale_version");
    }

    /// The `(width, height)` of a PNG, from its IHDR chunk.
    fn png_size(png: &[u8]) -> (u32, u32) {
        let be = |at: usize| u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]);
        (be(16), be(20))
    }

    #[test]
    fn viewport_render_returns_the_window() {
        let open = run(params(
            None,
            json!({ "command": "doc.open", "params": { "text": FLOWCHART } }),
        ))
        .expect("open");
        let session: Session = serde_json::from_value(open["session"].clone()).expect("session");
        let render = |params_json: Value| {
            let out = run(params(
                Some(session.clone()),
                json!({ "command": "doc.render", "params": params_json }),
            ))
            .expect("render");
            let png = STANDARD
                .decode(out["png_base64"].as_str().expect("png"))
                .expect("base64");
            (out["result"].clone(), png)
        };
        let (page, page_png) = render(json!({ "scale": 2 }));
        let w = page["width"].as_u64().expect("width") as f64 / 2.0;
        let h = page["height"].as_u64().expect("height") as f64 / 2.0;
        let (whole, whole_png) =
            render(json!({ "scale": 2, "viewport": { "x": 0, "y": 0, "w": w, "h": h } }));
        assert_eq!(whole_png, page_png, "whole-page window equals the page");
        assert_eq!(whole["rect"]["x"], 0);
        let (window, window_png) = render(
            json!({ "scale": 9.5, "viewport": { "x": 120.4, "y": 30.2, "w": 60, "h": 40 } }),
        );
        assert_eq!(
            window["rect"],
            json!({ "x": 1143, "y": 286, "w": 571, "h": 381 }),
            "{window}"
        );
        assert_eq!(png_size(&window_png), (571, 381));
        let err = run(params(
            Some(session),
            json!({ "command": "doc.render",
                    "params": { "scale": 400, "viewport": { "x": 0, "y": 0, "w": 100, "h": 100 } } }),
        ))
        .expect_err("too large");
        assert_eq!(err.code, "render.region_too_large");
    }

    #[test]
    fn bad_font_key_is_invalid_params() {
        let mut p = params(None, json!({ "command": "commands.list" }));
        p.fonts
            .insert("Inter.ttf".to_owned(), STANDARD.encode(b"x"));
        let err = run(p).expect_err("unknown font");
        assert_eq!(err.code, "request.invalid_params");
    }
}
