//! Per-file output replacement and partial batch reporting.

use serde_json::Value;
use std::fs;
use std::process::{Command, Output};
use tempfile::TempDir;

const DOC: &str = r##"zenith version=1 {
  project id="project" name="Writes"
  tokens format="zenith-token-v1" {
    token id="ink" type="color" value="#333333"
    token id="shade" type="shadow" { layer dx=(px)1 dy=(px)2 blur=(px)3 color=(token)"ink"; }
  }
  styles {}
  document id="document" title="Writes" {
    page id="first" w=(px)100 h=(px)80 {
      text id="label" x=(px)10 y=(px)10 w=(px)80 h=(px)40 fill=(token)"ink" role="data.title" { span "title"; }
    }
    page id="second" w=(px)100 h=(px)80 {
      rect id="card" x=(px)20 y=(px)20 w=(px)30 h=(px)30 fill=(token)"ink" shadow=(token)"shade"
    }
  }
  variants { variant id="square" source="second" w=(px)100 h=(px)80 {} }
}
"##;

struct Env {
    dir: TempDir,
    home: TempDir,
}
impl Env {
    fn new() -> Self {
        let env = Self {
            dir: TempDir::new().unwrap(),
            home: TempDir::new().unwrap(),
        };
        fs::write(env.dir.path().join("doc.zen"), DOC).unwrap();
        fs::write(
            env.dir.path().join("data.csv"),
            "name,title\nsame,First\nsame,Second\n",
        )
        .unwrap();
        env
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_zenith"))
            .args(args)
            .current_dir(self.dir.path())
            .env("HOME", self.home.path())
            .env("ZENITH_DATA_DIR", self.dir.path().join("store"))
            .output()
            .unwrap()
    }
    fn json(&self, args: &[&str]) -> (Output, Value) {
        let output = self.run(args);
        let report = serde_json::from_slice(&output.stdout).unwrap();
        (output, report)
    }
}

#[test]
fn merge_partial_row_reserves_committed_name_and_enters_manifest() {
    let env = Env::new();
    fs::create_dir_all(env.dir.path().join("out/same-page-2.svg")).unwrap();
    fs::write(
        env.dir.path().join("out/same-page-2.svg/keep"),
        "directory contents",
    )
    .unwrap();
    let (output, report) = env.json(&[
        "merge",
        "doc.zen",
        "data.csv",
        "--out-dir",
        "out",
        "--name-by",
        "name",
        "--format",
        "svg",
        "--manifest",
        "manifest.json",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(report["written"], 0);
    assert_eq!(report["failed"], 2);
    assert_eq!(
        report["rows"][0]["outputs"],
        serde_json::json!(["same-page-1.svg"])
    );
    assert_eq!(report["rows"][1]["outputs"], serde_json::json!([]));
    assert!(
        report["rows"][1]["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("collision: same-page-1.svg")
    );
    let baseline = env.dir.path().join("baseline");
    zenith_cli::commands::merge::run_with_format(
        DOC,
        "name,title\nsame,First\n",
        None,
        &baseline,
        Some("name"),
        zenith_cli::commands::render::BatchFormat::Svg,
    )
    .unwrap();
    assert_eq!(
        fs::read(env.dir.path().join("out/same-page-1.svg")).unwrap(),
        fs::read(baseline.join("same-page-1.svg")).unwrap()
    );
    assert_eq!(
        fs::read_to_string(env.dir.path().join("out/same-page-2.svg/keep")).unwrap(),
        "directory contents"
    );
    let manifest: Value =
        serde_json::from_slice(&fs::read(env.dir.path().join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["rows"],
        serde_json::json!([{"row":0,"key":"same","outputs":["same-page-1.svg"],"status":"failed"}])
    );
    assert_eq!(fs::read_dir(env.dir.path().join("out")).unwrap().count(), 2);
}

#[test]
fn variant_image_error_retains_committed_source_in_report_and_manifest() {
    for format in ["png", "svg"] {
        let env = Env::new();
        let image = env.dir.path().join(format!("out/doc-square.{format}"));
        fs::create_dir_all(&image).unwrap();
        let source = env.dir.path().join("out/doc-square.zen");
        fs::write(&source, "previous source").unwrap();
        let (output, report) = env.json(&[
            "variant",
            "doc.zen",
            "--out-dir",
            "out",
            "--format",
            format,
            "--manifest",
            "manifest.json",
            "--json",
        ]);
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(report["generated"], 0);
        assert_eq!(report["failed"], 1);
        assert_eq!(report["variants"][0]["outputs_zen"], "doc-square.zen");
        assert!(report["variants"][0].get("outputs_png").is_none());
        assert!(report["variants"][0].get("outputs_svg").is_none());
        let bytes = fs::read(&source).unwrap();
        assert_ne!(bytes, b"previous source");
        use zenith_core::KdlSource;
        zenith_core::KdlAdapter.parse(&bytes).unwrap();
        let manifest: Value =
            serde_json::from_slice(&fs::read(env.dir.path().join("manifest.json")).unwrap())
                .unwrap();
        assert_eq!(
            manifest["targets"],
            serde_json::json!([{"id":"square","source":"second","outputs_zen":"doc-square.zen","status":"failed"}])
        );
        assert!(image.is_dir());
        assert_eq!(fs::read_dir(env.dir.path().join("out")).unwrap().count(), 2);
    }
}

#[test]
fn variant_encoding_error_preserves_existing_source_and_image() {
    let env = Env::new();
    let doc = DOC.replace("  styles {}", "  assets { asset id=\"bad\" kind=\"image\" src=\"bad.png\"; }\n  styles {}").replace("      rect id=\"card\"", "      image id=\"image\" asset=\"bad\" x=(px)0 y=(px)0 w=(px)20 h=(px)20\n      rect id=\"card\"");
    fs::write(env.dir.path().join("doc.zen"), doc).unwrap();
    fs::write(env.dir.path().join("bad.png"), "invalid image").unwrap();
    fs::create_dir(env.dir.path().join("out")).unwrap();
    for name in ["doc-square.zen", "doc-square.svg"] {
        fs::write(env.dir.path().join("out").join(name), "previous output").unwrap();
    }
    let (output, report) = env.json(&[
        "variant",
        "doc.zen",
        "--out-dir",
        "out",
        "--format",
        "svg",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(report["variants"][0].get("outputs_zen").is_none());
    for name in ["doc-square.zen", "doc-square.svg"] {
        assert_eq!(
            fs::read_to_string(env.dir.path().join("out").join(name)).unwrap(),
            "previous output"
        );
    }
}

#[test]
fn manifest_error_keeps_batch_results_and_committed_files() {
    for command in ["merge", "variant"] {
        let env = Env::new();
        fs::write(env.dir.path().join("data.csv"), "name,title\none,First\n").unwrap();
        fs::create_dir(env.dir.path().join("manifest.json")).unwrap();
        fs::write(
            env.dir.path().join("manifest.json/keep"),
            "existing contents",
        )
        .unwrap();
        let mut args = vec![command, "doc.zen"];
        if command == "merge" {
            args.push("data.csv");
        }
        args.extend([
            "--out-dir",
            "out",
            "--format",
            "svg",
            "--manifest",
            "manifest.json",
            "--json",
        ]);
        let (output, report) = env.json(&args);
        assert_eq!(output.status.code(), Some(2));
        assert_eq!(
            report["schema"],
            if command == "merge" {
                "zenith-merge-v1"
            } else {
                "zenith-variant-v1"
            }
        );
        assert_eq!(
            report[if command == "merge" {
                "written"
            } else {
                "generated"
            }],
            1
        );
        assert_eq!(report["failed"], 0);
        assert_eq!(report["diagnostics"][0]["code"], "io.write_failed");
        assert_eq!(report["diagnostics"][0]["severity"], "error");
        if command == "merge" {
            assert_eq!(
                report["rows"][0]["outputs"],
                serde_json::json!(["row-0001-page-1.svg", "row-0001-page-2.svg"])
            );
        } else {
            assert_eq!(report["variants"][0]["outputs_zen"], "doc-square.zen");
            assert_eq!(report["variants"][0]["outputs_svg"], "doc-square.svg");
        }
        assert_eq!(fs::read_dir(env.dir.path().join("out")).unwrap().count(), 2);
        assert_eq!(
            fs::read_to_string(env.dir.path().join("manifest.json/keep")).unwrap(),
            "existing contents"
        );
    }
}

#[test]
fn human_batch_counts_committed_files_on_partial_errors() {
    let env = Env::new();
    fs::create_dir_all(env.dir.path().join("out/same-page-2.svg")).unwrap();
    let merge = env.run(&[
        "merge",
        "doc.zen",
        "data.csv",
        "--out-dir",
        "out",
        "--name-by",
        "name",
        "--format",
        "svg",
    ]);
    assert_eq!(merge.status.code(), Some(1));
    assert!(
        String::from_utf8(merge.stdout)
            .unwrap()
            .contains("wrote 1 file(s)")
    );
    fs::create_dir_all(env.dir.path().join("variants/doc-square.svg")).unwrap();
    let variant = env.run(&[
        "variant",
        "doc.zen",
        "--out-dir",
        "variants",
        "--format",
        "svg",
    ]);
    assert_eq!(variant.status.code(), Some(1));
    let stdout = String::from_utf8(variant.stdout).unwrap();
    assert!(stdout.contains("generated 0 variant(s)"));
    assert!(stdout.contains("wrote 1 file(s)"));
}

#[test]
fn human_render_error_lists_committed_paths_and_keeps_directory_conflict() {
    let env = Env::new();
    fs::create_dir(env.dir.path().join("blocked.svg")).unwrap();
    let output = env.run(&[
        "render",
        "doc.zen",
        "--png",
        "committed.png",
        "--svg",
        "blocked.svg",
    ]);
    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("written: committed.png"), "{stdout}");
    assert!(!stdout.contains("written: blocked.svg"));
    assert!(env.dir.path().join("committed.png").is_file());
    assert!(env.dir.path().join("blocked.svg").is_dir());
}
