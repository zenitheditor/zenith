//! Route one request string to its method and encode the response string.

use serde::de::DeserializeOwned;
use serde_json::Value;

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

/// Answer one JSON request with one JSON response.
///
/// Never panics. Every error, including malformed input, is a
/// `{"ok": false, "error": {...}}` response.
#[must_use]
pub fn handle_request(input: &str) -> String {
    encode(&respond(input))
}

/// A `{"ok": false}` response with `code` and `message`, for errors found
/// before a request exists (for example, a stdin read error).
#[must_use]
pub fn error_response(code: &str, message: &str) -> String {
    encode(&err_json(&ErrorBody::new(code, message)))
}

fn respond(input: &str) -> Value {
    let envelope: Envelope = match serde_json::from_str(input) {
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
    let outcome = match method {
        Method::Ping => Ok(ping::run()),
        Method::Diagnose => params(envelope.params, "diagnose").and_then(|p| diagnose::run(&p)),
        Method::Fonts => params(envelope.params, "fonts").and_then(|p| fonts::run(&p)),
        Method::Render => params(envelope.params, "render").and_then(|p| render::run(&p)),
        Method::Editor => params(envelope.params, "editor").and_then(editor::run),
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
                "invalid params for '{method}': {e}; pass {{\"source\": \"<.zen text>\"}} (editor: {{\"command\": {{...}}}}) plus optional project fields"
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
        let out = parse(&handle_request("{not json"));
        assert_eq!(out["ok"], false);
        assert_eq!(out["error"]["code"], "request.invalid_json");
    }

    #[test]
    fn unknown_method_is_an_error_response() {
        let out = parse(&handle_request(r#"{"method":"explode"}"#));
        assert_eq!(out["ok"], false);
        assert_eq!(out["error"]["code"], "request.unknown_method");
    }

    #[test]
    fn missing_source_is_invalid_params() {
        let out = parse(&handle_request(r#"{"method":"render","params":{}}"#));
        assert_eq!(out["ok"], false);
        assert_eq!(out["error"]["code"], "request.invalid_params");
    }

    #[test]
    fn editor_routes() {
        let out = parse(&handle_request(
            r#"{"method":"editor","params":{"command":{"command":"commands.list"}}}"#,
        ));
        assert_eq!(out["ok"], true, "{out}");
        assert!(out["result"]["result"]["commands"].is_array());
        assert_eq!(out["result"]["session"]["version"], 1);
        let err = parse(&handle_request(
            r#"{"method":"editor","params":{"command":{"command":"history.undo","version":1}}}"#,
        ));
        assert_eq!(err["ok"], false);
        assert_eq!(err["error"]["code"], "editor.nothing_to_undo");
    }

    #[test]
    fn ping_routes() {
        let out = parse(&handle_request(r#"{"method":"ping","params":{}}"#));
        assert_eq!(out["ok"], true);
        assert_eq!(out["result"]["version"], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn error_response_is_an_error_envelope() {
        let out = parse(&error_response("x.code", "msg"));
        assert_eq!(out["ok"], false);
        assert_eq!(out["error"]["code"], "x.code");
    }
}
