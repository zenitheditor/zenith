//! Pure logic for `zenith validate`.
//!
//! The public entry point [`run`] operates entirely on in-memory source text;
//! the caller is responsible for all filesystem I/O.

use std::path::Path;

use zenith_core::{
    Diagnostic, KdlAdapter, KdlSource, Severity, merge_brand_contract, validate_with_policy,
};

use crate::commands::composition_imports::load_import_graph;
use crate::commands::render::{
    collect_image_dimension_diagnostics, collect_missing_asset_diagnostics,
    compile_check_diagnostics,
};
use crate::commands::serialize_pretty;
use crate::config::{CliPolicyFlags, load_global_and_local, merge_policy};
use crate::json_types::{DiagnosticJson, ValidateOutput};
use crate::report::human_diagnostic_lines;

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
/// When `project_dir` is `Some` (the `.zen` file's parent directory), each
/// declared asset's file is checked for existence and a hard `asset.missing`
/// Error diagnostic is added for any that are absent, and that directory is the
/// starting point for the local `.zenith.kdl` config walk-up. When `None`, no
/// asset files are checked and no local config is discovered.
///
/// The effective diagnostic policy is `merge_policy(global, local, in_file,
/// flags)` — global/local config plus the document's own `diagnostics` block
/// plus the `--allow/--warn/--deny` flags — applied once via
/// [`validate_with_policy`]. With no config files and no flags the merged policy
/// is identical to the document's in-file policy, so output is unchanged.
///
/// With no Error diagnostic, every page also compiles (no raster), so the
/// compile-stage diagnostics `render` reports (`text.overflow`,
/// `font.unresolved`, …) show here in the same round. Repeats are removed.
/// JSON diagnostics carry 1-based `line`/`col` when they have a span.
///
/// - Parse errors and config-load errors produce `exit_code = 2`.
/// - Documents with at least one error-severity diagnostic produce
///   `exit_code = 1`.
/// - Clean documents produce `exit_code = 0`.
pub fn run(src: &str, project_dir: Option<&Path>, json: bool, flags: &CliPolicyFlags) -> CmdOutput {
    // Resolve config policy and brand contract ───────────────────────────────
    // Global config is always consulted; local config is walked up from the
    // document's directory when known. A load error is a hard exit-2 failure.
    let (global, local, global_brand, local_brand) = match load_global_and_local(project_dir) {
        Ok(quad) => quad,
        Err(msg) => {
            let d = Diagnostic::error("config.error", msg, None, None);
            return output(&[d], src, json, 2);
        }
    };

    // Parse ─────────────────────────────────────────────────────────────────
    let doc = match KdlAdapter.parse(src.as_bytes()) {
        Ok(d) => d,
        Err(e) => {
            let d = Diagnostic::error("parse.error", e.message, e.span, None);
            return output(&[d], src, json, 2);
        }
    };

    // Validate ───────────────────────────────────────────────────────────────
    // Policy: global ++ local ++ in-file ++ CLI flags (last-wins).
    // Brand:  global → local → in-file (per-category override, higher wins).
    let merged = merge_policy(&global, &local, &doc.diagnostic_policy, flags);
    let effective_brand = merge_brand_contract(
        &merge_brand_contract(&global_brand, &local_brand),
        &doc.brand_contract,
    );
    let mut diagnostics = validate_with_policy(&doc, &merged, &effective_brand).diagnostics;
    if let Some(dir) = project_dir {
        diagnostics.extend(collect_missing_asset_diagnostics(&doc, dir));
        diagnostics.extend(collect_image_dimension_diagnostics(&doc, dir));
    }
    let imports = load_import_graph(&doc, project_dir);
    diagnostics.extend(imports.diagnostics().iter().cloned());
    if !has_errors(&diagnostics) {
        diagnostics.extend(compile_check_diagnostics(
            &doc,
            project_dir,
            &imports,
            &merged,
        ));
    }
    let diagnostics = Diagnostic::dedup(diagnostics);
    let exit_code = if has_errors(&diagnostics) { 1 } else { 0 };
    output(&diagnostics, src, json, exit_code)
}

fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| d.severity == Severity::Error)
}

/// Format `diagnostics` as the JSON envelope or human lines.
///
/// `valid` is false when any diagnostic is an error.
fn output(diagnostics: &[Diagnostic], src: &str, json: bool, exit_code: u8) -> CmdOutput {
    let stdout = if json {
        serialize_pretty(&ValidateOutput {
            schema: "zenith-validate-v1",
            valid: !has_errors(diagnostics),
            diagnostics: DiagnosticJson::located_all(diagnostics, src),
        })
    } else if diagnostics.is_empty() {
        "ok — no diagnostics".to_owned()
    } else {
        human_diagnostic_lines(diagnostics).join("\n")
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
        let out = run(VALID_DOC, None, false, &CliPolicyFlags::default());
        assert_eq!(out.exit_code, 0, "stdout: {}", out.stdout);
    }

    #[test]
    fn valid_doc_human_output_is_ok() {
        let out = run(VALID_DOC, None, false, &CliPolicyFlags::default());
        assert!(
            out.stdout.contains("ok"),
            "expected 'ok' in human output; got: {}",
            out.stdout
        );
    }

    #[test]
    fn duplicate_id_exits_one() {
        let out = run(DUP_ID_DOC, None, false, &CliPolicyFlags::default());
        assert_eq!(out.exit_code, 1, "stdout: {}", out.stdout);
    }

    #[test]
    fn duplicate_id_reports_id_duplicate_code() {
        let out = run(DUP_ID_DOC, None, false, &CliPolicyFlags::default());
        assert!(
            out.stdout.contains("id.duplicate") || out.stdout.contains("token.duplicate_id"),
            "expected duplicate diagnostic code; got: {}",
            out.stdout
        );
    }

    #[test]
    fn valid_doc_json_has_schema_field() {
        let out = run(VALID_DOC, None, true, &CliPolicyFlags::default());
        assert!(
            out.stdout.contains("zenith-validate-v1"),
            "JSON must contain schema field; got: {}",
            out.stdout
        );
    }

    #[test]
    fn valid_doc_json_valid_true() {
        let out = run(VALID_DOC, None, true, &CliPolicyFlags::default());
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

        let out = run(src, Some(dir.path()), true, &CliPolicyFlags::default());

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
    fn parse_error_exits_two() {
        let out = run("not kdl !!!{{{", None, false, &CliPolicyFlags::default());
        assert_eq!(out.exit_code, 2, "stdout: {}", out.stdout);
    }
}
