use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

struct Env {
    dir: TempDir,
    store: TempDir,
}

impl Env {
    fn new(src: &str) -> Self {
        let env = Self {
            dir: tempfile::tempdir().expect("document directory"),
            store: tempfile::tempdir().expect("store directory"),
        };
        fs::write(env.dir.path().join("doc.zen"), src).expect("write document");
        env
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_zenith"))
            .args(["render", "doc.zen"])
            .args(args)
            .arg("--json")
            .current_dir(self.dir.path())
            .env("ZENITH_DATA_DIR", self.store.path())
            .output()
            .expect("run render")
    }

    fn read(&self, name: &str) -> Vec<u8> {
        fs::read(self.dir.path().join(name)).expect("read output")
    }
}

fn document(pages: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.svg" name="SVG"
  tokens format="zenith-token-v1" {{
    token id="color.red" type="color" value="#ff0000"
    token id="color.blue" type="color" value="#0000ff"
  }}
  styles {{}}
  document id="doc.svg" title="SVG" {{
{pages}
  }}
}}
"##
    )
}

const PAGES: &str = r#"    page id="page.a" w=(px)120 h=(px)80 {
      rect id="rect.a" x=(px)10 y=(px)10 w=(px)30 h=(px)20 fill=(token)"color.red"
    }
    page id="page.b" w=(px)140 h=(px)90 {
      rect id="rect.b" x=(px)20 y=(px)20 w=(px)40 h=(px)30 fill=(token)"color.blue"
    }"#;

fn json(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|error| {
        panic!(
            "JSON error: {error}; stdout: {}; stderr: {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    })
}

fn success(out: &Output) -> Value {
    assert!(
        out.status.success(),
        "stdout: {}; stderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    json(out)
}

#[test]
fn svg_default_page_matches_selected_page_and_batch_bytes() {
    let env = Env::new(&document(PAGES));
    let default = success(&env.run(&["--svg", "default.svg"]));
    success(&env.run(&["--svg", "first.svg", "--page", "1"]));
    success(&env.run(&["--svg", "second.svg", "--page", "2"]));
    let batch = success(&env.run(&["--all-pages-svg", "pages"]));
    success(&env.run(&["--all-pages-svg", "again"]));
    assert_eq!(env.read("default.svg"), env.read("first.svg"));
    assert_ne!(env.read("first.svg"), env.read("second.svg"));
    for (page, single) in [(1, "first.svg"), (2, "second.svg")] {
        assert_eq!(
            env.read(single),
            env.read(&format!("pages/page-{page}.svg"))
        );
        assert_eq!(
            env.read(single),
            env.read(&format!("again/page-{page}.svg"))
        );
    }
    assert_eq!(
        batch["outputs"],
        serde_json::json!([
            Path::new("pages").join("page-1.svg").display().to_string(),
            Path::new("pages").join("page-2.svg").display().to_string()
        ])
    );
    assert_eq!(batch["diagnostics"], default["diagnostics"]);
    assert_eq!(default["schema"], "zenith-render-v1");
    assert!(
        !default["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|d| d["code"] == "render.svg_rasterized")
    );
    let svg = String::from_utf8(env.read("default.svg")).expect("SVG UTF-8");
    assert!(svg.contains("<svg") && svg.contains("<path"), "{svg}");
    assert!(
        !svg.contains("data:image/png"),
        "plain geometry remains vector"
    );
}

#[test]
fn combined_outputs_keep_scaled_png_and_unscaled_svg_dimensions() {
    let env = Env::new(&document(PAGES));
    let out = success(&env.run(&[
        "--png", "out.png", "--pdf", "out.pdf", "--svg", "out.svg", "--scale", "0.5",
    ]));
    let outputs = out["outputs"].as_array().expect("outputs");
    for name in ["out.png", "out.pdf", "out.svg"] {
        assert!(outputs.contains(&serde_json::json!(name)), "{out}");
    }
    let png = env.read("out.png");
    assert_eq!(
        u32::from_be_bytes(png[16..20].try_into().expect("width")),
        60
    );
    assert_eq!(
        u32::from_be_bytes(png[20..24].try_into().expect("height")),
        40
    );
    let image = out["images"]
        .as_array()
        .expect("images")
        .iter()
        .find(|v| v["kind"] == "png")
        .expect("PNG metadata");
    assert_eq!(image["width"], 60);
    assert_eq!(image["height"], 40);
    assert_eq!(image["scale"], 0.5);
    assert!(env.read("out.pdf").starts_with(b"%PDF-"));
    success(&env.run(&["--svg", "unscaled.svg"]));
    assert_eq!(env.read("out.svg"), env.read("unscaled.svg"));
}

#[test]
fn png_and_svg_batches_keep_distinct_extensions_and_page_order() {
    let env = Env::new(&document(PAGES));
    success(&env.run(&["--all-pages", "png", "--all-pages-svg", "svg"]));
    for page in 1..=2 {
        assert!(
            env.read(&format!("png/page-{page}.png"))
                .starts_with(b"\x89PNG")
        );
        let svg = String::from_utf8(env.read(&format!("svg/page-{page}.svg"))).expect("SVG UTF-8");
        assert!(svg.contains("<svg"));
    }
    assert!(!env.dir.path().join("png/page-1.svg").exists());
    assert!(!env.dir.path().join("svg/page-1.png").exists());
}

#[test]
fn svg_scale_and_out_of_range_pages_return_errors_without_writes() {
    for args in [
        vec!["--svg", "out.svg", "--scale", "0.5"],
        vec!["--all-pages-svg", "pages", "--scale", "0.5"],
        vec!["--svg", "out.svg", "--page", "0"],
        vec!["--svg", "out.svg", "--page", "3"],
    ] {
        let env = Env::new(&document(PAGES));
        let out = env.run(&args);
        assert!(
            !out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stdout)
        );
        assert!(!env.dir.path().join("out.svg").exists());
        assert!(!env.dir.path().join("pages").exists());
        assert!(
            !json(&out)["diagnostics"]
                .as_array()
                .expect("diagnostics")
                .is_empty()
        );
    }
}

#[test]
fn last_page_font_policy_blocks_every_requested_output() {
    let last = r#"    page id="page.last" w=(px)120 h=(px)80 {
      text id="text.last" x=(px)0 y=(px)0 w=(px)100 h=(px)60 font-family=(token)"font.missing" {
        span "Last page"
      }
    }"#;
    let src = document(&format!("{PAGES}\n{last}")).replace(
        r#"token id="color.red""#,
        r#"token id="font.missing" type="fontFamily" value="Missing SVG Font"
    token id="color.red""#,
    );
    let env = Env::new(&src);
    let out = env.run(&[
        "--svg",
        "out.svg",
        "--png",
        "out.png",
        "--all-pages-svg",
        "pages",
        "--deny",
        "font.unresolved",
    ]);
    assert_eq!(out.status.code(), Some(2));
    let value = json(&out);
    assert_eq!(value["status"], "blocked");
    assert!(
        value["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|d| d["code"] == "font.unresolved" && d["severity"] == "error")
    );
    for name in ["out.svg", "out.png", "pages"] {
        assert!(!env.dir.path().join(name).exists(), "blocked output {name}");
    }
}

#[test]
fn fallback_advisory_reports_reason_and_policy_blocks_writes() {
    let src = fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/shadow.zen"),
    )
    .expect("shadow fixture");
    let env = Env::new(&src);
    let value = success(&env.run(&["--svg", "shadow.svg"]));
    let diagnostic = value["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .find(|d| d["code"] == "render.svg_rasterized")
        .expect("fallback diagnostic");
    assert_eq!(diagnostic["severity"], "advisory");
    assert!(
        diagnostic["message"]
            .as_str()
            .expect("message")
            .to_lowercase()
            .contains("effect"),
        "{diagnostic}"
    );
    let svg = String::from_utf8(env.read("shadow.svg")).expect("SVG UTF-8");
    assert!(svg.contains("data:image/png;base64,"));
    let out = env.run(&[
        "--svg",
        "blocked.svg",
        "--png",
        "blocked.png",
        "--deny",
        "render.svg_rasterized",
    ]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(json(&out)["status"], "blocked");
    assert!(!env.dir.path().join("blocked.svg").exists());
    assert!(!env.dir.path().join("blocked.png").exists());
    let allowed = success(&env.run(&["--svg", "allowed.svg", "--allow", "render.svg_rasterized"]));
    assert!(
        !allowed["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|d| d["code"] == "render.svg_rasterized")
    );
    assert_eq!(env.read("allowed.svg"), env.read("shadow.svg"));
}

#[test]
fn svg_write_error_reports_only_outputs_already_written() {
    let env = Env::new(&document(PAGES));
    fs::create_dir(env.dir.path().join("out.svg")).expect("create SVG destination directory");
    let out = env.run(&["--png", "out.png", "--svg", "out.svg"]);
    assert_eq!(out.status.code(), Some(2));
    let value = json(&out);
    assert_eq!(value["schema"], "zenith-render-v1");
    assert_eq!(value["status"], "blocked");
    assert!(
        value["diagnostics"]
            .as_array()
            .expect("diagnostics")
            .iter()
            .any(|diagnostic| diagnostic["code"] == "io.write_failed"
                && diagnostic["severity"] == "error")
    );
    assert_eq!(value["outputs"], serde_json::json!(["out.png"]));
    let images = value["images"].as_array().expect("image metadata");
    assert_eq!(images.len(), 1);
    assert_eq!(images[0]["kind"], "png");
    assert_eq!(images[0]["path"], "out.png");
    assert_eq!(images[0]["width"], 120);
    assert_eq!(images[0]["height"], 80);
    assert!(env.read("out.png").starts_with(b"\x89PNG"));
    assert!(env.dir.path().join("out.svg").is_dir());
    assert_eq!(
        fs::read_dir(env.dir.path().join("out.svg"))
            .expect("SVG directory")
            .count(),
        0
    );
}
