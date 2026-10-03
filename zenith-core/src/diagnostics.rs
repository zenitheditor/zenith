//! Shared diagnostic types used across all Zenith validation passes.
//!
//! Diagnostics are collected without hard-failing — callers push them into a
//! `Vec<Diagnostic>` and continue resolving what they can. `lib.rs` re-exports
//! the most-used symbols at the crate root.

use std::collections::BTreeSet;

use serde::Serialize;

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
    /// Rare optional metadata, boxed to keep `Diagnostic` small.
    ///
    /// Read it through [`Diagnostic::cause`], [`Diagnostic::import`], and
    /// [`Diagnostic::fix`].
    extra: Option<Box<DiagnosticExtra>>,
}

/// Rare optional metadata of a [`Diagnostic`], stored behind one box.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DiagnosticExtra {
    /// Shared root cause, when many subjects fail for one reason.
    ///
    /// Reporters group diagnostics with the same `(code, severity, cause)`.
    pub cause: Option<String>,
    /// Id of the composition import whose document `span` indexes into.
    ///
    /// `None` means `span` indexes into the document under validation. A
    /// reporter must not map a span with `Some` onto the host source text.
    pub import: Option<String>,
    /// Structured machine fix, when the diagnostic has exactly one safe fix.
    ///
    /// `zenith fix` applies it.
    pub fix: Option<FixHint>,
}

/// A structured machine fix carried by a [`Diagnostic`].
///
/// Every variant names the property on the diagnostic subject that the fix
/// edits. Values are `.zen` source text without quotes (`#ffffff`,
/// `(px)24`, `700`, `cover`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FixHint {
    /// Swap an unknown token id for the one declared id it most likely meant.
    ReplaceTokenRef {
        property: String,
        from: String,
        to: String,
    },
    /// Rename an unknown property to its unique did-you-mean.
    RenameProperty { from: String, to: String },
    /// Swap an invalid enum value for its unique did-you-mean.
    ReplaceValue {
        property: String,
        from: String,
        to: String,
    },
    /// Replace a raw visual literal with a token reference.
    ///
    /// `exact_match` names the declared token with the same value (inside the
    /// property's role for dimensions). Without one, `zenith fix` mints a
    /// token. `nearest` names the closest declared token for a human pick.
    RawLiteral {
        property: String,
        literal: String,
        /// Token type to reference or mint (`color`, `dimension`,
        /// `fontWeight`, `fontFamily`).
        token_type: String,
        exact_match: Option<String>,
        nearest: Option<String>,
    },
    /// Set `property` to `to`: replace the subject's own entry, or add one
    /// when the subject does not set it (`y` → `(px)140`).
    SetProperty { property: String, to: String },
    /// Remove the subject's own `property` entry: the attribute has no
    /// effect where it is set (`x` on an in-flow child).
    RemoveProperty { property: String },
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
            extra: None,
        }
    }

    /// Shared root cause, when many subjects fail for one reason.
    pub fn cause(&self) -> Option<&str> {
        self.extra.as_deref().and_then(|e| e.cause.as_deref())
    }

    /// Id of the composition import whose document `span` indexes into.
    pub fn import(&self) -> Option<&str> {
        self.extra.as_deref().and_then(|e| e.import.as_deref())
    }

    /// Structured machine fix, when the diagnostic has exactly one safe fix.
    pub fn fix(&self) -> Option<&FixHint> {
        self.extra.as_deref().and_then(|e| e.fix.as_ref())
    }

    fn extra_mut(&mut self) -> &mut DiagnosticExtra {
        self.extra.get_or_insert_with(Default::default)
    }

    /// Return this diagnostic with `fix` set (`None` leaves it unset).
    #[must_use]
    pub fn with_fix(mut self, fix: Option<FixHint>) -> Self {
        if fix.is_some() {
            self.extra_mut().fix = fix;
        }
        self
    }

    /// Return this diagnostic with `import` set: its `span` indexes into the
    /// document of the composition import with that id.
    #[must_use]
    pub fn with_import(mut self, import: impl Into<String>) -> Self {
        self.set_import(import);
        self
    }

    /// Set `import` in place: `span` indexes into the document of the
    /// composition import with that id.
    pub fn set_import(&mut self, import: impl Into<String>) {
        self.extra_mut().import = Some(import.into());
    }

    /// True when this diagnostic has [`Severity::Error`].
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// True when any diagnostic in `diagnostics` has [`Severity::Error`].
    pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
        diagnostics.iter().any(Diagnostic::is_error)
    }

    /// Return this diagnostic with `cause` set.
    #[must_use]
    pub fn with_cause(mut self, cause: impl Into<String>) -> Self {
        self.extra_mut().cause = Some(cause.into());
        self
    }

    /// Remove repeated diagnostics, keeping the first occurrence of each.
    ///
    /// Two diagnostics repeat when code, severity, message, span, subject,
    /// cause, import, and fix all match. The fix counts: two spans of one text
    /// node can share message, span, and subject yet carry different literals,
    /// and each needs its own fix. Order of the kept diagnostics is unchanged.
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
    import: Option<&'a str>,
    fix: Option<&'a FixHint>,
}

impl<'a> DedupKey<'a> {
    fn of(d: &'a Diagnostic) -> Self {
        Self {
            code: &d.code,
            severity: d.severity,
            message: &d.message,
            span: d.span.map(|s| (s.start, s.end)),
            subject_id: d.subject_id.as_deref(),
            cause: d.cause(),
            import: d.import(),
            fix: d.fix(),
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
    fn dedup_keeps_same_span_from_different_imports() {
        let span = Some(Span { start: 4, end: 9 });
        let host = Diagnostic::warning("x.y", "m", span, None);
        let imported = Diagnostic::warning("x.y", "m", span, None).with_import("lib");
        assert_eq!(Diagnostic::dedup(vec![host, imported]).len(), 2);
    }

    #[test]
    fn dedup_keeps_distinct_fixes() {
        let hint = |literal: &str| FixHint::RawLiteral {
            property: "fill".into(),
            literal: literal.into(),
            token_type: "color".into(),
            exact_match: None,
            nearest: None,
        };
        let a = Diagnostic::error("x.y", "m", None, None).with_fix(Some(hint("#000000")));
        let b = Diagnostic::error("x.y", "m", None, None).with_fix(Some(hint("#ffffff")));
        assert_eq!(Diagnostic::dedup(vec![a.clone(), b, a]).len(), 2);
    }

    #[test]
    fn is_error_is_true_only_for_error_severity() {
        assert!(Diagnostic::error("x.y", "m", None, None).is_error());
        assert!(!Diagnostic::warning("x.y", "m", None, None).is_error());
        assert!(!Diagnostic::advisory("x.y", "m", None, None).is_error());
    }

    #[test]
    fn has_errors_checks_every_diagnostic() {
        let warn = Diagnostic::warning("x.y", "m", None, None);
        let err = Diagnostic::error("x.y", "m", None, None);
        assert!(!Diagnostic::has_errors(&[]));
        assert!(!Diagnostic::has_errors(std::slice::from_ref(&warn)));
        assert!(Diagnostic::has_errors(&[warn, err]));
    }

    #[test]
    fn with_cause_sets_cause() {
        let d = Diagnostic::advisory("x.y", "m", None, None).with_cause("root");
        assert_eq!(d.cause(), Some("root"));
    }

    #[test]
    fn diagnostic_stays_small() {
        // Large `Diagnostic` values trip clippy `result_large_err` on every
        // `Result<_, Diagnostic>`. Rare metadata lives behind one box.
        assert!(std::mem::size_of::<Diagnostic>() <= 112);
    }
}
