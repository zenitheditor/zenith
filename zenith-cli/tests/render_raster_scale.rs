//! Vector fallback resolution across render and batch entry points.

#[path = "common/vector_capture.rs"]
mod vector_capture;

use serde_json::Value;
use std::fs;
use std::process::{Command, Output};
use tempfile::TempDir;
use vector_capture::{svg_capture_dimensions, svg_without_capture_data};
use zenith_cli::commands::render::{BatchExportOptions, BatchFormat};

const DOC: &str = r##"zenith version=1 {
  project id="project" name="Capture"
  tokens format="zenith-token-v1" {
    token id="ink" type="color" value="#333333"
    token id="shade" type="shadow" { layer dx=(px)1 dy=(px)2 blur=(px)3 color=(token)"ink"; }
  }
  styles {}
  document id="document" title="Capture" {
    page id="page" w=(px)100 h=(px)80 {
      rect id="card" x=(px)20 y=(px)20 w=(px)30 h=(px)30 fill=(token)"ink" shadow=(token)"shade"
      text id="label" x=(px)10 y=(px)55 w=(px)80 h=(px)20 fill=(token)"ink" role="data.name" { span "name"; }
    }
  }
  variants { variant id="square" source="page" w=(px)100 h=(px)80 {} }
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
        fs::write(env.dir.path().join("data.csv"), "name\nAlice\n").unwrap();
        env
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_zenith"))
            .args(args)
            .arg("--json")
            .current_dir(self.dir.path())
            .env("HOME", self.home.path())
            .env("ZENITH_DATA_DIR", self.dir.path().join("store"))
            .output()
            .unwrap()
    }
    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.dir.path().join(name)).unwrap()
    }
}
fn success(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn svg_scale_doubles_capture_pixels_and_preserves_vector_geometry() {
    let env = Env::new();
    let default = success(env.run(&["render", "doc.zen", "--svg", "one.svg"]));
    let doubled = success(env.run(&[
        "render",
        "doc.zen",
        "--svg",
        "two.svg",
        "--raster-scale",
        "2",
        "--allow",
        "render.svg_rasterized",
    ]));
    success(env.run(&[
        "render",
        "doc.zen",
        "--svg",
        "explicit.svg",
        "--raster-scale",
        "1",
    ]));
    let first = env.read("one.svg");
    assert_eq!(first, env.read("explicit.svg"));
    let second = env.read("two.svg");
    let dimensions = svg_capture_dimensions(&first);
    assert!(!dimensions.is_empty());
    assert_eq!(
        svg_capture_dimensions(&second),
        dimensions
            .iter()
            .map(|&(w, h)| (2 * w, 2 * h))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        svg_without_capture_data(&first),
        svg_without_capture_data(&second)
    );
    assert!(first.contains("viewBox=\"0 0 100 80\""));
    assert!(
        default["rasterized_regions"][0]
            .get("raster_scale")
            .is_none()
    );
    assert_eq!(doubled["rasterized_regions"][0]["raster_scale"], 2.0);
    assert_eq!(doubled["rasterized_regions"][0]["format"], "svg");
    assert!(
        !doubled["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "render.svg_rasterized")
    );
    success(env.run(&[
        "render",
        "doc.zen",
        "--all-pages-svg",
        "pages",
        "--raster-scale",
        "2",
    ]));
    assert_eq!(second, env.read("pages/page-1.svg"));
}

#[test]
fn mixed_png_scale_and_vector_raster_scale_are_independent() {
    let env = Env::new();
    success(env.run(&["render", "doc.zen", "--png", "one.png", "--scale", "0.5"]));
    success(env.run(&[
        "render",
        "doc.zen",
        "--svg",
        "one.svg",
        "--raster-scale",
        "2",
    ]));
    let report = success(env.run(&[
        "render",
        "doc.zen",
        "--png",
        "mixed.png",
        "--svg",
        "mixed.svg",
        "--pdf",
        "mixed.pdf",
        "--scale",
        "0.5",
        "--raster-scale",
        "2",
    ]));
    assert_eq!(
        fs::read(env.dir.path().join("one.png")).unwrap(),
        fs::read(env.dir.path().join("mixed.png")).unwrap()
    );
    assert_eq!(env.read("one.svg"), env.read("mixed.svg"));
    assert_eq!(report["images"][0]["width"], 50);
    assert_eq!(report["images"][0]["height"], 40);
    assert_eq!(report["images"][0]["scale"], 0.5);
    assert!(
        report["rasterized_regions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["raster_scale"] == 2.0)
    );
}

#[test]
fn invalid_or_nonvector_raster_scale_writes_nothing() {
    for bad in ["0", "-1", "4.01", "NaN", "inf", "text"] {
        let env = Env::new();
        let output = env.run(&[
            "render",
            "doc.zen",
            "--png",
            "out.png",
            "--svg",
            "out.svg",
            "--pdf",
            "out.pdf",
            "--raster-scale",
            bad,
        ]);
        assert_eq!(output.status.code(), Some(2), "{bad}");
        for name in ["out.png", "out.svg", "out.pdf"] {
            assert!(!env.dir.path().join(name).exists());
        }
    }
    for args in [
        vec![
            "render",
            "doc.zen",
            "--png",
            "out.png",
            "--raster-scale",
            "1",
        ],
        vec![
            "render",
            "doc.zen",
            "--scene",
            "out.json",
            "--raster-scale",
            "2",
        ],
        vec![
            "merge",
            "doc.zen",
            "data.csv",
            "--out-dir",
            "out",
            "--raster-scale",
            "1",
        ],
        vec![
            "variant",
            "doc.zen",
            "--out-dir",
            "out",
            "--raster-scale",
            "2",
        ],
    ] {
        let env = Env::new();
        assert_eq!(env.run(&args).status.code(), Some(2));
        for name in ["out.png", "out.json", "out"] {
            assert!(!env.dir.path().join(name).exists());
        }
    }
}

#[test]
fn batch_svg_uses_raster_scale_and_format_wrappers_keep_default_bytes() {
    let env = Env::new();
    for command in ["merge", "variant"] {
        let mut args = vec![command, "doc.zen"];
        if command == "merge" {
            args.push("data.csv");
        }
        args.extend([
            "--out-dir",
            command,
            "--format",
            "svg",
            "--raster-scale",
            "2",
        ]);
        success(env.run(&args));
    }
    let merge = zenith_cli::commands::merge::run_with_format(
        DOC,
        "name\nAlice\n",
        None,
        &env.dir.path().join("default_merge"),
        None,
        BatchFormat::Svg,
    )
    .unwrap();
    let variant = zenith_cli::commands::variant::run_variant_with_format(
        DOC,
        None,
        &env.dir.path().join("default_variant"),
        "doc",
        BatchFormat::Svg,
    )
    .unwrap();
    assert!(merge.failed().is_empty());
    assert_eq!(variant.generated(), 1);
    let options = BatchExportOptions {
        format: BatchFormat::Svg,
        raster_scale: 1.0,
    };
    zenith_cli::commands::merge::run_with_options(
        DOC,
        "name\nAlice\n",
        None,
        &env.dir.path().join("options_merge"),
        None,
        options,
    )
    .unwrap();
    zenith_cli::commands::variant::run_variant_with_options(
        DOC,
        None,
        &env.dir.path().join("options_variant"),
        "doc",
        options,
    )
    .unwrap();
    for (command, file) in [("merge", "row-0001.svg"), ("variant", "doc-square.svg")] {
        let original = env.read(&format!("default_{command}/{file}"));
        assert_eq!(original, env.read(&format!("options_{command}/{file}")));
        let doubled = env.read(&format!("{command}/{file}"));
        assert_eq!(
            svg_capture_dimensions(&doubled),
            svg_capture_dimensions(&original)
                .iter()
                .map(|&(w, h)| (2 * w, 2 * h))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            svg_without_capture_data(&original),
            svg_without_capture_data(&doubled)
        );
    }
    for command in ["merge", "variant"] {
        for bad in ["0", "-1", "NaN", "inf", "5"] {
            let mut args = vec![command, "doc.zen"];
            if command == "merge" {
                args.push("data.csv");
            }
            args.extend(["--out-dir", "bad", "--format", "svg", "--raster-scale", bad]);
            assert_eq!(env.run(&args).status.code(), Some(2));
            assert!(!env.dir.path().join("bad").exists());
        }
    }
}

fn pdf_image_dimensions(pdf: &[u8]) -> Vec<(u32, u32)> {
    String::from_utf8_lossy(pdf)
        .split("/Subtype /Image")
        .skip(1)
        .map(|dictionary| {
            let header = dictionary.split("stream").next().unwrap();
            let words: Vec<_> = header.split_whitespace().collect();
            let dimension = |key| {
                words.windows(2).find(|pair| pair[0] == key).unwrap()[1]
                    .parse::<u32>()
                    .unwrap()
            };
            (dimension("/Width"), dimension("/Height"))
        })
        .collect()
}

#[test]
fn pdf_scale_doubles_capture_pixels_and_preserves_page_size() {
    let env = Env::new();
    let integer_bounds = DOC
        .lines()
        .filter(|line| !line.contains("text id="))
        .collect::<Vec<_>>()
        .join("\n")
        .replace(" shadow=(token)\"shade\"", " blend-mode=\"multiply\"");
    fs::write(env.dir.path().join("doc.zen"), integer_bounds).unwrap();
    let default = success(env.run(&["render", "doc.zen", "--pdf", "one.pdf"]));
    success(env.run(&[
        "render",
        "doc.zen",
        "--pdf",
        "explicit.pdf",
        "--raster-scale",
        "1",
    ]));
    let doubled = success(env.run(&[
        "render",
        "doc.zen",
        "--pdf",
        "two.pdf",
        "--raster-scale",
        "2",
    ]));
    let first = fs::read(env.dir.path().join("one.pdf")).unwrap();
    assert_eq!(
        first,
        fs::read(env.dir.path().join("explicit.pdf")).unwrap()
    );
    let second = fs::read(env.dir.path().join("two.pdf")).unwrap();
    let dimensions = pdf_image_dimensions(&first);
    assert!(!dimensions.is_empty());
    assert_eq!(
        pdf_image_dimensions(&second),
        dimensions
            .iter()
            .map(|&(w, h)| (2 * w, 2 * h))
            .collect::<Vec<_>>()
    );
    for bytes in [&first, &second] {
        assert!(String::from_utf8_lossy(bytes).contains("/MediaBox [0 0 100 80]"));
    }
    assert!(
        default["rasterized_regions"][0]
            .get("raster_scale")
            .is_none()
    );
    assert_eq!(doubled["rasterized_regions"][0]["raster_scale"], 2.0);
}

#[test]
fn native_vector_output_keeps_default_bytes_at_higher_raster_scale() {
    let env = Env::new();
    fs::write(
        env.dir.path().join("doc.zen"),
        DOC.replace(" shadow=(token)\"shade\"", ""),
    )
    .unwrap();
    success(env.run(&[
        "render",
        "doc.zen",
        "--svg",
        "native.svg",
        "--pdf",
        "native.pdf",
    ]));
    let report = success(env.run(&[
        "render",
        "doc.zen",
        "--svg",
        "scaled.svg",
        "--pdf",
        "scaled.pdf",
        "--raster-scale",
        "2",
    ]));
    assert!(report.get("rasterized_regions").is_none());
    for extension in ["svg", "pdf"] {
        assert_eq!(
            fs::read(env.dir.path().join(format!("native.{extension}"))).unwrap(),
            fs::read(env.dir.path().join(format!("scaled.{extension}"))).unwrap()
        );
    }
}

#[test]
fn vector_capture_metadata_lists_only_written_paths_after_write_error() {
    let env = Env::new();
    fs::create_dir(env.dir.path().join("blocked.svg")).unwrap();
    let output = env.run(&[
        "render",
        "doc.zen",
        "--pdf",
        "written.pdf",
        "--svg",
        "blocked.svg",
        "--raster-scale",
        "2",
    ]);
    assert_eq!(output.status.code(), Some(2));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let path = "written.pdf";
    assert_eq!(report["outputs"], serde_json::json!([path]));
    let regions = report["rasterized_regions"].as_array().unwrap();
    assert!(!regions.is_empty());
    assert!(
        regions
            .iter()
            .all(|r| r["path"] == path && r["format"] == "pdf" && r["raster_scale"] == 2.0)
    );
}
