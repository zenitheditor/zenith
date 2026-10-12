//! Row defects that block a merge row unless the policy says otherwise.

use zenith_core::{Diagnostic, DiagnosticPolicy, Severity};

/// Codes that mean the row would ship a visibly broken image.
const BLOCKING_CODES: [&str; 2] = ["font.glyph_missing", "text.overflow"];

/// Raise each blocking code to an Error unless `policy` names it.
///
/// A policy `allow` or `warn` for the code keeps the code non-blocking.
/// A policy `deny` already raised it. Every other diagnostic passes unchanged.
pub(super) fn block_row_defects(
    diagnostics: Vec<Diagnostic>,
    policy: &DiagnosticPolicy,
) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|mut d| {
            let blocking = BLOCKING_CODES.contains(&d.code.as_str())
                && policy.verb_for(&d.code, d.subject_id.as_deref()).is_none();
            if blocking && d.severity != Severity::Error {
                d.severity = Severity::Error;
            }
            d
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::{PolicyEntry, PolicyVerb};

    fn diag(code: &str, severity: Severity) -> Diagnostic {
        let mut d = Diagnostic::error(code, "m", None, None);
        d.severity = severity;
        d
    }

    #[test]
    fn unnamed_blocking_code_becomes_an_error() {
        let out = block_row_defects(
            vec![diag("font.glyph_missing", Severity::Warning)],
            &DiagnosticPolicy::default(),
        );
        assert_eq!(out[0].severity, Severity::Error);
    }

    #[test]
    fn other_codes_pass_unchanged() {
        let out = block_row_defects(
            vec![diag("contrast.low", Severity::Advisory)],
            &DiagnosticPolicy::default(),
        );
        assert_eq!(out[0].severity, Severity::Advisory);
    }

    #[test]
    fn a_policy_warn_keeps_the_code_non_blocking() {
        let mut policy = DiagnosticPolicy::default();
        policy.entries.push(PolicyEntry {
            verb: PolicyVerb::Warn,
            code: "text.overflow".to_owned(),
            subjects: Vec::new(),
            source_span: None,
        });
        let out = block_row_defects(vec![diag("text.overflow", Severity::Warning)], &policy);
        assert_eq!(out[0].severity, Severity::Warning);
    }
}
