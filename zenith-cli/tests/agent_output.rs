//! Integration tests: every `--json` path prints machine-readable JSON, and a
//! render reports each diagnostic once.
//!
//! Each test runs the `zenith` binary in a tempdir and parses stdout as JSON.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

// ── Helpers ───────────────────────────────────────────────────────────────────

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

fn zenith(args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .args(args)
        .output()
        .expect("run zenith");
    Run {
        code: output.status.code().expect("exit code"),
        stdout: String::from_utf8(output.stdout).expect("stdout utf8"),
        stderr: String::from_utf8(output.stderr).expect("stderr utf8"),
    }
}

fn json(run: &Run) -> Value {
    serde_json::from_str(&run.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not one JSON document ({e}); stdout:\n{}\nstderr:\n{}",
            run.stdout, run.stderr
        )
    })
}

fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, body).expect("write fixture");
    path
}

fn s(path: &Path) -> &str {
    path.to_str().expect("utf8 path")
}

fn diagnostics(v: &Value) -> &Vec<Value> {
    v["diagnostics"].as_array().expect("diagnostics array")
}

fn codes(v: &Value) -> Vec<String> {
    diagnostics(v)
        .iter()
        .map(|d| d["code"].as_str().expect("code").to_owned())
        .collect()
}

/// Assert `run` printed a `zenith-error-v1` envelope whose first code is `code`.
fn assert_error_envelope(run: &Run, code: &str) {
    let v = json(run);
    assert_eq!(v["schema"], "zenith-error-v1", "stdout: {}", run.stdout);
    assert_eq!(codes(&v).first().map(String::as_str), Some(code));
    let d = &diagnostics(&v)[0];
    assert_eq!(d["severity"], "error");
    assert!(
        !d["message"].as_str().unwrap_or("").is_empty(),
        "message must be non-empty"
    );
}

// ── Fixtures ──────────────────────────────────────────────────────────────────

const VALID_DOC: &str = r##"zenith version=1 {
  project id="proj.ok" name="OK"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
  }
  styles {}
  document id="doc.ok" title="OK" {
    page id="page.ok" w=(px)100 h=(px)100 {
      rect id="rect.bg" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.bg"
    }
  }
}
"##;

const DUP_TOKEN_DOC: &str = r##"zenith version=1 {
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

/// Validates clean; compiling it raises the `text.fit_failed` Error.
const FIT_FAILED_DOC: &str = r##"zenith version=1 {
  project id="proj.fit" name="Fit"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.fit" title="Fit" {
    page id="page.fit" w=(px)400 h=(px)400 {
      text id="text.fit" x=(px)10 y=(px)10 w=(px)60 h=(px)20 overflow="fit" {
        span "The quick brown fox jumps over the lazy dog and keeps on going"
      }
    }
  }
}
"##;

/// Validates clean; compiling it raises the `text.overflow` Warning.
const CLIP_DOC: &str = r##"zenith version=1 {
  project id="proj.clip" name="Clip"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.clip" title="Clip" {
    page id="page.clip" w=(px)400 h=(px)400 {
      text id="text.clipped" x=(px)10 y=(px)10 w=(px)60 h=(px)20 overflow="clip" {
        span "The quick brown fox jumps over the lazy dog and keeps on going"
      }
    }
  }
}
"##;

/// Three pages, each with a `(data)` background: one shared `data.no_context`.
const THREE_PAGE_DATA_DOC: &str = r##"zenith version=1 {
  project id="proj.three" name="Three"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.three" title="Three" {
    page id="page.1" w=(px)60 h=(px)40 background=(data)"c"
    page id="page.2" w=(px)60 h=(px)40 background=(data)"c"
    page id="page.3" w=(px)60 h=(px)40 background=(data)"c"
  }
}
"##;

/// A table with `n` cells whose text asks for a missing font family.
fn table_doc(n: usize) -> String {
    let rows: String = (0..n)
        .map(|i| {
            format!(
                "        row {{ cell {{ text id=\"cell.{i}\" x=(px)0 y=(px)0 w=(px)200 h=(px)20 \
                 font-family=(token)\"font.missing\" {{ span \"c{i}\" }} }} }}\n"
            )
        })
        .collect();
    format!(
        r##"zenith version=1 {{
  project id="proj.tbl" name="Table"
  tokens format="zenith-token-v1" {{
    token id="font.missing" type="fontFamily" value="Totally Missing Family"
  }}
  styles {{}}
  document id="doc.tbl" title="Table" {{
    page id="page.tbl" w=(px)400 h=(px)400 {{
      table id="tbl" x=(px)0 y=(px)0 w=(px)400 h=(px)400 {{
        column
{rows}      }}
    }}
  }}
}}
"##
    )
}

// ── render --json: blocked paths ──────────────────────────────────────────────

#[test]
fn render_json_validation_error_prints_blocked_envelope() {
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "dup.zen", DUP_TOKEN_DOC);
    let png = tmp.path().join("out.png");
    let run = zenith(&["render", s(&doc), "--png", s(&png), "--json"]);
    assert_eq!(run.code, 1, "stderr: {}", run.stderr);
    let v = json(&run);
    assert_eq!(v["schema"], "zenith-render-v1");
    assert_eq!(v["status"], "blocked");
    assert!(
        diagnostics(&v).iter().any(|d| d["severity"] == "error"),
        "an error diagnostic must be present: {}",
        run.stdout
    );
    assert!(!png.exists(), "a blocked render must not write the PNG");
}

#[test]
fn render_json_parse_error_prints_blocked_envelope_with_location() {
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "bad.zen", "zenith version=1 {\n  oops {{{\n");
    let png = tmp.path().join("out.png");
    let run = zenith(&["render", s(&doc), "--png", s(&png), "--json"]);
    assert_eq!(run.code, 2, "stderr: {}", run.stderr);
    let v = json(&run);
    assert_eq!(v["schema"], "zenith-render-v1");
    assert_eq!(v["status"], "blocked");
    assert_eq!(codes(&v), vec!["parse.error"]);
    let d = &diagnostics(&v)[0];
    if d.get("line").is_some() {
        assert!(d["line"].as_u64().expect("line") >= 1);
        assert!(d["col"].as_u64().expect("col") >= 1);
    }
}

#[test]
fn render_json_compile_error_prints_blocked_envelope() {
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "fit.zen", FIT_FAILED_DOC);
    let png = tmp.path().join("out.png");
    let run = zenith(&["render", s(&doc), "--png", s(&png), "--json"]);
    assert_eq!(run.code, 2, "stderr: {}", run.stderr);
    let v = json(&run);
    assert_eq!(v["status"], "blocked");
    assert!(codes(&v).iter().any(|c| c == "text.fit_failed"));
    assert_eq!(v["outputs"].as_array().map(Vec::len), Some(0));
    let d = diagnostics(&v)
        .iter()
        .find(|d| d["code"] == "text.fit_failed")
        .expect("fit_failed entry");
    assert_eq!(d["line"], 7, "fit_failed must point at the text node line");
    assert!(!png.exists());
}

#[test]
fn render_json_ok_has_status_and_outputs() {
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "ok.zen", VALID_DOC);
    let png = tmp.path().join("out.png");
    let pdf = tmp.path().join("out.pdf");
    let run = zenith(&[
        "render",
        s(&doc),
        "--png",
        s(&png),
        "--pdf",
        s(&pdf),
        "--json",
    ]);
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    let v = json(&run);
    assert_eq!(v["status"], "ok");
    assert_eq!(v["outputs"].as_array().map(Vec::len), Some(2));
}

#[test]
fn render_json_missing_output_flag_is_error_envelope() {
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "ok.zen", VALID_DOC);
    let run = zenith(&["render", s(&doc), "--json"]);
    assert_eq!(run.code, 2);
    assert_error_envelope(&run, "cli.invalid_argument");
}

#[test]
fn render_json_missing_file_is_error_envelope() {
    let tmp = TempDir::new().expect("tempdir");
    let png = tmp.path().join("out.png");
    let missing = tmp.path().join("missing.zen");
    let run = zenith(&["render", s(&missing), "--png", s(&png), "--json"]);
    assert_eq!(run.code, 2);
    assert_error_envelope(&run, "io.read_failed");
}

// ── Other commands: error paths print JSON ────────────────────────────────────

#[test]
fn validate_json_missing_file_is_error_envelope() {
    let tmp = TempDir::new().expect("tempdir");
    let missing = tmp.path().join("missing.zen");
    let run = zenith(&["validate", s(&missing), "--json"]);
    assert_eq!(run.code, 2);
    assert_error_envelope(&run, "io.read_failed");
}

#[test]
fn tx_json_bad_transaction_is_error_envelope() {
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "ok.zen", VALID_DOC);
    let tx = write(tmp.path(), "tx.json", "{ not json");
    let run = zenith(&["tx", s(&doc), s(&tx), "--json"]);
    assert_eq!(run.code, 2);
    assert_error_envelope(&run, "tx.parse");
}

#[test]
fn tx_json_doc_parse_error_is_error_envelope() {
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "bad.zen", "oops {{{");
    let tx = write(tmp.path(), "tx.json", r#"{"ops":[]}"#);
    let run = zenith(&["tx", s(&doc), s(&tx), "--json"]);
    assert_eq!(run.code, 2);
    assert_error_envelope(&run, "parse.error");
}

#[test]
fn inspect_json_parse_error_is_error_envelope() {
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "bad.zen", "oops {{{");
    let run = zenith(&["inspect", s(&doc), "--json"]);
    assert_eq!(run.code, 2);
    assert_error_envelope(&run, "parse.error");
}

#[test]
fn tokens_json_parse_error_is_error_envelope() {
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "bad.zen", "oops {{{");
    let run = zenith(&["tokens", s(&doc), "--json"]);
    assert_eq!(run.code, 2);
    assert_error_envelope(&run, "parse.error");
}

#[test]
fn fmt_json_parse_error_is_error_envelope() {
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "bad.zen", "oops {{{");
    let run = zenith(&["fmt", s(&doc), "--json"]);
    assert_eq!(run.code, 2);
    assert_error_envelope(&run, "parse.error");
}

#[test]
fn merge_json_without_data_nodes_is_error_envelope() {
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "ok.zen", VALID_DOC);
    let csv = write(tmp.path(), "rows.csv", "name\nAda\n");
    let out = tmp.path().join("out");
    let run = zenith(&["merge", s(&doc), s(&csv), "--out-dir", s(&out), "--json"]);
    assert_eq!(run.code, 2);
    assert_error_envelope(&run, "merge.no_data_nodes");
}

#[test]
fn merge_json_parse_error_is_error_envelope() {
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "bad.zen", "oops {{{");
    let csv = write(tmp.path(), "rows.csv", "name\nAda\n");
    let out = tmp.path().join("out");
    let run = zenith(&["merge", s(&doc), s(&csv), "--out-dir", s(&out), "--json"]);
    assert_eq!(run.code, 2);
    assert_error_envelope(&run, "parse.error");
}

#[test]
fn variant_json_parse_error_is_error_envelope() {
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "bad.zen", "oops {{{");
    let out = tmp.path().join("out");
    let run = zenith(&["variant", s(&doc), "--out-dir", s(&out), "--json"]);
    assert_eq!(run.code, 2);
    assert_error_envelope(&run, "parse.error");
}

#[test]
fn library_show_json_unknown_package_is_error_envelope() {
    let tmp = TempDir::new().expect("tempdir");
    let run = zenith(&[
        "library",
        "show",
        "@nope/missing#thing",
        s(tmp.path()),
        "--json",
    ]);
    assert_eq!(run.code, 2);
    assert_error_envelope(&run, "library.show_failed");
}

#[test]
fn library_add_json_malformed_spec_is_error_envelope() {
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "ok.zen", VALID_DOC);
    let run = zenith(&[
        "library",
        "add",
        "no-hash-here",
        "--into",
        s(&doc),
        "--json",
    ]);
    assert_eq!(run.code, 2);
    assert_error_envelope(&run, "library.add_failed");
}

#[test]
fn theme_apply_json_missing_doc_is_error_envelope() {
    let tmp = TempDir::new().expect("tempdir");
    let missing = tmp.path().join("missing.zen");
    let run = zenith(&["theme", "apply", "cobalt", s(&missing), "--json"]);
    assert_eq!(run.code, 2);
    assert_error_envelope(&run, "io.read_failed");
}

#[test]
fn theme_apply_json_unknown_pack_is_error_envelope() {
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "ok.zen", VALID_DOC);
    let run = zenith(&["theme", "apply", "no-such-pack-xyz", s(&doc), "--json"]);
    assert_eq!(run.code, 2);
    assert_error_envelope(&run, "theme.apply_failed");
}

// ── Duplicates and grouping ───────────────────────────────────────────────────

/// Identity of one JSON diagnostic for the duplicate check.
fn identity(d: &Value) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        d["code"], d["severity"], d["message"], d["subject_id"], d["line"]
    )
}

#[test]
fn all_pages_json_reports_each_diagnostic_once() {
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "three.zen", THREE_PAGE_DATA_DOC);
    let out = tmp.path().join("pages");
    let run = zenith(&["render", s(&doc), "--all-pages", s(&out), "--json"]);
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    let v = json(&run);
    assert_eq!(v["status"], "ok");
    let all = diagnostics(&v);
    let unique: BTreeSet<String> = all.iter().map(identity).collect();
    assert_eq!(unique.len(), all.len(), "duplicates: {}", run.stdout);
    let no_context = codes(&v).iter().filter(|c| *c == "data.no_context").count();
    assert_eq!(no_context, 1, "shared diagnostic once: {}", run.stdout);
    assert_eq!(v["outputs"].as_array().map(Vec::len), Some(3));
}

#[test]
fn validate_json_reports_compile_stage_text_overflow() {
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "clip.zen", CLIP_DOC);
    let run = zenith(&["validate", s(&doc), "--json"]);
    assert_eq!(run.code, 0, "stdout: {}", run.stdout);
    let v = json(&run);
    assert_eq!(v["schema"], "zenith-validate-v1");
    assert_eq!(v["valid"], true);
    assert!(
        codes(&v).iter().any(|c| c == "text.overflow"),
        "validate must report text.overflow: {}",
        run.stdout
    );
}

#[test]
fn table_cells_in_missing_family_report_one_font_unresolved_each() {
    const N: usize = 5;
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "table.zen", &table_doc(N));
    let png = tmp.path().join("out.png");
    let run = zenith(&["render", s(&doc), "--png", s(&png), "--json"]);
    assert_eq!(
        run.code, 0,
        "stdout: {}\nstderr: {}",
        run.stdout, run.stderr
    );
    let v = json(&run);
    let unresolved: Vec<&Value> = diagnostics(&v)
        .iter()
        .filter(|d| d["code"] == "font.unresolved")
        .collect();
    assert_eq!(unresolved.len(), N, "stdout: {}", run.stdout);
    assert!(
        unresolved.iter().all(|d| d["cause"].is_string()),
        "font.unresolved carries a cause"
    );

    let validate = zenith(&["validate", s(&doc), "--json"]);
    let vv = json(&validate);
    let in_validate = codes(&vv)
        .iter()
        .filter(|c| *c == "font.unresolved")
        .count();
    assert_eq!(in_validate, N, "validate stdout: {}", validate.stdout);
}

#[test]
fn render_human_groups_same_cause_advisories() {
    let tmp = TempDir::new().expect("tempdir");
    let doc = write(tmp.path(), "table.zen", &table_doc(4));
    let png = tmp.path().join("out.png");
    let run = zenith(&["render", s(&doc), "--png", s(&png)]);
    assert_eq!(run.code, 0, "stderr: {}", run.stderr);
    let grouped: Vec<&str> = run
        .stderr
        .lines()
        .filter(|l| l.starts_with("advisory[font.unresolved]"))
        .collect();
    assert_eq!(grouped.len(), 1, "stderr: {}", run.stderr);
    assert!(grouped[0].contains("4 nodes: cell.0, cell.1, cell.2, cell.3"));
}
