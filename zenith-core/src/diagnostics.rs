//! Shared diagnostic types used across all Zenith validation passes.
//!
//! Diagnostics are collected without hard-failing — callers push them into a
//! `Vec<Diagnostic>` and continue resolving what they can. `lib.rs` re-exports
//! the most-used symbols at the crate root.

use std::collections::BTreeSet;

use crate::ast::Span;

/// The severity level of a [`Diagnostic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// A definite problem that prevents correct output.
    Error,
    /// A potential problem or version-relative issue; output may still be
    /// produced.
    Warning,
    /// Informational note; does not block output.
    Advisory,
}

/// A single structured diagnostic produced during validation or resolution.
///
/// Diagnostics carry a stable `code` that agents and tooling can key on, a
/// human-readable `message`, an optional source `span`, and an optional
/// `subject_id` naming the token (or future: node/style) the diagnostic is
/// about.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    /// Stable dot-separated code, e.g. `"token.cyclic_reference"`.
    pub code: String,
    /// Severity classification.
    pub severity: Severity,
    /// Human-readable description.
    pub message: String,
    /// Source location, when available.
    pub span: Option<Span>,
    /// The token ID (or future: node/style ID) the diagnostic concerns.
    pub subject_id: Option<String>,
    /// Shared root cause, when many subjects fail for one reason.
    ///
    /// Reporters group diagnostics with the same `(code, severity, cause)`.
    pub cause: Option<Box<str>>,
}

impl Diagnostic {
    /// Construct a new diagnostic with all fields explicit.
    pub fn new(
        code: impl Into<String>,
        severity: Severity,
        message: impl Into<String>,
        span: Option<Span>,
        subject_id: Option<String>,
    ) -> Self {
        Self {
            code: code.into(),
            severity,
            message: message.into(),
            span,
            subject_id,
            cause: None,
        }
    }

    /// Return this diagnostic with `cause` set.
    #[must_use]
    pub fn with_cause(mut self, cause: impl Into<String>) -> Self {
        self.cause = Some(cause.into().into_boxed_str());
        self
    }

    /// Remove repeated diagnostics, keeping the first occurrence of each.
    ///
    /// Two diagnostics repeat when code, severity, message, span, subject,
    /// and cause all match. Order of the kept diagnostics is unchanged.
    #[must_use]
    pub fn dedup(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
        let keep: Vec<bool> = {
            let mut seen: BTreeSet<DedupKey<'_>> = BTreeSet::new();
            diagnostics
                .iter()
                .map(|d| seen.insert(DedupKey::of(d)))
                .collect()
        };
        diagnostics
            .into_iter()
            .zip(keep)
            .filter_map(|(d, k)| k.then_some(d))
            .collect()
    }

    /// Shorthand for an [`Severity::Error`]-level diagnostic.
    pub fn error(
        code: impl Into<String>,
        message: impl Into<String>,
        span: Option<Span>,
        subject_id: Option<String>,
    ) -> Self {
        Self::new(code, Severity::Error, message, span, subject_id)
    }

    /// Shorthand for a [`Severity::Warning`]-level diagnostic.
    pub fn warning(
        code: impl Into<String>,
        message: impl Into<String>,
        span: Option<Span>,
        subject_id: Option<String>,
    ) -> Self {
        Self::new(code, Severity::Warning, message, span, subject_id)
    }

    /// Shorthand for an [`Severity::Advisory`]-level diagnostic.
    pub fn advisory(
        code: impl Into<String>,
        message: impl Into<String>,
        span: Option<Span>,
        subject_id: Option<String>,
    ) -> Self {
        Self::new(code, Severity::Advisory, message, span, subject_id)
    }
}

/// Ordered identity of a [`Diagnostic`] for [`Diagnostic::dedup`].
#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct DedupKey<'a> {
    code: &'a str,
    severity: Severity,
    message: &'a str,
    span: Option<(usize, usize)>,
    subject_id: Option<&'a str>,
    cause: Option<&'a str>,
}

impl<'a> DedupKey<'a> {
    fn of(d: &'a Diagnostic) -> Self {
        Self {
            code: &d.code,
            severity: d.severity,
            message: &d.message,
            span: d.span.map(|s| (s.start, s.end)),
            subject_id: d.subject_id.as_deref(),
            cause: d.cause.as_deref(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_keeps_first_occurrence_in_order() {
        let a = Diagnostic::advisory("font.unresolved", "a", None, Some("n1".into()));
        let b = Diagnostic::warning("text.overflow", "b", None, Some("n2".into()));
        let out = Diagnostic::dedup(vec![a.clone(), b.clone(), a.clone(), b.clone()]);
        assert_eq!(out, vec![a, b]);
    }

    #[test]
    fn dedup_keeps_distinct_spans_and_subjects() {
        let a = Diagnostic::advisory("x.y", "m", Some(Span { start: 0, end: 1 }), None);
        let b = Diagnostic::advisory("x.y", "m", Some(Span { start: 2, end: 3 }), None);
        let c = Diagnostic::advisory("x.y", "m", None, Some("s".into()));
        assert_eq!(Diagnostic::dedup(vec![a, b, c]).len(), 3);
    }

    #[test]
    fn with_cause_sets_cause() {
        let d = Diagnostic::advisory("x.y", "m", None, None).with_cause("root");
        assert_eq!(d.cause.as_deref(), Some("root"));
    }
}
