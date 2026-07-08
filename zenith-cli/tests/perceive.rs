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
