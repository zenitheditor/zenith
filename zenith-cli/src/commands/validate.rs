//! Pure logic for `zenith validate`.
//!
//! The public entry point [`run`] operates entirely on in-memory source text;
//! the caller is responsible for all filesystem I/O.

use std::path::Path;

use zenith_core::{Diagnostic, KdlAdapter, KdlSource, merge_brand_contract, validate_with_policy};

use crate::commands::composition_imports::load_import_graph;
use crate::commands::render::{
    collect_image_dimension_diagnostics, collect_missing_asset_diagnostics,
    compile_check_diagnostics,
};
use crate::commands::serialize_pretty;
use crate::config::{CliPolicyFlags, load_global_and_local, merge_policy};
use crate::json_types::{DiagnosticJson, ValidateOutput};
use crate::report::{ImportFiles, Locator, attributed_loader_diagnostics, human_diagnostic_lines};

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
/// `font.unresolved`, `contrast.*`, …) show here in the same round. Repeats are removed.
/// JSON diagnostics carry 1-based `line`/`col` when they have a span. A span
/// from an imported file adds `file` and locates over that file's text.
///
/// - Parse errors and config-load errors produce `exit_code = 2`.
/// - Documents with at least one error-severity diagnostic produce
///   `exit_code = 1`.
/// - Clean documents produce `exit_code = 0`.
pub fn run(src: &str, project_dir: Option<&Path>, json: bool, flags: &CliPolicyFlags) -> CmdOutput {
    let collected = collect(src, project_dir, flags);
    output(
        &collected.diagnostics,
        src,
        &collected.files,
        json,
        collected.exit_code,
    )
}

/// Diagnostics of one validate run, before formatting.
#[derive(Debug)]
pub struct Collected {
    /// Every diagnostic, repeats removed.
    pub diagnostics: Vec<Diagnostic>,
    /// Import files that diagnostic spans index into.
    pub files: ImportFiles,
    /// 0 = no errors, 1 = validation errors, 2 = parse/config error.
    pub exit_code: u8,
}

/// Run the full validate pipeline on `src` and return its diagnostics.
///
/// This is the single source of what `zenith validate` reports; see [`run`]
/// for the stages. `zenith fix` uses it for its `remaining` list.
pub fn collect(src: &str, project_dir: Option<&Path>, flags: &CliPolicyFlags) -> Collected {
    let failed = |d: Diagnostic| Collected {
        diagnostics: vec![d],
        files: ImportFiles::default(),
        exit_code: 2,
    };
    // Resolve config policy and brand contract ───────────────────────────────
    // Global config is always consulted; local config is walked up from the
    // document's directory when known. A load error is a hard exit-2 failure.
    let (global, local, global_brand, local_brand) = match load_global_and_local(project_dir) {
        Ok(quad) => quad,
        Err(msg) => return failed(Diagnostic::error("config.error", msg, None, None)),
    };

    // Parse ─────────────────────────────────────────────────────────────────
    let doc = match KdlAdapter.parse(src.as_bytes()) {
        Ok(d) => d,
        Err(e) => return failed(Diagnostic::error("parse.error", e.message, e.span, None)),
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
    diagnostics.extend(attributed_loader_diagnostics(&imports));
    if !Diagnostic::has_errors(&diagnostics) {
        diagnostics.extend(compile_check_diagnostics(
            &doc,
            project_dir,
            &imports,
            &merged,
        ));
    }
    let diagnostics = Diagnostic::dedup(diagnostics);
    let exit_code = if Diagnostic::has_errors(&diagnostics) {
        1
    } else {
        0
    };
    Collected {
        diagnostics,
        files: ImportFiles::from_graph(&imports),
        exit_code,
    }
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
        let out = run(src, None, true, &CliPolicyFlags::default());
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
        let out = run(IDLESS_TEXT_DOC, None, false, &CliPolicyFlags::default());
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
        let out = run("not kdl !!!{{{", None, false, &CliPolicyFlags::default());
        assert_eq!(out.exit_code, 2, "stdout: {}", out.stdout);
    }
}
