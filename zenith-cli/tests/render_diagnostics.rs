//! Integration tests: `render` reports every diagnostic in one call, and a
//! span from an imported file is never mapped onto the host source text.
//!
//! Each test runs the `zenith` binary in a tempdir and parses stdout as JSON.

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

fn zenith(args: &[&str]) -> (i32, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .args(args)
        .output()
        .expect("run zenith");
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    let v = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is not JSON ({e}):\n{stdout}"));
    (output.status.code().expect("exit code"), v)
}

fn s(path: &Path) -> &str {
    path.to_str().expect("utf8 path")
}

fn find<'a>(v: &'a Value, code: &str) -> Vec<&'a Value> {
    v["diagnostics"]
        .as_array()
        .expect("diagnostics array")
        .iter()
        .filter(|d| d["code"] == code)
        .collect()
}

/// A valid document with an unused token, which validate reports as a
/// non-error diagnostic.
const UNUSED_TOKEN_DOC: &str = r##"zenith version=1 {
  project id="proj.ut" name="Unused"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.unused" type="color" value="#abcdef"
  }
  styles {}
  document id="doc.ut" title="Unused" {
    page id="page.ut" w=(px)100 h=(px)100 {
      rect id="rect.bg" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.bg"
    }
  }
}
"##;

#[test]
fn render_success_json_includes_validation_diagnostics() {
    let dir = TempDir::new().expect("tempdir");
    let doc = dir.path().join("doc.zen");
    fs::write(&doc, UNUSED_TOKEN_DOC).expect("write doc");
    let out = dir.path().join("out.png");

    let (code, v) = zenith(&["render", s(&doc), "--png", s(&out), "--json"]);

    assert_eq!(code, 0, "{v}");
    assert_eq!(v["status"], "ok");
    let unused = find(&v, "token.unused");
    assert_eq!(unused.len(), 1, "diagnostics: {}", v["diagnostics"]);
    assert_eq!(unused[0]["subject_id"], "color.unused");
    assert!(unused[0]["line"].is_u64(), "host span keeps line: {v}");
}

#[test]
fn render_success_human_prints_validation_diagnostics() {
    let dir = TempDir::new().expect("tempdir");
    let doc = dir.path().join("doc.zen");
    fs::write(&doc, UNUSED_TOKEN_DOC).expect("write doc");
    let out = dir.path().join("out.png");

    let output = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .args(["render", s(&doc), "--png", s(&out)])
        .output()
        .expect("run zenith");

    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr utf8");
    assert!(stderr.contains("token.unused"), "stderr: {stderr}");
}

/// `lib.zen` declares a nested import whose file is missing. The span of that
/// diagnostic sits in `lib.zen`, not in the (shorter) host document.
const LIB_WITH_MISSING_NESTED: &str = r#"zenith version=1 {
  project id="proj.lib" name="Lib"

  imports {
    import id="deep" kind="zen" src="absent.zen"
  }
  document id="doc.lib" title="Lib" {
    page id="page.lib" w=(px)10 h=(px)10
  }
}
"#;

const HOST_IMPORTING_LIB: &str = r#"zenith version=1 {
  project id="proj.host" name="Host"
  imports {
    import id="lib" kind="zen" src="lib.zen"
  }
  document id="doc.host" title="Host" {
    page id="page.host" w=(px)10 h=(px)10
  }
}
"#;

#[test]
fn nested_import_diagnostic_locates_in_the_imported_file() {
    let dir = TempDir::new().expect("tempdir");
    let doc = dir.path().join("host.zen");
    fs::write(&doc, HOST_IMPORTING_LIB).expect("write host");
    let lib = dir.path().join("lib.zen");
    fs::write(&lib, LIB_WITH_MISSING_NESTED).expect("write lib");

    let (code, v) = zenith(&["validate", s(&doc), "--json"]);

    assert_eq!(code, 1, "{v}");
    let missing = find(&v, "import.missing");
    assert_eq!(missing.len(), 1, "diagnostics: {}", v["diagnostics"]);
    let d = missing[0];
    let file = d["file"].as_str().expect("file names the imported file");
    assert!(file.ends_with("lib.zen"), "file: {file}");
    let expected_line = LIB_WITH_MISSING_NESTED
        .lines()
        .position(|l| l.contains("import id=\"deep\""))
        .expect("import line")
        + 1;
    assert_eq!(d["line"], expected_line as u64, "diagnostic: {d}");
    assert_eq!(d["col"], 5, "diagnostic: {d}");
}

#[test]
fn host_import_diagnostic_has_no_file() {
    let dir = TempDir::new().expect("tempdir");
    let doc = dir.path().join("host.zen");
    fs::write(&doc, HOST_IMPORTING_LIB).expect("write host");

    let (code, v) = zenith(&["validate", s(&doc), "--json"]);

    assert_eq!(code, 1, "{v}");
    let missing = find(&v, "import.missing");
    assert_eq!(missing.len(), 1, "diagnostics: {}", v["diagnostics"]);
    assert!(
        missing[0].get("file").is_none(),
        "diagnostic: {}",
        missing[0]
    );
    assert_eq!(missing[0]["line"], 4);
}

#[test]
fn imported_asset_diagnostic_locates_in_the_imported_file_on_render() {
    let dir = TempDir::new().expect("tempdir");
    let lib_src = r#"zenith version=1 {
  project id="proj.lib" name="Lib"

  assets {
    asset id="logo" kind="image" src="missing.png"
  }
  document id="doc.lib" title="Lib" {
    page id="page.lib" w=(px)10 h=(px)10
  }
}
"#;
    let doc = dir.path().join("host.zen");
    fs::write(&doc, HOST_IMPORTING_LIB).expect("write host");
    fs::write(dir.path().join("lib.zen"), lib_src).expect("write lib");
    let out = dir.path().join("out.png");

    let (code, v) = zenith(&["render", s(&doc), "--png", s(&out), "--json"]);

    assert_eq!(code, 2, "{v}");
    assert_eq!(v["status"], "blocked");
    let missing = find(&v, "import.asset_missing");
    assert_eq!(missing.len(), 1, "diagnostics: {}", v["diagnostics"]);
    let d = missing[0];
    assert!(d["file"].as_str().expect("file").ends_with("lib.zen"));
    assert_eq!(d["line"], 5, "diagnostic: {d}");
    assert_eq!(d["col"], 5, "diagnostic: {d}");
}

/// An imported component whose text names a font family no face provides.
/// Compile reports `font.unresolved` with the span of that text node, which
/// sits in `lib.zen`.
const LIB_WITH_UNRESOLVED_FONT: &str = r##"zenith version=1 {
  project id="proj.lib" name="Lib"
  tokens format="zenith-token-v1" {
    token id="font.odd" type="fontFamily" value="Totally Missing Family"
  }
  styles {}
  components {
    component id="card" {
      text id="label" x=(px)0 y=(px)0 w=(px)80 h=(px)20 font-family=(token)"font.odd" {
        span "hi"
      }
    }
  }
  document id="doc.lib" title="Lib" {
    page id="page.lib" w=(px)10 h=(px)10
  }
}
"##;

const HOST_WITH_INSTANCE: &str = r#"zenith version=1 {
  project id="proj.host" name="Host"
  imports {
    import id="lib" kind="zen" src="lib.zen"
  }
  document id="doc.host" title="Host" {
    page id="page.host" w=(px)100 h=(px)100 {
      instance id="card" source="lib#component.card" x=(px)0 y=(px)0
    }
  }
}
"#;

#[test]
fn imported_component_diagnostic_locates_in_the_imported_file() {
    let dir = TempDir::new().expect("tempdir");
    let doc = dir.path().join("host.zen");
    fs::write(&doc, HOST_WITH_INSTANCE).expect("write host");
    fs::write(dir.path().join("lib.zen"), LIB_WITH_UNRESOLVED_FONT).expect("write lib");
    let out = dir.path().join("out.png");

    let (code, v) = zenith(&["render", s(&doc), "--png", s(&out), "--json"]);

    assert_eq!(code, 0, "{v}");
    let found = find(&v, "font.unresolved");
    assert_eq!(found.len(), 1, "diagnostics: {}", v["diagnostics"]);
    let d = found[0];
    assert!(
        d["file"].as_str().expect("file").ends_with("lib.zen"),
        "{d}"
    );
    let line = LIB_WITH_UNRESOLVED_FONT
        .lines()
        .position(|l| l.contains("text id=\"label\""))
        .expect("text line")
        + 1;
    assert_eq!(d["line"], line as u64, "diagnostic: {d}");
    assert_eq!(d["col"], 7, "diagnostic: {d}");
}

/// An imported document whose own token block has an alias to an unknown
/// token. The span of the diagnostic sits in `lib.zen`.
const LIB_WITH_BAD_TOKEN: &str = r##"zenith version=1 {
  project id="proj.lib" name="Lib"
  tokens format="zenith-token-v1" {
    token id="color.a" type="color" value="#ffffff"
    token id="color.alias" type="color" value=(token)"color.nope"
  }
  styles {}
  document id="doc.lib" title="Lib" {
    page id="page.lib" w=(px)10 h=(px)10
  }
}
"##;

#[test]
fn imported_token_diagnostic_locates_in_the_imported_file() {
    let dir = TempDir::new().expect("tempdir");
    let doc = dir.path().join("host.zen");
    fs::write(&doc, HOST_IMPORTING_LIB).expect("write host");
    fs::write(dir.path().join("lib.zen"), LIB_WITH_BAD_TOKEN).expect("write lib");
    let out = dir.path().join("out.png");

    let (_code, v) = zenith(&["render", s(&doc), "--png", s(&out), "--json"]);

    let found = find(&v, "token.unknown_reference");
    assert_eq!(found.len(), 1, "diagnostics: {}", v["diagnostics"]);
    let d = found[0];
    assert!(
        d["file"].as_str().expect("file").ends_with("lib.zen"),
        "{d}"
    );
    let line = LIB_WITH_BAD_TOKEN
        .lines()
        .position(|l| l.contains("color.alias"))
        .expect("token line")
        + 1;
    assert_eq!(d["line"], line as u64, "diagnostic: {d}");
    assert_eq!(d["col"], 5, "diagnostic: {d}");
}

/// An imported page whose text names a font family no face provides.
const LIB_WITH_PAGE: &str = r##"zenith version=1 {
  project id="proj.lib" name="Lib"
  tokens format="zenith-token-v1" {
    token id="font.odd" type="fontFamily" value="Totally Missing Family"
  }
  styles {}
  document id="doc.lib" title="Lib" {
    page id="x" w=(px)100 h=(px)100 {
      text id="label" x=(px)0 y=(px)0 w=(px)80 h=(px)20 font-family=(token)"font.odd" {
        span "hi"
      }
    }
  }
}
"##;

const HOST_WITH_PAGE_SOURCE: &str = r#"zenith version=1 {
  project id="proj.host" name="Host"
  imports {
    import id="lib" kind="zen" src="lib.zen"
  }
  document id="doc.host" title="Host" {
    page id="page.host" source="lib#page.x" w=(px)100 h=(px)100
  }
}
"#;

#[test]
fn imported_page_diagnostic_locates_in_the_imported_file() {
    let dir = TempDir::new().expect("tempdir");
    let doc = dir.path().join("host.zen");
    fs::write(&doc, HOST_WITH_PAGE_SOURCE).expect("write host");
    fs::write(dir.path().join("lib.zen"), LIB_WITH_PAGE).expect("write lib");
    let out = dir.path().join("out.png");

    let (code, v) = zenith(&["render", s(&doc), "--png", s(&out), "--json"]);

    assert_eq!(code, 0, "{v}");
    let found = find(&v, "font.unresolved");
    assert_eq!(found.len(), 1, "diagnostics: {}", v["diagnostics"]);
    let d = found[0];
    assert!(
        d["file"].as_str().expect("file").ends_with("lib.zen"),
        "{d}"
    );
    let line = LIB_WITH_PAGE
        .lines()
        .position(|l| l.contains("text id=\"label\""))
        .expect("text line")
        + 1;
    assert_eq!(d["line"], line as u64, "diagnostic: {d}");
    assert_eq!(d["col"], 7, "diagnostic: {d}");
}
