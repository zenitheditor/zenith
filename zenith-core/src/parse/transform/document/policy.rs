//! The document-level `diagnostics { … }` lint-policy block.

use kdl::{KdlNode, KdlValue};

use crate::ast::policy::{DiagnosticPolicy, PolicyEntry, PolicyVerb};
use crate::error::{ParseError, ParseErrorCode};
use crate::parse::transform::helpers::node_span;

/// Transform the document-level `diagnostics { … }` block into a
/// [`DiagnosticPolicy`].
///
/// Each child node is one policy entry whose NAME is the verb (`allow`, `deny`,
/// or `warn`) and whose FIRST positional argument is the diagnostic code string:
///
/// ```text
/// diagnostics {
///   allow "layout.off_canvas"
///   allow "layout.off_canvas" "bg.glow" "bg.rim"
///   deny  "token.unused"
///   warn  "node.unknown_property"
/// }
/// ```
///
/// A child whose name is not a recognized verb is silently ignored
/// (forward-compat — same posture as every other document block). A recognized
/// verb whose code argument is missing or non-string is a hard [`ParseError`]
/// (the entry is meaningless without a code). Declaration order is preserved;
/// last-wins resolution happens at consult time (see [`DiagnosticPolicy::verb_for`]).
pub(crate) fn transform_diagnostic_policy(node: &KdlNode) -> Result<DiagnosticPolicy, ParseError> {
    let mut entries: Vec<PolicyEntry> = Vec::new();
    if let Some(children) = node.children() {
        for child in children.nodes() {
            let (verb, verb_name) = match child.name().value() {
                "allow" => (PolicyVerb::Allow, "allow"),
                "deny" => (PolicyVerb::Deny, "deny"),
                "warn" => (PolicyVerb::Warn, "warn"),
                // Unknown verb → ignore (forward-compat).
                _ => continue,
            };
            let mut positional = child
                .entries()
                .iter()
                .filter(|entry| entry.name().is_none());
            let code = match positional.next().map(|entry| entry.value()) {
                Some(KdlValue::String(s)) => s.clone(),
                _ => {
                    return Err(ParseError::spanless(
                        ParseErrorCode::InvalidPropertyValue,
                        format!(
                            "diagnostics `{verb_name}` entry requires a quoted diagnostic-code \
                             string as its first argument, e.g. `{verb_name} \"layout.off_canvas\"`"
                        ),
                    ));
                }
            };
            let mut subjects: Vec<String> = Vec::new();
            for (idx, entry) in positional.enumerate() {
                match entry.value() {
                    KdlValue::String(s) => subjects.push(s.clone()),
                    _ => {
                        let subject_index = idx + 1;
                        return Err(ParseError::spanless(
                            ParseErrorCode::InvalidPropertyValue,
                            format!(
                                "diagnostics `{verb_name}` subject argument {subject_index} must \
                                 be a quoted subject-id string, e.g. `{verb_name} \
                                 \"layout.off_canvas\" \"bg.glow\"`"
                            ),
                        ));
                    }
                }
            }
            if child.entries().iter().any(|entry| {
                entry
                    .name()
                    .map(|name| name.value() == "subject" || name.value() == "subjects")
                    .unwrap_or(false)
            }) {
                return Err(ParseError::spanless(
                    ParseErrorCode::InvalidPropertyValue,
                    "diagnostics scoped subjects must be positional strings after the \
                     diagnostic code, e.g. `allow \"layout.off_canvas\" \"bg.glow\"`",
                ));
            }
            entries.push(PolicyEntry {
                verb,
                code,
                subjects,
                source_span: node_span(child),
            });
        }
    }
    Ok(DiagnosticPolicy { entries })
}
