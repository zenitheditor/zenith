//! Wire types of the JSON protocol: the request envelope, the response
//! envelope, the error body, and the diagnostic shape.

use serde::{Deserialize, Serialize};
use serde_json::Value;
pub(crate) use zenith_editor::DiagnosticOut;
use zenith_editor::{EditorError, Offer};

/// One request: a method name and its parameters.
#[derive(Debug, Deserialize)]
pub(crate) struct Envelope {
    /// `ping`, `diagnose`, `fonts`, or `render`.
    pub(crate) method: String,
    /// Method parameters. A missing `params` reads as `null`.
    #[serde(default)]
    pub(crate) params: Value,
}

/// A method error: a stable code, a message that names the next action, the
/// diagnostics that caused it, and the follow-up commands an `editor`
/// rejection offers, when any.
#[derive(Debug, Serialize, PartialEq)]
pub(crate) struct ErrorBody {
    pub(crate) code: String,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) diagnostics: Vec<DiagnosticOut>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) offers: Vec<Offer>,
}

impl ErrorBody {
    /// An error with no diagnostics.
    pub(crate) fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
            diagnostics: Vec::new(),
            offers: Vec::new(),
        }
    }
}

impl From<EditorError> for ErrorBody {
    fn from(e: EditorError) -> Self {
        Self {
            code: e.code,
            message: e.message,
            diagnostics: e.diagnostics,
            offers: e.offers,
        }
    }
}

/// The success envelope around `result`.
pub(crate) fn ok_json(result: Value) -> Value {
    serde_json::json!({ "ok": true, "result": result })
}

/// The failure envelope around `error`.
pub(crate) fn err_json(error: &ErrorBody) -> Value {
    match serde_json::to_value(error) {
        Ok(body) => serde_json::json!({ "ok": false, "error": body }),
        Err(e) => serde_json::json!({
            "ok": false,
            "error": {
                "code": "response.encode_failed",
                "message": format!("could not encode the error body: {e}"),
            },
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{Diagnostic, Span};

    #[test]
    fn diagnostic_locates_its_span() {
        let src = "a\nbc\n";
        let d = Diagnostic::error("x.bad", "boom", Some(Span { start: 3, end: 4 }), None);
        let out = DiagnosticOut::from_diagnostic(&d, src);
        assert_eq!(out.line, Some(2));
        assert_eq!(out.col, Some(2));
        assert_eq!(out.severity, "error");
    }

    #[test]
    fn diagnostic_without_span_has_no_location() {
        let d = Diagnostic::advisory("x.note", "fyi", None, Some("n".into()));
        let out = DiagnosticOut::from_diagnostic(&d, "");
        assert_eq!(out.line, None);
        assert_eq!(out.subject_id.as_deref(), Some("n"));
        assert_eq!(out.severity, "advisory");
    }

    #[test]
    fn envelope_without_params_reads_null() {
        let env: Envelope = serde_json::from_str(r#"{"method":"ping"}"#).expect("envelope");
        assert_eq!(env.method, "ping");
        assert!(env.params.is_null());
    }
}
