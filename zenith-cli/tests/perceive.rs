use std::fs;
use std::process::Command;

fn vector_doc() -> &'static str {
    r#"zenith version=1 {
  project id="proj.perceive" name="Perceive"
  tokens format="zenith-token-v1" { }
  styles { }
  document id="doc.perceive" title="Perceive" {
    page id="page.main" w=(px)120 h=(px)120 {
      path id="mark" closed=#true {
        anchor x=(px)0 y=(px)0
        anchor x=(px)40 y=(px)0
        anchor x=(px)40 y=(px)40
        anchor x=(px)0 y=(px)40
      }
    }
  }
}"#
}

#[test]
fn perceive_vector_json_reports_path_metrics() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let doc = tmp.path().join("logo.zen");
    fs::write(&doc, vector_doc()).expect("write doc");

    let output = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .arg("perceive")
        .arg("vector")
        .arg(&doc)
        .arg("--json")
        .output()
        .expect("run zenith");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("\"schema\": \"zenith-perceive-vector-v1\""));
    assert!(stdout.contains("\"path_count\": 1"));
    assert!(stdout.contains("\"id\": \"mark\""));
}

#[test]
fn perceive_vector_node_filter_reports_selected_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let doc = tmp.path().join("logo.zen");
    fs::write(&doc, vector_doc()).expect("write doc");

    let output = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .arg("perceive")
        .arg("vector")
        .arg(&doc)
        .arg("--node")
        .arg("mark")
        .arg("--json")
        .output()
        .expect("run zenith");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(stdout.contains("\"path_count\": 1"));
    assert!(stdout.contains("\"id\": \"mark\""));
}

fn page_doc() -> &'static str {
    r##"zenith version=1 {
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.fg" type="color" value="#000000"
  }
  document id="doc.page" title="Page" {
    page id="page.one" w=(px)60 h=(px)40 background=(token)"color.bg" {
      construction {
        guide id="g" type="segment" x1=(px)10 y1=(px)0 x2=(px)10 y2=(px)40
      }
      rect id="r" x=(px)10 y=(px)10 w=(px)20 h=(px)20 fill=(token)"color.fg"
    }
    page id="page.two" w=(px)60 h=(px)40 background=(token)"color.bg" { }
  }
}"##
}

fn perceive_page(args: &[&str]) -> std::process::Output {
    let tmp = tempfile::tempdir().expect("tempdir");
    let doc = tmp.path().join("page.zen");
    fs::write(&doc, page_doc()).expect("write doc");
    Command::new(env!("CARGO_BIN_EXE_zenith"))
        .args(["perceive", "page"])
        .arg(&doc)
        .args(args)
        .output()
        .expect("run zenith")
}

#[test]
fn perceive_page_json_is_the_schema_one_envelope() {
    let output = perceive_page(&["--json"]);
    assert!(output.status.success(), "{output:?}");
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(v["schema"], "zenith-perceive-page-v1");
    assert_eq!(v["decimals"], 4);
    assert_eq!(v["pages"].as_array().map(Vec::len), Some(1));
    let page = &v["pages"][0];
    assert_eq!(page["ink"]["pixel_count"], 400);
    assert_eq!(page["metrics"]["grid_conformance"]["status"], "measured");
    assert_eq!(page["metrics"]["grid_conformance"]["guide_count"], 1);
    assert_eq!(page["metrics"]["clearspace"]["status"], "not_applicable");
    assert!(
        v.get("version").is_none(),
        "no tool version in the envelope"
    );
}

#[test]
fn perceive_page_all_pages_and_page_select_pages() {
    let all = perceive_page(&["--all-pages", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&all.stdout).expect("json");
    assert_eq!(v["pages"].as_array().map(Vec::len), Some(2));
    let two = perceive_page(&["--page", "2", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&two.stdout).expect("json");
    assert_eq!(v["pages"][0]["id"], "page.two");
}

#[test]
fn perceive_page_text_names_every_metric() {
    let output = perceive_page(&[]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    assert!(stdout.contains("page 1 (page.one): 60x40 px"), "{stdout}");
    assert!(stdout.contains("clearspace: not applicable"), "{stdout}");
    assert!(stdout.contains("grid_conformance: guides 1"), "{stdout}");
}

#[test]
fn perceive_page_out_of_range_is_a_json_error() {
    let output = perceive_page(&["--page", "9", "--json"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(v["schema"], "zenith-error-v1");
}

#[test]
fn perceive_page_rejects_page_with_all_pages() {
    let output = perceive_page(&["--page", "1", "--all-pages"]);
    assert!(!output.status.success());
}
