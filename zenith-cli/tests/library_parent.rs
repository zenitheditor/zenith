//! Integration tests for `zenith library add --parent`.

use std::process::Command;

use serde_json::Value;

const DOC: &str = r#"zenith version=1 {
  project id="proj.x" name="Target"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="d" title="x" {
    page id="pg" w=(px)800 h=(px)600 {
      frame id="card" x=(px)100 y=(px)50 w=(px)300 h=(px)200 {}
    }
  }
}
"#;

fn add(dir: &std::path::Path, extra: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .args([
            "library",
            "add",
            "@zenith/flowchart#decision",
            "--into",
            "t.zen",
            "--page",
            "pg",
            "--at",
            "5,6",
        ])
        .args(extra)
        .current_dir(dir)
        .env("ZENITH_DATA_DIR", dir.join("data"))
        .output()
        .expect("run zenith");
    (
        out.status.success(),
        String::from_utf8(out.stdout).expect("stdout utf8"),
        String::from_utf8(out.stderr).expect("stderr utf8"),
    )
}

#[test]
fn parent_flag_nests_the_instance_and_reports_parent_in_json() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("t.zen"), DOC).expect("write doc");

    let (ok, stdout, stderr) = add(dir.path(), &["--parent", "card", "--json"]);
    assert!(ok, "{stdout}\n{stderr}");
    let json: Value = serde_json::from_str(&stdout).expect("json");
    assert_eq!(json["schema"], "zenith-library-add-v1");
    assert_eq!(json["parent"], "card");
    assert_eq!(json["written"], true);

    let src = std::fs::read_to_string(dir.path().join("t.zen")).expect("read doc");
    let card = src.find("frame id=\"card\"").expect("card frame");
    let inst = src.find("instance id=\"decision\"").expect("instance");
    assert!(inst > card, "instance must follow the card frame:\n{src}");
    let close = src[card..].find("\n      }").map(|i| i + card);
    assert!(
        close.is_some_and(|c| inst < c),
        "instance must sit inside the card frame:\n{src}"
    );
}

#[test]
fn json_has_no_parent_field_without_the_flag() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("t.zen"), DOC).expect("write doc");

    let (ok, stdout, stderr) = add(dir.path(), &["--json"]);
    assert!(ok, "{stdout}\n{stderr}");
    let json: Value = serde_json::from_str(&stdout).expect("json");
    assert!(json.get("parent").is_none(), "{json:#}");
}

#[test]
fn unknown_parent_exits_nonzero_and_leaves_the_file_unchanged() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("t.zen"), DOC).expect("write doc");

    let (ok, _, stderr) = add(dir.path(), &["--parent", "nope"]);
    assert!(!ok);
    assert!(stderr.contains("parent 'nope' not found"), "{stderr}");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("t.zen")).expect("read doc"),
        DOC
    );
}
