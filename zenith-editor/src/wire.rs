//! Wire shapes shared by every command result: diagnostics and text
//! deltas as the page reads them.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use zenith_core::{Diagnostic, LineIndex, Severity};

use crate::error::EditorError;
use crate::session::TextDelta;

/// `value` as JSON.
///
/// # Errors
///
/// `editor.encode_failed` when `value` does not encode.
pub(crate) fn to_json<T: Serialize>(value: &T) -> Result<Value, EditorError> {
    serde_json::to_value(value).map_err(|e| {
        EditorError::new(
            "editor.encode_failed",
            format!("could not encode the result: {e}; report this as an engine bug"),
        )
    })
}

/// One diagnostic as the page reads it. `start` / `end` are byte offsets
/// and `line` / `col` 1-based positions in the text the diagnostic came
/// from; all four are absent when the span is in an imported file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticOut {
    /// Diagnostic code, for example `tx.token_bound`.
    pub code: String,
    /// `error`, `warning`, or `advisory`.
    pub severity: String,
    /// What is wrong and how to fix it.
    pub message: String,
    /// The node the diagnostic is about.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<String>,
    /// The composition import whose file holds the span, when not the
    /// document itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import: Option<String>,
    /// Span start, in bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<usize>,
    /// Span end, in bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<usize>,
    /// 1-based line of `start`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// 1-based column of `start`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub col: Option<usize>,
}

impl DiagnosticOut {
    /// Convert `d`, locating its span over `src`. A span from an imported
    /// document keeps no offsets, since `src` is not its text.
    #[must_use]
    pub fn from_diagnostic(d: &Diagnostic, src: &str) -> Self {
        Self::located(d, &LineIndex::new(src))
    }

    /// [`from_diagnostic`](Self::from_diagnostic) over an indexed source.
    fn located(d: &Diagnostic, lines: &LineIndex<'_>) -> Self {
        let span = if d.import().is_some() { None } else { d.span };
        let located = span.and_then(|s| lines.line_col(s.start));
        Self {
            code: d.code.clone(),
            severity: severity_name(d.severity).to_owned(),
            message: d.message.clone(),
            subject_id: d.subject_id.clone(),
            import: d.import().map(str::to_owned),
            start: span.map(|s| s.start),
            end: span.map(|s| s.end),
            line: located.map(|(line, _)| line),
            col: located.map(|(_, col)| col),
        }
    }

    /// Convert every diagnostic of `list` over `src`, in order.
    #[must_use]
    pub fn all(list: &[Diagnostic], src: &str) -> Vec<Self> {
        let lines = LineIndex::new(src);
        list.iter().map(|d| Self::located(d, &lines)).collect()
    }

    /// A spanless advisory.
    #[must_use]
    pub fn advisory(code: &str, message: String) -> Self {
        Self {
            code: code.to_owned(),
            severity: "advisory".to_owned(),
            message,
            subject_id: None,
            import: None,
            start: None,
            end: None,
            line: None,
            col: None,
        }
    }
}

/// The wire name of `severity`.
#[must_use]
pub fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Advisory => "advisory",
    }
}

/// A text change for the page's editor to apply as one transaction.
///
/// `start` / `end` are byte offsets into the text before the change, for
/// native callers. `from` / `to` are the same range in UTF-16 code units,
/// the positions a JavaScript string and CodeMirror use. Replace that range
/// with `insert`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeltaOut {
    /// Range start, in bytes.
    pub start: usize,
    /// Range end, in bytes.
    pub end: usize,
    /// Range start, in UTF-16 code units.
    pub from: usize,
    /// Range end, in UTF-16 code units.
    pub to: usize,
    /// The replacement text.
    pub insert: String,
}

impl DeltaOut {
    /// The wire form of `delta`, which applies to `before`.
    #[must_use]
    pub fn new(delta: &TextDelta, before: &str) -> Self {
        Self {
            start: delta.start,
            end: delta.end,
            from: utf16_offset(before, delta.start),
            to: utf16_offset(before, delta.end),
            insert: delta.insert.clone(),
        }
    }
}

/// The UTF-16 offset of byte offset `byte` in `text`. A byte offset past
/// the end or inside a character counts the characters that end before it.
fn utf16_offset(text: &str, byte: usize) -> usize {
    text.char_indices()
        .take_while(|(i, c)| i + c.len_utf8() <= byte)
        .map(|(_, c)| c.len_utf16())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::Span;

    #[test]
    fn diagnostic_locates_its_span() {
        let d = Diagnostic::error("x.bad", "boom", Some(Span { start: 3, end: 4 }), None);
        let out = DiagnosticOut::from_diagnostic(&d, "a\nbc\n");
        assert_eq!((out.line, out.col), (Some(2), Some(2)));
        assert_eq!(out.severity, "error");
    }

    #[test]
    fn utf16_offsets_count_code_units() {
        let text = "a\u{1F600}b";
        let delta = TextDelta {
            start: 5,
            end: 6,
            insert: "c".to_owned(),
        };
        let out = DeltaOut::new(&delta, text);
        assert_eq!((out.from, out.to), (3, 4));
        assert_eq!(utf16_offset("héllo", 3), 2);
    }
}
