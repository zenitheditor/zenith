//! Batch SVG policy and command-line coverage.

use std::fs;
use std::process::{Command, Output};
use tempfile::TempDir;

const DOC: &str = r##"zenith version=1 {
  project id="project" name="Batch"
  tokens format="zenith-token-v1" {
    token id="ink" type="color" value="#222222"
    token id="shade" type="shadow" { layer dx=(px)1 dy=(px)1 blur=(px)2 color=(token)"ink"; }
  }
  styles {}
  document id="document" title="Batch" {
    page id="first" w=(px)100 h=(px)100 {
      text id="label" x=(px)0 y=(px)0 w=(px)100 h=(px)40 fill=(token)"ink" role="data.name" { span "name"; }
    }
    page id="second" w=(px)100 h=(px)100 {
      rect id="card" x=(px)20 y=(px)20 w=(px)30 h=(px)30 fill=(token)"ink" shadow=(token)"shade"
    }
  }
  variants {
    variant id="plain" source="first" w=(px)100 h=(px)100 {}
    variant id="shadow" source="second" w=(px)100 h=(px)100 {}
  }
}
"##;

struct Env {
    dir: TempDir,
    home: TempDir,
}

impl Env {
    fn new(doc: &str) -> Self {
        let env = Self {
            dir: TempDir::new().unwrap(),
            home: TempDir::new().unwrap(),
        };
        fs::write(env.dir.path().join("doc.zen"), doc).unwrap();
        fs::write(env.dir.path().join("data.csv"), "name\nAlice\n").unwrap();
        env
    }

    fn run(&self, command: &str, args: &[&str]) -> Output {
        let mut process = Command::new(env!("CARGO_BIN_EXE_zenith"));
        process.args([command, "doc.zen"]);
        if command == "merge" {
            process.arg("data.csv");
        }
        process
            .args(["--out-dir", "out"])
            .args(args)
            .current_dir(self.dir.path())
            .env("HOME", self.home.path());
        process.output().unwrap()
    }
}

#[test]
fn fallback_policy_applies_document_local_and_global_tiers_before_writes() {
    for tier in ["document", "local", "global"] {
        for verb in ["allow", "warn", "deny"] {
            for command in ["merge", "variant"] {
                let policy = format!("diagnostics {{ {verb} \"render.svg_rasterized\"; }}\n");
                let doc = if tier == "document" {
                    DOC.replacen("  styles {}", &format!("  {policy}  styles {{}}"), 1)
                } else {
                    DOC.to_owned()
                };
                let env = Env::new(&doc);
                match tier {
                    "local" => fs::write(env.dir.path().join(".zenith.kdl"), &policy).unwrap(),
                    "global" => {
                        let config = env.home.path().join(".config/zenith");
                        fs::create_dir_all(&config).unwrap();
                        fs::write(config.join("config.kdl"), &policy).unwrap();
                    }
                    "document" => {}
                    _ => unreachable!(),
                }
                let output = env.run(
                    command,
                    &["--format", "svg", "--json", "--manifest", "manifest.json"],
                );
                let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(
                    output.status.success(),
                    verb != "deny",
                    "{tier}/{verb}/{command}: {report}"
                );
                let records = report[if command == "merge" {
                    "rows"
                } else {
                    "variants"
                }]
                .as_array()
                .unwrap();
                let diagnostics: Vec<_> = records
                    .iter()
                    .flat_map(|r| r["diagnostics"].as_array().unwrap())
                    .filter(|d| d["code"] == "render.svg_rasterized")
                    .collect();
                if verb == "allow" {
                    assert!(diagnostics.is_empty());
                } else {
                    assert_eq!(diagnostics.len(), 1);
                    assert_eq!(
                        diagnostics[0]["severity"],
                        if verb == "deny" { "error" } else { "warning" }
                    );
                }
                if verb == "deny" {
                    if command == "merge" {
                        assert_eq!(fs::read_dir(env.dir.path().join("out")).unwrap().count(), 0);
                        assert_eq!(report["rows"][0]["outputs"], serde_json::json!([]));
                    } else {
                        assert!(env.dir.path().join("out/doc-plain.svg").exists());
                        assert!(!env.dir.path().join("out/doc-shadow.svg").exists());
                        assert!(!env.dir.path().join("out/doc-shadow.zen").exists());
                        assert_eq!(diagnostics[0]["subject_id"], "shadow");
                    }
                }
            }
        }
    }
}

#[test]
fn fallback_advisories_appear_in_human_and_json_reports() {
    for command in ["merge", "variant"] {
        let env = Env::new(DOC);
        let human = env.run(command, &["--format", "svg"]);
        assert!(human.status.success());
        let stderr = String::from_utf8(human.stderr).unwrap();
        assert!(stderr.contains("render.svg_rasterized"), "{stderr}");
        assert!(stderr.contains(if command == "merge" {
            "row 1:"
        } else {
            "variant shadow:"
        }));
        let json = env.run(command, &["--format", "svg", "--json"]);
        assert!(json.status.success());
        let report: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
        assert!(
            String::from_utf8(json.stdout)
                .unwrap()
                .contains("render.svg_rasterized")
        );
        if command == "variant" {
            let variant = report["variants"]
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["id"] == "shadow")
                .unwrap();
            let fallback = variant["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .find(|d| d["code"] == "render.svg_rasterized")
                .unwrap();
            assert_eq!(fallback["subject_id"], "shadow");
        }
    }
}

#[test]
fn png_default_keeps_manifest_and_json_fields_and_ignores_svg_policy() {
    for command in ["merge", "variant"] {
        let env = Env::new(DOC);
        fs::write(env.dir.path().join(".zenith.kdl"), "malformed {").unwrap();
        let output = env.run(command, &["--json", "--manifest", "manifest.json"]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(env.dir.path().join("manifest.json")).unwrap())
                .unwrap();
        assert_eq!(manifest["generator"], "1");
        if command == "variant" {
            assert_eq!(
                manifest["targets"][0],
                serde_json::json!({"id":"plain", "source":"first", "outputs_zen":"doc-plain.zen", "outputs_png":"doc-plain.png"})
            );
            assert_eq!(
                report["variants"][0],
                serde_json::json!({"id":"plain", "source":"first", "status":"ok", "outputs_zen":"doc-plain.zen", "outputs_png":"doc-plain.png", "diagnostics":[]})
            );
        } else {
            assert_eq!(
                manifest["rows"][0],
                serde_json::json!({"row":0,"outputs":["row-0001-page-1.png","row-0001-page-2.png"]})
            );
            assert_eq!(
                report["rows"][0],
                serde_json::json!({"row":0,"status":"ok","outputs":["row-0001-page-1.png","row-0001-page-2.png"],"diagnostics":[]})
            );
        }
        assert!(
            !String::from_utf8(output.stdout)
                .unwrap()
                .contains("outputs_svg")
        );
    }
}

#[test]
fn unsupported_batch_format_returns_argument_error_without_outputs() {
    for command in ["merge", "variant"] {
        let env = Env::new(DOC);
        let output = env.run(command, &["--format", "pdf"]);
        assert_eq!(output.status.code(), Some(2));
        assert!(!env.dir.path().join("out").exists());
    }
}

#[test]
fn svg_encoder_error_leaves_no_row_outputs() {
    let doc = DOC.replace("  styles {}", "  assets { asset id=\"bad\" kind=\"image\" src=\"bad.png\"; }\n  styles {}").replace("      rect id=\"card\"", "      image id=\"bad.image\" asset=\"bad\" x=(px)0 y=(px)0 w=(px)10 h=(px)10\n      rect id=\"card\"");
    let env = Env::new(&doc);
    fs::write(env.dir.path().join("bad.png"), b"invalid image").unwrap();
    let output = env.run("merge", &["--format", "svg", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        report["rows"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "render.svg_failed"),
        "{report}"
    );
    assert_eq!(fs::read_dir(env.dir.path().join("out")).unwrap().count(), 0);
}

#[test]
fn svg_encoder_error_leaves_no_variant_companion() {
    let doc = DOC.replace("  styles {}", "  assets { asset id=\"bad\" kind=\"image\" src=\"bad.png\"; }\n  styles {}").replace("      rect id=\"card\"", "      image id=\"bad.image\" asset=\"bad\" x=(px)0 y=(px)0 w=(px)10 h=(px)10\n      rect id=\"card\"");
    let env = Env::new(&doc);
    fs::write(env.dir.path().join("bad.png"), b"invalid image").unwrap();
    let output = env.run("variant", &["--format", "svg", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        report["variants"][1]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "render.svg_failed"),
        "{report}"
    );
    assert!(!env.dir.path().join("out/doc-shadow.svg").exists());
    assert!(!env.dir.path().join("out/doc-shadow.zen").exists());
    assert!(env.dir.path().join("out/doc-plain.svg").exists());
}

#[test]
fn variant_companion_write_error_keeps_error_and_fallback_diagnostics() {
    let env = Env::new(DOC);
    fs::create_dir_all(env.dir.path().join("out/doc-shadow.zen")).unwrap();
    let output = env.run("variant", &["--format", "svg", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostics = report["variants"][1]["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"] == "render.svg_rasterized")
    );
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"] == "variant.failed" && d["severity"] == "error")
    );
    assert!(!env.dir.path().join("out/doc-shadow.svg").exists());
}

#[test]
fn merge_later_page_write_error_keeps_error_and_fallback_diagnostics() {
    let env = Env::new(DOC);
    fs::create_dir_all(env.dir.path().join("out/row-0001-page-2.svg")).unwrap();
    let output = env.run(
        "merge",
        &["--format", "svg", "--json", "--manifest", "manifest.json"],
    );
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["written"], 0);
    assert_eq!(report["failed"], 1);
    assert_eq!(report["rows"][0]["status"], "failed");
    assert_eq!(report["rows"][0]["outputs"], serde_json::json!([]));
    let diagnostics = report["rows"][0]["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"] == "render.svg_rasterized")
    );
    let error = diagnostics
        .iter()
        .find(|d| d["code"] == "merge.row_failed")
        .unwrap();
    assert_eq!(error["severity"], "error");
    let message = error["message"].as_str().unwrap();
    assert!(message.contains("write error"), "{message}");
    assert!(message.contains("row-0001-page-2.svg"), "{message}");
    assert!(env.dir.path().join("out/row-0001-page-1.svg").is_file());
    assert!(env.dir.path().join("out/row-0001-page-2.svg").is_dir());
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(env.dir.path().join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["rows"], serde_json::json!([]));
}
