//! Resolve a diagnostic span to a file, line, and column.
//!
//! One definition serves the JSON `line`/`col`/`file` fields and the human
//! `line:col` text.

use std::collections::BTreeMap;

use zenith_core::{Diagnostic, ParseError, line_col};

use zenith_pipeline::imports::ImportFiles;

/// Where a diagnostic span points.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Location {
    /// Path of the imported file that holds the span. `None` for the host.
    pub file: Option<String>,
    /// 1-based `(line, column)`. `None` when the span or its text is unknown.
    pub line_col: Option<(usize, usize)>,
}

impl Location {
    /// Human text: `line:col` for the host, `file:line:col` for an import.
    /// `None` when there is no line and column.
    pub(crate) fn display(&self) -> Option<String> {
        let (line, col) = self.line_col?;
        Some(match &self.file {
            Some(file) => format!("{file}:{line}:{col}"),
            None => format!("{line}:{col}"),
        })
    }
}

/// Locates diagnostic spans over the host source and its import files.
///
/// Import file text is read once per import id.
#[derive(Debug)]
pub(crate) struct Locator<'a> {
    src: &'a str,
    files: Option<&'a ImportFiles>,
    texts: BTreeMap<String, Option<String>>,
}

impl<'a> Locator<'a> {
    /// A locator over the host `src` with no import files.
    pub(crate) fn new(src: &'a str) -> Self {
        Self {
            src,
            files: None,
            texts: BTreeMap::new(),
        }
    }

    /// A locator over the host `src` and the import `files`.
    pub(crate) fn with_files(src: &'a str, files: &'a ImportFiles) -> Self {
        Self {
            src,
            files: Some(files),
            texts: BTreeMap::new(),
        }
    }

    /// Locate the span of `d`.
    ///
    /// A span with no import indexes into the host source. A span from an
    /// import indexes into that import's file: `file` names it and `line_col`
    /// comes from its text. An unknown or unreadable file leaves `line_col`
    /// empty.
    pub(crate) fn locate(&mut self, d: &Diagnostic) -> Location {
        let Some(span) = d.span else {
            return Location::default();
        };
        let Some(import) = d.import() else {
            return Location {
                file: None,
                line_col: line_col(self.src, span.start),
            };
        };
        let Some(path) = self.files.and_then(|f| f.path(import)) else {
            return Location::default();
        };
        let text = self
            .texts
            .entry(import.to_owned())
            .or_insert_with(|| std::fs::read_to_string(path).ok());
        Location {
            file: Some(path.display().to_string()),
            line_col: text.as_deref().and_then(|text| line_col(text, span.start)),
        }
    }
}

/// Format a parse error as `error[parse.error] line:col: message`.
///
/// The location is omitted when the error has no span or the span is past the
/// end of `src`.
pub(crate) fn parse_error_line(src: &str, e: &ParseError) -> String {
    let at = e
        .span
        .and_then(|span| line_col(src, span.start))
        .map(|(line, col)| format!(" {line}:{col}"))
        .unwrap_or_default();
    format!("error[parse.error]{at}: {}", e.message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zenith_core::Span;

    #[test]
    fn host_span_gives_line_col() {
        let d = Diagnostic::error("x.bad", "m", Some(Span { start: 3, end: 4 }), None);
        let loc = Locator::new("ab\ncd").locate(&d);
        assert_eq!(loc.line_col, Some((2, 1)));
        assert_eq!(loc.display().as_deref(), Some("2:1"));
    }

    #[test]
    fn spanless_has_no_location() {
        let d = Diagnostic::error("x.bad", "m", None, None);
        assert_eq!(Locator::new("ab").locate(&d), Location::default());
        assert_eq!(Location::default().display(), None);
    }

    #[test]
    fn import_span_with_unknown_file_has_no_location() {
        let d = Diagnostic::error("x.bad", "m", Some(Span { start: 0, end: 1 }), None)
            .with_import("brand");
        assert_eq!(Locator::new("ab").locate(&d), Location::default());
    }

    #[test]
    fn parse_error_line_with_and_without_span() {
        let with = ParseError::with_span(
            zenith_core::ParseErrorCode::InvalidPropertyValue,
            Span { start: 3, end: 4 },
            "bad",
        );
        assert_eq!(
            parse_error_line("ab\ncd", &with),
            "error[parse.error] 2:1: bad"
        );
        let without =
            ParseError::spanless(zenith_core::ParseErrorCode::InvalidPropertyValue, "bad");
        assert_eq!(
            parse_error_line("ab\ncd", &without),
            "error[parse.error]: bad"
        );
    }
}
