//! [`EditorError`]: why a command did not run, and the follow-up commands
//! ([`Offer`]) the UI can send instead.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use zenith_core::Diagnostic;

use crate::wire::DiagnosticOut;

/// Why a command did not run. The session is unchanged.
///
/// `code` is stable (`editor.*`, or a pipeline code such as
/// `render.page_out_of_range`). `message` names what failed and the next
/// action. `diagnostics` holds the causes, for example the `tx.*` codes of a
/// rejected gesture. `offers` lists the commands the UI can send to go
/// ahead, for example the same gesture with `detach: true`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EditorError {
    /// Stable error code.
    pub code: String,
    /// What failed and what to do next.
    pub message: String,
    /// The diagnostics that caused the error, when any.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<DiagnosticOut>,
    /// Follow-up commands the UI can offer, in a stable order.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub offers: Vec<Offer>,
}

impl EditorError {
    /// An error with no diagnostics and no offers.
    #[must_use]
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
            diagnostics: Vec::new(),
            offers: Vec::new(),
        }
    }

    /// This error with `diagnostics`, located over `src`.
    #[must_use]
    pub fn with_diagnostics(mut self, diagnostics: &[Diagnostic], src: &str) -> Self {
        self.diagnostics = DiagnosticOut::all(diagnostics, src);
        self
    }

    /// This error with `offers`.
    #[must_use]
    pub fn with_offers(mut self, offers: Vec<Offer>) -> Self {
        self.offers = offers;
        self
    }

    /// `editor.invalid_params` for `command`, naming the serde error.
    #[must_use]
    pub fn invalid_params(command: &str, detail: impl std::fmt::Display) -> Self {
        Self::new(
            "editor.invalid_params",
            format!(
                "invalid params for '{command}': {detail}; send the params that \
                 commands.list documents for '{command}'"
            ),
        )
    }

    /// `editor.unknown_node` for `id`.
    #[must_use]
    pub fn unknown_node(id: &str) -> Self {
        Self::new(
            "editor.unknown_node",
            format!(
                "no node has id '{id}' in the current text; pick an id from doc.outline \
                 or select.hit"
            ),
        )
    }

    /// `editor.no_selection` for `command`.
    #[must_use]
    pub fn no_selection(command: &str) -> Self {
        Self::new(
            "editor.no_selection",
            format!("'{command}' needs a node: pass an id, or select one with select.set"),
        )
    }
}

impl std::fmt::Display for EditorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for EditorError {}

/// A follow-up command the UI can offer after an error: send `command` with
/// `params` to go ahead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Offer {
    /// Stable offer id: `detach`, `detach_anchor`, `set_size`, `reorder`,
    /// `absolute`, `replace_value`, `unlock`, or `show`.
    pub id: String,
    /// A short label for a button or menu item.
    pub label: String,
    /// The command to send.
    pub command: String,
    /// Its complete params.
    pub params: Value,
}
