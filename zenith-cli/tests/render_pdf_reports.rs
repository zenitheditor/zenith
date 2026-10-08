use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;

fn document() -> String {
    let source =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/shadow.zen"))
            .expect("shadow source");
    let start = source.find("    page ").expect("page start");
    format!(
        "{}    page id=\"page.native\" w=(px)360 h=(px)260 {{}}\n{}",
        &source[..start],
        &source[start..]
    )
}

#[test]
fn pdf_reports_selected_document_page_and_governs_fallback() {
    let directory = tempfile::tempdir().expect("document directory");
    let store = tempfile::tempdir().expect("store directory");
    fs::write(directory.path().join("doc.zen"), document()).expect("write source");
    let run = |out: &str, extra: &[&str]| {
        let result = Command::new(env!("CARGO_BIN_EXE_zenith"))
            .args(["render", "doc.zen", "--pdf", out, "--page", "2", "--json"])
            .args(extra)
            .current_dir(directory.path())
            .env("ZENITH_DATA_DIR", store.path())
            .output()
            .expect("run CLI");
        let json: Value = serde_json::from_slice(&result.stdout).expect("JSON output");
        (result, json)
    };
    let (normal, report) = run("normal.pdf", &[]);
    assert!(normal.status.success(), "{report}");
    assert_eq!(report["rasterized_regions"][0]["page"], 2);
    assert_eq!(report["rasterized_regions"][0]["format"], "pdf");
    assert!(
        report["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|d| d["code"] == "render.pdf_rasterized")
    );
    let (allowed, report) = run("allowed.pdf", &["--allow", "render.pdf_rasterized"]);
    assert!(allowed.status.success(), "{report}");
    assert_eq!(report["rasterized_regions"][0]["page"], 2);
    assert!(
        !report["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|d| d["code"] == "render.pdf_rasterized")
    );
    assert_eq!(
        fs::read(directory.path().join("normal.pdf")).expect("normal bytes"),
        fs::read(directory.path().join("allowed.pdf")).expect("allowed bytes")
    );
    let (denied, report) = run("denied.pdf", &["--deny", "render.pdf_rasterized"]);
    assert!(!denied.status.success());
    assert_eq!(report["status"], "blocked");
    assert!(!directory.path().join("denied.pdf").exists());
}

#[test]
fn native_pdf_omits_empty_fallback_metadata() {
    let directory = tempfile::tempdir().expect("document directory");
    fs::write(directory.path().join("doc.zen"), document()).expect("write source");
    let result = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .args([
            "render",
            "doc.zen",
            "--pdf",
            "native.pdf",
            "--page",
            "1",
            "--json",
        ])
        .current_dir(directory.path())
        .env("ZENITH_DATA_DIR", directory.path().join("store"))
        .output()
        .expect("run CLI");
    let report: Value = serde_json::from_slice(&result.stdout).expect("JSON output");
    assert!(result.status.success(), "{report}");
    assert!(report.get("rasterized_regions").is_none());
}

#[test]
fn selected_pdf_resource_error_names_the_source_page() {
    let directory = tempfile::tempdir().expect("document directory");
    fs::write(directory.path().join("broken.png"), b"invalid image").expect("write asset");
    fs::write(
        directory.path().join("doc.zen"),
        r#"zenith version=1 {
  project id="project" name="PDF"
  assets { asset id="asset" kind="image" src="broken.png"; }
  document id="document" title="PDF" {
    page id="first" w=(px)100 h=(px)100 {}
    page id="second" w=(px)100 h=(px)100 {}
    page id="third" w=(px)100 h=(px)100 {
      image id="image" asset="asset" x=(px)0 y=(px)0 w=(px)20 h=(px)20
    }
  }
}"#,
    )
    .expect("write document");
    let result = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .args([
            "render", "doc.zen", "--pdf", "out.pdf", "--page", "3", "--json",
        ])
        .current_dir(directory.path())
        .env("ZENITH_DATA_DIR", directory.path().join("store"))
        .output()
        .expect("run CLI");
    let report: Value = serde_json::from_slice(&result.stdout).expect("JSON output");
    assert!(!result.status.success(), "{report}");
    let diagnostic = report["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .find(|d| d["code"] == "render.pdf_failed")
        .expect("PDF diagnostic");
    assert!(
        diagnostic["message"]
            .as_str()
            .expect("error text")
            .contains("document page 3"),
        "{diagnostic}"
    );
    assert!(!directory.path().join("out.pdf").exists());
}
