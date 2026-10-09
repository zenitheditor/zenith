//! [`Event`]: one live notification for every client of a document.

use serde_json::Value;

/// A named event with a JSON payload. The HTTP server sends it as a
/// Server-Sent Event (`event: <name>`, `data: <json>`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Event {
    /// `session`, `external_change`, `saved`, `state`, or `shutdown`.
    pub(crate) name: &'static str,
    /// The payload.
    pub(crate) data: Value,
}

impl Event {
    /// An event `name` with payload `data`.
    pub(crate) fn new(name: &'static str, data: Value) -> Self {
        Self { name, data }
    }
}
