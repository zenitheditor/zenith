//! `zenith validate`: the pipeline's validate run on the native host, then
//! JSON or human formatting.
//!
//! The public entry point [`run`] operates on in-memory source text; the
//! native host reads config, imports, and assets.

use std::path::Path;

use zenith_core::Diagnostic;
use zenith_pipeline::imports::ImportFiles;
use zenith_pipeline::{PolicyFlags, Validation, validate_source};

use crate::commands::serialize_pretty;
use crate::json_types::{DiagnosticJson, ValidateOutput};
use crate::native;
use crate::report::{Locator, human_diagnostic_lines};

// ── Result type ───────────────────────────────────────────────────────────────

/// The outcome of a validate run.
#[derive(Debug)]
pub struct CmdOutput {
    /// Text to write to stdout.
    pub stdout: String,
    /// Exit code: 0 = no errors, 1 = validation errors, 2 = parse/io error.
    pub exit_code: u8,
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Validate `src` and return formatted output.
///
/// `project_dir` is the `.zen` file's parent directory, or `None`. See
/// [`validate_source`] for the stages: config policy and brand, parse,
/// validate, asset checks, import loading, and the compile check.
///
/// JSON diagnostics carry 1-based `line`/`col` when they have a span. A span
/// from an imported file adds `file` and locates over that file's text.
pub fn run(src: &str, project_dir: Option<&Path>, json: bool, flags: &PolicyFlags) -> CmdOutput {
    let collected = collect(src, project_dir, flags);
    output(
        &collected.diagnostics,
        src,
        &collected.files,
        json,
        collected.exit_code,
    )
}

/// The diagnostics of `zenith validate` for `src` on the native host.
///
/// This is the single source of what `zenith validate` reports. `zenith fix`
/// uses it for its `remaining` list.
#[must_use]
pub fn collect(src: &str, project_dir: Option<&Path>, flags: &PolicyFlags) -> Validation {
    validate_source(native::host(), src, project_dir, flags)
}

/// Format `diagnostics` as the JSON envelope or human lines.
///
/// `valid` is false when any diagnostic is an error.
fn output(
    diagnostics: &[Diagnostic],
    src: &str,
    files: &ImportFiles,
    json: bool,
    exit_code: u8,
) -> CmdOutput {
    let stdout = if json {
        serialize_pretty(&ValidateOutput {
            schema: "zenith-validate-v1",
            valid: !Diagnostic::has_errors(diagnostics),
            diagnostics: DiagnosticJson::located_all_in(diagnostics, src, files),
        })
    } else if diagnostics.is_empty() {
        "ok — no diagnostics".to_owned()
    } else {
        human_diagnostic_lines(diagnostics, &mut Locator::with_files(src, files)).join("\n")
    };
    CmdOutput { stdout, exit_code }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_DOC: &str = r##"zenith version=1 {
  project id="proj.v" name="Validate Test"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#f8fafc"
    token id="color.accent" type="color" value="#3b82f6"
  }
  styles {}
  document id="doc.v" title="Validate Test" {
    page id="page.v" w=(px)320 h=(px)200 {
      rect id="rect.bg" x=(px)0 y=(px)0 w=(px)320 h=(px)200 fill=(token)"color.bg"
      rect id="rect.accent" x=(px)40 y=(px)40 w=(px)240 h=(px)120 fill=(token)"color.accent"
    }
  }
}
"##;

    const DUP_ID_DOC: &str = r##"zenith version=1 {
  project id="proj.d" name="Dup"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.bg" type="color" value="#000000"
  }
  styles {}
  document id="doc.d" title="Dup" {
    page id="page.d" w=(px)100 h=(px)100 {
      rect id="rect.d" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.bg"
    }
  }
}
"##;

    #[test]
    fn valid_doc_exits_zero() {
        let out = run(VALID_DOC, None, false, &PolicyFlags::default());
        assert_eq!(out.exit_code, 0, "stdout: {}", out.stdout);
    }

    #[test]
    fn valid_doc_human_output_is_ok() {
        let out = run(VALID_DOC, None, false, &PolicyFlags::default());
        assert!(
            out.stdout.contains("ok"),
            "expected 'ok' in human output; got: {}",
            out.stdout
        );
    }

    #[test]
    fn duplicate_id_exits_one() {
        let out = run(DUP_ID_DOC, None, false, &PolicyFlags::default());
        assert_eq!(out.exit_code, 1, "stdout: {}", out.stdout);
    }

    #[test]
    fn duplicate_id_reports_id_duplicate_code() {
        let out = run(DUP_ID_DOC, None, false, &PolicyFlags::default());
        assert!(
            out.stdout.contains("id.duplicate") || out.stdout.contains("token.duplicate_id"),
            "expected duplicate diagnostic code; got: {}",
            out.stdout
        );
    }

    #[test]
    fn valid_doc_json_has_schema_field() {
        let out = run(VALID_DOC, None, true, &PolicyFlags::default());
        assert!(
            out.stdout.contains("zenith-validate-v1"),
            "JSON must contain schema field; got: {}",
            out.stdout
        );
    }

    #[test]
    fn valid_doc_json_valid_true() {
        let out = run(VALID_DOC, None, true, &PolicyFlags::default());
        assert!(
            out.stdout.contains(r#""valid": true"#),
            "valid doc JSON must have valid=true; got: {}",
            out.stdout
        );
    }

    #[test]
    fn import_missing_json_marks_document_invalid() {
        let dir = tempfile::tempdir().expect("tempdir");
        let src = r#"zenith version=1 {
  project id="proj.import" name="Import"
  imports {
    import id="brand" kind="zen" src="missing.zen"
  }
  document id="doc.import" title="Import" {
    page id="page.import" w=(px)100 h=(px)100
  }
}
"#;

        let out = run(src, Some(dir.path()), true, &PolicyFlags::default());

        assert_eq!(out.exit_code, 1, "stdout: {}", out.stdout);
        assert!(
            out.stdout.contains(r#""valid": false"#),
            "JSON must mark document invalid; got: {}",
            out.stdout
        );
        assert!(
            out.stdout.contains(r#""code": "import.missing""#),
            "JSON must contain import.missing; got: {}",
            out.stdout
        );
    }

    #[test]
    fn text_contrast_reports_once_with_the_authored_span() {
        let src = r##"zenith version=1 {
  project id="proj.c" name="C"
  tokens format="zenith-token-v1" {
    token id="color.w" type="color" value="#ffffff"
  }
  styles {}
  document id="doc.c" title="C" {
    page id="page.c" w=(px)200 h=(px)100 background=(token)"color.w" {
      text id="t" x=(px)10 y=(px)10 w=(px)180 h=(px)40 fill=(token)"color.w" { span "Hidden" }
    }
  }
}
"##;
        let out = run(src, None, true, &PolicyFlags::default());
        let json: serde_json::Value = serde_json::from_str(&out.stdout).expect("json");
        let found: Vec<&serde_json::Value> = json["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .filter(|d| d["code"] == "contrast.invisible")
            .collect();
        assert_eq!(found.len(), 1, "stdout: {}", out.stdout);
        assert_eq!(found[0]["subject_id"], "t");
        assert_eq!(found[0]["line"], 9, "stdout: {}", out.stdout);
    }

    /// An id-less `text` on line 7, column 7.
    const IDLESS_TEXT_DOC: &str = r##"zenith version=1 {
  project id="proj.t" name="T"
  tokens format="zenith-token-v1" {
  }
  document id="doc.t" title="T" {
    page id="pg" w=(px)100 h=(px)100 {
      text x=(px)0 y=(px)0 w=(px)50 h=(px)20 { span "Hi" }
    }
  }
}
"##;

    #[test]
    fn human_output_shows_parse_error_location() {
        let out = run(IDLESS_TEXT_DOC, None, false, &PolicyFlags::default());
        assert_eq!(out.exit_code, 2, "stdout: {}", out.stdout);
        assert!(
            out.stdout.starts_with("error[parse.error] 7:7: "),
            "stdout: {}",
            out.stdout
        );
    }

    #[test]
    fn human_output_without_span_has_no_location() {
        let d = Diagnostic::error("x.bad", "boom", None, Some("n".into()));
        let out = output(&[d], IDLESS_TEXT_DOC, &ImportFiles::default(), false, 1);
        assert_eq!(out.stdout, "error[x.bad] (n): boom");
    }

    #[test]
    fn parse_error_exits_two() {
        let out = run("not kdl !!!{{{", None, false, &PolicyFlags::default());
        assert_eq!(out.exit_code, 2, "stdout: {}", out.stdout);
    }
}
