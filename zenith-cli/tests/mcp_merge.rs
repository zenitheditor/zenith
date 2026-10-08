//! MCP merge format parity and committed-output reporting.

#[path = "common/vector_capture.rs"]
mod vector_capture;

use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::TempDir;
use vector_capture::{svg_capture_dimensions, svg_without_capture_data};
use zenith_cli::commands::render::{BatchExportOptions, BatchFormat};

const DOC: &str = r##"zenith version=1 {
  project id="project" name="MCP Merge"
  tokens format="zenith-token-v1" {
    token id="ink" type="color" value="#333333"
    token id="shade" type="shadow" { layer dx=(px)1 dy=(px)2 blur=(px)3 color=(token)"ink"; }
  }
  styles {}
  document id="document" title="MCP Merge" {
    page id="first" w=(px)100 h=(px)80 {
      text id="label" x=(px)10 y=(px)10 w=(px)80 h=(px)40 fill=(token)"ink" role="data.title" { span "title"; }
    }
    page id="second" w=(px)100 h=(px)80 {
      rect id="card" x=(px)20 y=(px)20 w=(px)30 h=(px)30 fill=(token)"ink" shadow=(token)"shade"
    }
  }
}
"##;

struct Env {
    dir: TempDir,
    home: TempDir,
    store: TempDir,
}
impl Env {
    fn new(source: &str, csv: &str) -> Self {
        let env = Self {
            dir: TempDir::new().unwrap(),
            home: TempDir::new().unwrap(),
            store: TempDir::new().unwrap(),
        };
        fs::write(env.dir.path().join("doc.zen"), source).unwrap();
        fs::write(env.dir.path().join("data.csv"), csv).unwrap();
        env
    }
    fn arguments(&self, out: &str) -> Value {
        json!({"doc":self.dir.path().join("doc.zen"),"data":self.dir.path().join("data.csv"),"out_dir":self.dir.path().join(out)})
    }
    fn request(&self, request: Value) -> Value {
        let mut child = Command::new(env!("CARGO_BIN_EXE_zenith"))
            .arg("mcp")
            .env("HOME", self.home.path())
            .env("ZENITH_DATA_DIR", self.store.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        writeln!(child.stdin.take().unwrap(), "{request}").unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
    fn merge(&self, args: Value) -> Value {
        self.request(json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"zenith_merge","arguments":args}}))["result"].clone()
    }
}
fn structured(result: &Value) -> &Value {
    &result["structuredContent"]
}
fn mirror_matches(result: &Value) {
    let text: Value = serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text, result["structuredContent"]);
}

#[test]
fn omitted_format_keeps_png_bytes_and_legacy_summary_fields() {
    let csv = "title\nFirst\nSecond\n";
    let env = Env::new(DOC, csv);
    let result = env.merge(env.arguments("out"));
    assert_eq!(result["isError"], false, "{result}");
    let output = structured(&result);
    assert_eq!(output["total_rows"], 2);
    assert_eq!(output["written"], 2);
    assert_eq!(output["failed"], 0);
    assert_eq!(output["failures"], json!([]));
    assert_eq!(output["format"], "png");
    assert_eq!(output["files_written"], 4);
    assert!(output.get("diagnostics").is_none());
    assert!(output.get("raster_scale").is_none());
    let baseline = env.dir.path().join("baseline");
    let report = zenith_cli::commands::merge::run(DOC, csv, None, &baseline, None).unwrap();
    let paths: Vec<_> = report
        .written()
        .iter()
        .map(|name| env.dir.path().join("out").join(name).display().to_string())
        .collect();
    assert_eq!(output["outputs"], json!(paths));
    for name in report.written() {
        assert_eq!(
            fs::read(baseline.join(&name)).unwrap(),
            fs::read(env.dir.path().join("out").join(&name)).unwrap()
        );
    }
    mirror_matches(&result);
}

#[test]
fn svg_native_and_captured_outputs_match_shared_batch_options() {
    for captured in [false, true] {
        let source = if captured {
            DOC.to_owned()
        } else {
            DOC.replace(" shadow=(token)\"shade\"", "")
        };
        let csv = "title\nFirst\n";
        let env = Env::new(&source, csv);
        for scale in [1.0, 2.0] {
            let out = format!("out-{scale}");
            let baseline = env.dir.path().join(format!("baseline-{scale}"));
            let mut args = env.arguments(&out);
            args["format"] = json!("svg");
            args["raster_scale"] = json!(scale);
            args["manifest"] = json!(env.dir.path().join(format!("manifests/{scale}.json")));
            let result = env.merge(args);
            assert_eq!(result["isError"], false, "{result}");
            let report = zenith_cli::commands::merge::run_with_options(
                &source,
                csv,
                None,
                &baseline,
                None,
                BatchExportOptions {
                    format: BatchFormat::Svg,
                    raster_scale: scale,
                },
            )
            .unwrap();
            assert_eq!(structured(&result)["files_written"], 2);
            for name in report.written() {
                assert_eq!(
                    fs::read(baseline.join(&name)).unwrap(),
                    fs::read(env.dir.path().join(&out).join(&name)).unwrap()
                );
            }
            let manifest: Value = serde_json::from_slice(
                &fs::read(env.dir.path().join(format!("manifests/{scale}.json"))).unwrap(),
            )
            .unwrap();
            assert_eq!(
                manifest,
                serde_json::to_value(zenith_cli::commands::merge::build_manifest(
                    &source, csv, None, &report
                ))
                .unwrap()
            );
            let diagnostics = structured(&result)["rows"][0]["diagnostics"]
                .as_array()
                .unwrap();
            assert_eq!(
                diagnostics
                    .iter()
                    .any(|d| d["code"] == "render.svg_rasterized"),
                captured
            );
            if scale == 1.0 {
                assert!(structured(&result).get("raster_scale").is_none());
            } else {
                assert_eq!(structured(&result)["raster_scale"], 2.0);
            }
            mirror_matches(&result);
        }
        let first = fs::read_to_string(env.dir.path().join("out-1/row-0001-page-2.svg")).unwrap();
        let second = fs::read_to_string(env.dir.path().join("out-2/row-0001-page-2.svg")).unwrap();
        if captured {
            let dimensions = svg_capture_dimensions(&first);
            assert!(!dimensions.is_empty());
            assert_eq!(
                svg_capture_dimensions(&second),
                dimensions
                    .iter()
                    .map(|&(w, h)| (w * 2, h * 2))
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                svg_without_capture_data(&first),
                svg_without_capture_data(&second)
            );
        } else {
            assert_eq!(first, second);
        }
    }
}

#[test]
fn invalid_format_and_raster_scale_return_errors_before_output_writes() {
    let env = Env::new(DOC, "title\nFirst\n");
    let invalid = [
        json!({"format":"pdf"}),
        json!({"format":false}),
        json!({"format":null}),
        json!({"raster_scale":1}),
        json!({"format":"png","raster_scale":2}),
        json!({"format":"svg","raster_scale":0}),
        json!({"format":"svg","raster_scale":-1}),
        json!({"format":"svg","raster_scale":4.01}),
        json!({"format":"svg","raster_scale":"NaN"}),
        json!({"format":"svg","raster_scale":"inf"}),
        json!({"format":"svg","raster_scale":null}),
        json!({"format":"svg","raster_scale":true}),
    ];
    for extra in invalid {
        let mut args = env.arguments("out");
        args["manifest"] = json!(env.dir.path().join("manifests/output.json"));
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        let result = env.merge(args);
        assert_eq!(result["isError"], true, "{result}");
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("cli.invalid_argument")
        );
        assert!(!env.dir.path().join("out").exists());
        assert!(!env.dir.path().join("manifests").exists());
    }
}

#[test]
fn denied_fallback_retains_attributed_report_without_any_row_output() {
    let env = Env::new(DOC, "title\nFirst\n");
    fs::write(
        env.dir.path().join(".zenith.kdl"),
        "diagnostics { deny \"render.svg_rasterized\"; }",
    )
    .unwrap();
    let mut args = env.arguments("out");
    args["format"] = json!("svg");
    args["raster_scale"] = json!(2);
    let result = env.merge(args);
    assert_eq!(result["isError"], true, "{result}");
    let output = structured(&result);
    assert_eq!(output["written"], 0);
    assert_eq!(output["failed"], 1);
    assert_eq!(output["files_written"], 0);
    assert_eq!(output["outputs"], json!([]));
    assert_eq!(output["rows"][0]["row"], 0);
    assert!(
        output["rows"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "render.svg_rasterized" && d["severity"] == "error")
    );
    assert_eq!(fs::read_dir(env.dir.path().join("out")).unwrap().count(), 0);
    mirror_matches(&result);
}

#[test]
fn partial_page_write_reserves_name_and_records_committed_file_in_manifest() {
    let env = Env::new(DOC, "name,title\nsame,First\nsame,Second\n");
    fs::create_dir_all(env.dir.path().join("out/same-page-2.svg")).unwrap();
    let mut args = env.arguments("out");
    args["format"] = json!("svg");
    args["name_by"] = json!("name");
    args["manifest"] = json!(env.dir.path().join("manifest.json"));
    let result = env.merge(args);
    assert_eq!(result["isError"], true, "{result}");
    let output = structured(&result);
    assert_eq!(output["total_rows"], 2);
    assert_eq!(output["written"], 0);
    assert_eq!(output["failed"], 2);
    assert_eq!(output["files_written"], 1);
    assert_eq!(
        output["outputs"],
        json!([env
            .dir
            .path()
            .join("out")
            .join("same-page-1.svg")
            .display()
            .to_string()])
    );
    assert_eq!(output["rows"][0]["outputs"], json!(["same-page-1.svg"]));
    assert_eq!(output["rows"][1]["outputs"], json!([]));
    assert!(
        output["failures"][1]["error"]
            .as_str()
            .unwrap()
            .contains("collision: same-page-1.svg")
    );
    assert_eq!(output["failures"][0]["row"], 1);
    assert_eq!(output["failures"][1]["row"], 2);
    let manifest: Value =
        serde_json::from_slice(&fs::read(env.dir.path().join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["rows"],
        json!([{"row":0,"key":"same","outputs":["same-page-1.svg"],"status":"failed"}])
    );
    let baseline = env.dir.path().join("baseline");
    zenith_cli::commands::merge::run_with_format(
        DOC,
        "name,title\nsame,First\n",
        None,
        &baseline,
        Some("name"),
        BatchFormat::Svg,
    )
    .unwrap();
    assert_eq!(
        fs::read(env.dir.path().join("out/same-page-1.svg")).unwrap(),
        fs::read(baseline.join("same-page-1.svg")).unwrap()
    );
    assert_eq!(fs::read_dir(env.dir.path().join("out")).unwrap().count(), 2);
    mirror_matches(&result);
}

#[test]
fn protected_manifest_error_keeps_previous_bytes_and_successful_row_counts() {
    let env = Env::new(DOC, "title\nFirst\n");
    let manifest = env.dir.path().join("manifest.json");
    fs::write(&manifest, "previous manifest").unwrap();
    let original_permissions = fs::metadata(&manifest).unwrap().permissions();
    let mut readonly = original_permissions.clone();
    readonly.set_readonly(true);
    fs::set_permissions(&manifest, readonly).unwrap();
    let mut args = env.arguments("out");
    args["format"] = json!("svg");
    args["manifest"] = json!(manifest);
    let result = env.merge(args);
    fs::set_permissions(&manifest, original_permissions).unwrap();
    assert_eq!(result["isError"], true, "{result}");
    let output = structured(&result);
    assert_eq!(output["written"], 1);
    assert_eq!(output["failed"], 0);
    assert_eq!(output["failures"], json!([]));
    assert_eq!(output["files_written"], 2);
    assert_eq!(output["diagnostics"][0]["code"], "io.write_failed");
    assert_eq!(fs::read_to_string(manifest).unwrap(), "previous manifest");
    assert_eq!(fs::read_dir(env.dir.path()).unwrap().count(), 4);
    mirror_matches(&result);
}

#[test]
fn merge_schema_exposes_optional_format_and_svg_raster_scale_bounds() {
    let env = Env::new(DOC, "title\nFirst\n");
    let response = env.request(json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}));
    let tool = response["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "zenith_merge")
        .unwrap();
    let schema = &tool["inputSchema"];
    assert_eq!(
        schema["properties"]["format"]["enum"],
        json!(["png", "svg"])
    );
    assert_eq!(schema["properties"]["format"]["default"], "png");
    assert_eq!(schema["properties"]["raster_scale"]["exclusiveMinimum"], 0);
    assert_eq!(schema["properties"]["raster_scale"]["maximum"], 4);
    assert!(
        schema["properties"]["raster_scale"]["description"]
            .as_str()
            .unwrap()
            .contains("svg only")
    );
    assert_eq!(schema["required"], json!(["doc", "data", "out_dir"]));
}

#[test]
fn manifests_cannot_replace_inputs_or_planned_pages() {
    for csv in ["title\nFirst\n", "title\n"] {
        for manifest in ["doc.zen", "data.csv", "missing/row-0001-page-1.svg"] {
            if csv == "title\n" && manifest.starts_with("missing") {
                continue;
            }
            let env = Env::new(DOC, csv);
            let mut args = env.arguments("missing");
            args["format"] = json!("svg");
            args["manifest"] = json!(env.dir.path().join(manifest));
            let result = env.merge(args);
            assert_eq!(result["isError"], true, "{result}");
            assert!(!env.dir.path().join("missing").exists());
            assert_eq!(
                fs::read_to_string(env.dir.path().join("doc.zen")).unwrap(),
                DOC
            );
            assert_eq!(
                fs::read_to_string(env.dir.path().join("data.csv")).unwrap(),
                csv
            );
        }
    }
}
