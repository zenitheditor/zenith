//! [`PipelineError`]: why a pipeline call stopped, with its diagnostics.

use zenith_core::Diagnostic;

use crate::imports::ImportFiles;

/// One error line: `error[<code>]: <message>`.
#[must_use]
pub fn format_error_diag(d: &Diagnostic) -> String {
    format!("error[{}]: {}", d.code, d.message)
}

/// Why a pipeline call stopped.
///
/// `diagnostics` holds every diagnostic known when the call stopped. It
/// always has at least one [`Severity::Error`](zenith_core::Severity::Error)
/// entry. `message` joins the error lines.
#[derive(Debug)]
pub struct PipelineError {
    /// Human-readable message: the error diagnostics, one per line.
    pub message: String,
    /// Recommended exit code: `1` for validation errors, `2` otherwise.
    pub exit_code: u8,
    /// Every diagnostic known at the failure point, in report order.
    pub diagnostics: Vec<Diagnostic>,
    /// Files of the composition imports, for locating diagnostic spans.
    pub import_files: ImportFiles,
}

impl PipelineError {
    /// One error diagnostic with `code` and `msg`.
    #[must_use]
    pub fn new(code: &str, msg: impl Into<String>, exit_code: u8) -> Self {
        Self::blocked(vec![Diagnostic::error(code, msg, None, None)], exit_code)
    }

    /// A call stopped by `diagnostics`. Repeats are removed. The message
    /// joins the error lines.
    #[must_use]
    pub fn blocked(diagnostics: Vec<Diagnostic>, exit_code: u8) -> Self {
        let diagnostics = Diagnostic::dedup(diagnostics);
        let message = diagnostics
            .iter()
            .filter(|d| d.is_error())
            .map(format_error_diag)
            .collect::<Vec<_>>()
            .join("\n");
        Self {
            message,
            exit_code,
            diagnostics,
            import_files: ImportFiles::default(),
        }
    }

    /// This error with the composition import files its diagnostic spans can
    /// index into.
    #[must_use]
    pub fn with_import_files(mut self, import_files: ImportFiles) -> Self {
        self.import_files = import_files;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_joins_error_lines_only() {
        let err = PipelineError::blocked(
            vec![
                Diagnostic::error("a.one", "first", None, None),
                Diagnostic::advisory("a.note", "fyi", None, None),
                Diagnostic::error("a.two", "second", None, None),
            ],
            1,
        );
        assert_eq!(err.message, "error[a.one]: first\nerror[a.two]: second");
        assert_eq!(err.diagnostics.len(), 3);
        assert_eq!(err.exit_code, 1);
    }
}
