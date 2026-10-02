//! [`CliError`]: one failure, printed as JSON on stdout or text on stderr.

use std::process::ExitCode;

use crate::commands::serialize_pretty;
use crate::json_types::{DiagnosticJson, ErrorOutput};

/// Schema id of the failure envelope.
pub(crate) const ERROR_SCHEMA: &str = "zenith-error-v1";

/// A failure outside a command's own output shape.
///
/// `human` is the exact stderr text. Each line of the form
/// `error[<code>]: <message>` becomes one JSON diagnostic with that code. Any
/// other line uses `code`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CliError {
    /// Fallback diagnostic code, `<namespace>.<snake_event>`.
    pub(crate) code: String,
    /// Human text printed to stderr.
    pub(crate) human: String,
    /// Process exit code.
    pub(crate) exit_code: u8,
}

impl CliError {
    /// Build a failure with fallback `code`, stderr text `human`, and exit code.
    pub(crate) fn new(code: impl Into<String>, human: impl Into<String>, exit_code: u8) -> Self {
        Self {
            code: code.into(),
            human: human.into(),
            exit_code,
        }
    }

    /// Bad command-line usage (exit code 2).
    pub(crate) fn usage(human: impl Into<String>) -> Self {
        Self::new("cli.invalid_argument", human, 2)
    }

    /// One JSON diagnostic per non-empty line of `human`.
    pub(crate) fn diagnostics(&self) -> Vec<DiagnosticJson> {
        let mut out: Vec<DiagnosticJson> = self
            .human
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| {
                let (code, message) = split_code(line);
                DiagnosticJson::error(code.unwrap_or(&self.code), message)
            })
            .collect();
        if out.is_empty() {
            out.push(DiagnosticJson::error(self.code.clone(), "command failed"));
        }
        out
    }

    /// The `zenith-error-v1` envelope as pretty JSON.
    pub(crate) fn to_json(&self) -> String {
        serialize_pretty(&ErrorOutput {
            schema: ERROR_SCHEMA,
            diagnostics: self.diagnostics(),
        })
    }

    /// Print the failure and return its exit code.
    ///
    /// `json` prints the envelope to stdout. Otherwise the human text goes to
    /// stderr.
    pub(crate) fn emit(&self, json: bool) -> ExitCode {
        if json {
            println!("{}", self.to_json());
        } else {
            eprintln!("{}", self.human);
        }
        ExitCode::from(self.exit_code)
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.human)
    }
}

/// Split `error[<code>]: <message>` or `error: <message>` into its parts.
fn split_code(line: &str) -> (Option<&str>, &str) {
    if let Some(rest) = line.strip_prefix("error[")
        && let Some((code, message)) = rest.split_once("]: ")
        && !code.is_empty()
        && !code.contains(char::is_whitespace)
    {
        return (Some(code), message);
    }
    match line.strip_prefix("error: ") {
        Some(message) => (None, message),
        None => (None, line),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coded_line_keeps_its_code() {
        let e = CliError::new("tx.failed", "error[tx.parse]: unknown op 'Foo'", 2);
        let d = e.diagnostics();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code, "tx.parse");
        assert_eq!(d[0].message, "unknown op 'Foo'");
        assert_eq!(d[0].severity, "error");
    }

    #[test]
    fn plain_line_uses_fallback_code() {
        let e = CliError::new("merge.setup_failed", "error: no nodes", 2);
        let d = e.diagnostics();
        assert_eq!(d[0].code, "merge.setup_failed");
        assert_eq!(d[0].message, "no nodes");
    }

    #[test]
    fn each_line_is_one_diagnostic() {
        let e = CliError::new("x.y", "error[a.b]: one\n\nerror[c.d]: two", 1);
        let codes: Vec<String> = e.diagnostics().into_iter().map(|d| d.code).collect();
        assert_eq!(codes, vec!["a.b", "c.d"]);
    }

    #[test]
    fn json_envelope_has_schema() {
        let e = CliError::usage("error: bad flag");
        let v: serde_json::Value = serde_json::from_str(&e.to_json()).expect("valid JSON");
        assert_eq!(v["schema"], ERROR_SCHEMA);
        assert_eq!(v["diagnostics"][0]["code"], "cli.invalid_argument");
    }
}
