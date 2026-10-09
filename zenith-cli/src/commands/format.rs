//! Shared serialisation and diagnostic-line formatting helpers.

/// Serialise `value` to pretty-printed JSON, falling back to the error
/// message string if serialisation itself fails (which cannot happen for
/// these well-typed DTOs, but is kept as a safe fallback).
pub(crate) fn serialize_pretty<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|e| e.to_string())
}

/// Serialise `value` to compact (whitespace-free) JSON.
///
/// This is the token-minimal form used by the MCP server for the text mirror of
/// a structured result — `serialize_pretty` is for human terminals, this is for
/// machine consumers where every whitespace byte is a wasted token.
pub(crate) fn serialize_compact<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|e| e.to_string())
}

/// Format a single diagnostic as a human-readable line:
/// `severity[code] (subject_id): message` (the subject is omitted when absent).
pub(crate) fn format_diagnostic_line(d: &zenith_core::Diagnostic) -> String {
    format_located_diagnostic_line(d, None)
}

/// Format a single diagnostic as
/// `severity[code] (subject_id) location: message`.
///
/// The subject and location are each omitted when absent. `location` is the
/// text of [`crate::report::Location::display`].
pub(crate) fn format_located_diagnostic_line(
    d: &zenith_core::Diagnostic,
    location: Option<&str>,
) -> String {
    let sev = crate::json_types::severity_str(&d.severity);
    let subject = d
        .subject_id
        .as_deref()
        .map(|s| format!(" ({})", s))
        .unwrap_or_default();
    let at = location.map(|l| format!(" {l}")).unwrap_or_default();
    format!("{}[{}]{}{}: {}", sev, d.code, subject, at, d.message)
}
