//! Route one request to its method and encode the response string.

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::framing::{attached_fonts, split};
use crate::methods::{diagnose, editor, fonts, ping, render};
use crate::protocol::{Envelope, ErrorBody, err_json, ok_json};

/// The methods this module answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Method {
    Ping,
    Diagnose,
    Fonts,
    Render,
    Editor,
}

impl Method {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "ping" => Some(Self::Ping),
            "diagnose" => Some(Self::Diagnose),
            "fonts" => Some(Self::Fonts),
            "render" => Some(Self::Render),
            "editor" => Some(Self::Editor),
            _ => None,
        }
    }
}

/// Answer one request with one JSON response.
///
/// `input` is the JSON envelope, then, when the envelope lists `fonts`, one
/// NUL byte and the raw bytes of each listed font file in list order.
///
/// Never panics. Every error, including malformed input, is a
/// `{"ok": false, "error": {...}}` response.
#[must_use]
pub fn handle_request(input: &[u8]) -> String {
    encode(&respond(input))
}

/// A `{"ok": false}` response with `code` and `message`, for errors found
/// before a request exists (for example, a stdin read error).
#[must_use]
pub fn error_response(code: &str, message: &str) -> String {
    encode(&err_json(&ErrorBody::new(code, message)))
}

fn respond(input: &[u8]) -> Value {
    let (text, tail) = match split(input) {
        Ok(parts) => parts,
        Err(error) => return err_json(&error),
    };
    let envelope: Envelope = match serde_json::from_str(text) {
        Ok(env) => env,
        Err(e) => {
            return err_json(&ErrorBody::new(
                "request.invalid_json",
                format!(
                    "request is not a JSON envelope: {e}; send {{\"method\": \"ping\"|\"diagnose\"|\"fonts\"|\"render\"|\"editor\", \"params\": {{...}}}}"
                ),
            ));
        }
    };
    let Some(method) = Method::parse(&envelope.method) else {
        return err_json(&ErrorBody::new(
            "request.unknown_method",
            format!(
                "unknown method '{}'; use ping, diagnose, fonts, render, or editor",
                envelope.method
            ),
        ));
    };
    let font_files = match attached_fonts(&envelope.fonts, tail) {
        Ok(files) => files,
        Err(error) => return err_json(&error),
    };
    let outcome = match method {
        Method::Ping => Ok(ping::run()),
        Method::Diagnose => {
            params(envelope.params, "diagnose").and_then(|p| diagnose::run(&p, font_files))
        }
        Method::Fonts => params(envelope.params, "fonts").and_then(|p| fonts::run(&p, font_files)),
        Method::Render => {
            params(envelope.params, "render").and_then(|p| render::run(&p, font_files))
        }
        Method::Editor => {
            params(envelope.params, "editor").and_then(|p| editor::run(p, font_files))
        }
    };
    match outcome {
        Ok(result) => ok_json(result),
        Err(error) => err_json(&error),
    }
}

/// Decode the `params` of `method`.
fn params<T: DeserializeOwned>(raw: Value, method: &str) -> Result<T, ErrorBody> {
    serde_json::from_value(raw).map_err(|e| {
        ErrorBody::new(
            "request.invalid_params",
            format!(
                "invalid params for '{method}': {e}; pass {{\"source\": \"<.zen text>\"}} (editor: {{\"command\": {{...}}}}) plus optional project fields; font bytes follow the envelope, listed in its fonts"
            ),
        )
    })
}

fn encode(response: &Value) -> String {
    serde_json::to_string(response).unwrap_or_else(|e| {
        format!(
            "{{\"ok\":false,\"error\":{{\"code\":\"response.encode_failed\",\"message\":{}}}}}",
            Value::String(format!("could not encode the response: {e}"))
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(out: &str) -> Value {
        serde_json::from_str(out).expect("response is JSON")
    }

    #[test]
    fn malformed_json_is_an_error_response() {
        let out = parse(&handle_request(b"{not json"));
        assert_eq!(out["ok"], false);
        assert_eq!(out["error"]["code"], "request.invalid_json");
    }

    #[test]
    fn unknown_method_is_an_error_response() {
        let out = parse(&handle_request(br#"{"method":"explode"}"#));
        assert_eq!(out["ok"], false);
        assert_eq!(out["error"]["code"], "request.unknown_method");
    }

    #[test]
    fn missing_source_is_invalid_params() {
        let out = parse(&handle_request(br#"{"method":"render","params":{}}"#));
        assert_eq!(out["ok"], false);
        assert_eq!(out["error"]["code"], "request.invalid_params");
    }

    #[test]
    fn editor_routes() {
        let out = parse(&handle_request(
            br#"{"method":"editor","params":{"command":{"command":"commands.list"}}}"#,
        ));
        assert_eq!(out["ok"], true, "{out}");
        assert!(out["result"]["result"]["commands"].is_array());
        assert_eq!(out["result"]["session"]["version"], 1);
        let err = parse(&handle_request(
            br#"{"method":"editor","params":{"command":{"command":"history.undo","version":1}}}"#,
        ));
        assert_eq!(err["ok"], false);
        assert_eq!(err["error"]["code"], "editor.nothing_to_undo");
    }

    #[test]
    fn ping_routes() {
        let out = parse(&handle_request(br#"{"method":"ping","params":{}}"#));
        assert_eq!(out["ok"], true);
        assert_eq!(out["result"]["version"], env!("CARGO_PKG_VERSION"));
    }

    const SCRIPTS: &str = include_str!("../../examples/text-scripts.zen");
    const HEBREW: &str = "NotoSansHebrew-Regular.ttf";

    /// The bytes of fallback font file `file`.
    fn fallback_file(file: &str) -> Vec<u8> {
        let path = format!(
            "{}/../zenith-fonts-fallback/fonts/{file}",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read(path).expect("fallback font file")
    }

    /// A `fonts` request for `SCRIPTS` with `fonts` attached after the
    /// envelope.
    fn fonts_request(fonts: &[(&str, &[u8])]) -> Vec<u8> {
        let parts: Vec<Value> = fonts
            .iter()
            .map(|(file, bytes)| serde_json::json!({ "file": file, "size": bytes.len() }))
            .collect();
        let envelope = serde_json::json!({
            "method": "fonts",
            "params": { "source": SCRIPTS },
            "fonts": parts,
        });
        let mut out = envelope.to_string().into_bytes();
        out.push(0);
        for (_, bytes) in fonts {
            out.extend_from_slice(bytes);
        }
        out
    }

    /// The fallback files the `fonts` reply still names.
    fn needed_fallback(out: &Value) -> Vec<String> {
        out["result"]["faces"]
            .as_array()
            .expect("faces")
            .iter()
            .filter(|f| f["fallback"] == true)
            .filter_map(|f| f["file"].as_str().map(str::to_owned))
            .collect()
    }

    /// `true` when this build embeds the fallback chain, as when the
    /// workspace builds the CLI beside this crate.
    fn chain_embedded() -> bool {
        zenith_fonts_fallback::fallback_faces()
            .iter()
            .all(|f| f.is_embedded())
    }

    #[test]
    fn attached_font_bytes_register_the_face() {
        if chain_embedded() {
            return;
        }
        let hebrew = fallback_file(HEBREW);
        let without = parse(&handle_request(&fonts_request(&[])));
        assert!(
            needed_fallback(&without).iter().any(|f| f == HEBREW),
            "{without}"
        );
        let with = parse(&handle_request(&fonts_request(&[(HEBREW, &hebrew)])));
        assert_eq!(with["ok"], true, "{with}");
        assert!(
            !needed_fallback(&with).iter().any(|f| f == HEBREW),
            "{with}"
        );
    }

    #[test]
    fn each_request_reads_only_its_own_font_bytes() {
        if chain_embedded() {
            return;
        }
        // The module keeps no state: the same name with other bytes is a
        // different face, never the bytes of an earlier request.
        let hebrew = fallback_file(HEBREW);
        let good = parse(&handle_request(&fonts_request(&[(HEBREW, &hebrew)])));
        assert!(
            !needed_fallback(&good).iter().any(|f| f == HEBREW),
            "{good}"
        );
        let junk = parse(&handle_request(&fonts_request(&[(HEBREW, b"not a font")])));
        assert!(needed_fallback(&junk).iter().any(|f| f == HEBREW), "{junk}");
    }

    #[test]
    fn listed_sizes_that_miss_the_tail_are_rejected() {
        let mut input = fonts_request(&[(HEBREW, b"abcd")]);
        input.pop();
        let out = parse(&handle_request(&input));
        assert_eq!(out["error"]["code"], "request.invalid_fonts", "{out}");
    }

    #[test]
    fn fonts_inside_params_are_invalid_params() {
        let out = parse(&handle_request(
            br#"{"method":"fonts","params":{"source":"x","fonts":{"NotoSerif-Bold.ttf":"QQ=="}}}"#,
        ));
        assert_eq!(out["error"]["code"], "request.invalid_params", "{out}");
    }

    #[test]
    fn a_non_utf8_envelope_is_an_error_response() {
        let out = parse(&handle_request(b"{\xff}"));
        assert_eq!(out["error"]["code"], "request.invalid_utf8", "{out}");
    }

    #[test]
    fn error_response_is_an_error_envelope() {
        let out = parse(&error_response("x.code", "msg"));
        assert_eq!(out["ok"], false);
        assert_eq!(out["error"]["code"], "x.code");
    }
}
