//! JSON-RPC 2.0 envelope helpers and the `tools/call` result shape.
//!
//! These build the wire-level success/error objects so the dispatcher in
//! `mod.rs` stays a thin routing table. Nothing here knows what any tool does.

use serde_json::{Value, json};

/// A JSON-RPC `result` response for request `id`.
pub fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// A JSON-RPC `error` response for request `id`.
pub fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// The structured outcome of a tool invocation.
///
/// Every successful tool returns a compact structured object; the text mirror
/// (`text`) is the same object serialised compactly, so clients that only read
/// `content[].text` still receive the full machine result. Setup errors carry a
/// human message without a structured payload. Partial batch errors retain their
/// structured report and its JSON text mirror with `is_error = true`.
pub struct ToolResult {
    /// The `structuredContent` payload (omitted from the wire when `None`).
    pub structured: Option<Value>,
    /// The `content[0].text` mirror (compact JSON for success, message for error).
    pub text: String,
    /// Whether this result represents a tool-execution error.
    pub is_error: bool,
    /// Base64 PNGs sent as MCP image content blocks after the text.
    pub images: Vec<String>,
}

impl ToolResult {
    /// A successful structured result. `text` is the compact JSON mirror.
    pub fn ok(structured: Value, text: String) -> Self {
        Self {
            structured: Some(structured),
            text,
            is_error: false,
            images: Vec::new(),
        }
    }

    /// A structured result that is an error when `is_error` (for example a
    /// batch with failed rows, or an editor command the engine refused).
    pub fn report(structured: Value, text: String, is_error: bool) -> Self {
        Self {
            structured: Some(structured),
            text,
            is_error,
            images: Vec::new(),
        }
    }

    /// This result with `png` as an image content block.
    pub fn with_png(mut self, png: &[u8]) -> Self {
        self.images.push(super::base64::encode(png));
        self
    }

    /// A tool-execution error carrying a human-readable message.
    pub fn err(message: impl Into<String>) -> Self {
        Self {
            structured: None,
            text: message.into(),
            is_error: true,
            images: Vec::new(),
        }
    }

    /// Render this result as the `tools/call` result object.
    pub fn into_payload(self) -> Value {
        let mut obj = serde_json::Map::new();
        let mut content = vec![json!({ "type": "text", "text": self.text })];
        content.extend(
            self.images
                .iter()
                .map(|data| json!({ "type": "image", "data": data, "mimeType": "image/png" })),
        );
        obj.insert("content".into(), Value::Array(content));
        if let Some(structured) = self.structured {
            obj.insert("structuredContent".into(), structured);
        }
        obj.insert("isError".into(), Value::Bool(self.is_error));
        Value::Object(obj)
    }
}
