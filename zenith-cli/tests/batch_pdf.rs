//! Batch PDF output for `merge` and `variant`, and `merge --combine`.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

/// Two pages: a data-bound label, and a shadowed card that PDF rasterizes.
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

/// One A4 page with a 3 mm bleed and a data-bound name.
const A4: &str = r##"zenith version=1 {
  project id="project" name="Badge"
  tokens format="zenith-token-v1" {
    token id="ink" type="color" value="#1e293b"
    token id="paper" type="color" value="#ffffff"
    token id="size.name" type="dimension" value=(px)72
  }
  styles {}
  document id="document" title="Badge" {
    page id="front" w=(px)794 h=(px)1123 background=(token)"paper" bleed=(pt)8.504 {
      text id="name" x=(px)60 y=(px)400 w=(px)674 h=(px)120 font-size=(token)"size.name" fill=(token)"ink" role="data.name" { span "Name"; }
    }
  }
}
"##;

struct Env {
    dir: TempDir,
    home: TempDir,
}

impl Env {
    fn new(doc: &str, csv: &str) -> Self {
        let env = Self {
            dir: TempDir::new().unwrap(),
            home: TempDir::new().unwrap(),
        };
        fs::write(env.dir.path().join("doc.zen"), doc).unwrap();
        fs::write(env.dir.path().join("data.csv"), csv).unwrap();
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

    fn read(&self, name: &str) -> Vec<u8> {
        fs::read(self.dir.path().join(name)).unwrap()
    }

    fn json(&self, name: &str) -> serde_json::Value {
        serde_json::from_slice(&self.read(name)).unwrap()
    }
}

fn report(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "{e}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn text(pdf: &[u8]) -> String {
    String::from_utf8_lossy(pdf).into_owned()
}

/// The page count from the page tree `/Count`.
fn page_count(pdf: &[u8]) -> usize {
    let text = text(pdf);
    let start = text.find("/Count ").expect("page tree count") + "/Count ".len();
    let digits: String = text[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let count = digits.parse().unwrap();
    assert_eq!(text.matches("/Type /Page\n").count(), count, "page objects");
    count
}

/// The glyph count of each text object, in content stream order.
fn glyphs_per_text_object(pdf: &[u8]) -> Vec<usize> {
    text(pdf)
        .split("\nBT\n")
        .skip(1)
        .map(|object| {
            object
                .split("\nET\n")
                .next()
                .unwrap()
                .matches(" Tj")
                .count()
        })
        .collect()
}

fn array(text: &str, key: &str) -> Vec<f64> {
    let needle = format!("/{key} [");
    let start = text.find(&needle).expect("key present") + needle.len();
    let end = start + text[start..].find(']').expect("array closes");
    text[start..end]
        .split_whitespace()
        .map(|v| v.parse::<f64>().unwrap())
        .collect()
}

fn image_dimensions(pdf: &[u8]) -> Vec<(u32, u32)> {
    text(pdf)
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

fn codes(record: &serde_json::Value) -> Vec<String> {
    record["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn merge_pdf_writes_one_file_per_row_with_every_page() {
    let env = Env::new(DOC, "name\nAlice\nBob\n");
    let output = env.run(
        "merge",
        &["--format", "pdf", "--json", "--manifest", "manifest.json"],
    );
    assert!(output.status.success(), "{}", report(&output));
    let report = report(&output);
    for (row, name) in [(0, "row-0001.pdf"), (1, "row-0002.pdf")] {
        assert_eq!(report["rows"][row]["outputs"], serde_json::json!([name]));
        let pdf = env.read(&format!("out/{name}"));
        assert!(pdf.starts_with(b"%PDF-"));
        assert_eq!(page_count(&pdf), 2);
        assert!(codes(&report["rows"][row]).contains(&"render.pdf_rasterized".to_owned()));
    }
    assert_eq!(fs::read_dir(env.dir.path().join("out")).unwrap().count(), 2);
    assert!(report.get("combined").is_none());
    let manifest = env.json("manifest.json");
    assert_eq!(manifest["schema"], "zenith-merge-manifest-v1");
    assert_eq!(
        manifest["rows"],
        serde_json::json!([
            {"row": 0, "outputs": ["row-0001.pdf"]},
            {"row": 1, "outputs": ["row-0002.pdf"]}
        ])
    );
    assert!(manifest.get("combined").is_none());
}

#[test]
fn a4_bleed_template_keeps_the_trim_box() {
    let env = Env::new(A4, "name\nAlice\n");
    let output = env.run("merge", &["--format", "pdf", "--name-by", "name"]);
    assert!(output.status.success());
    let text = text(&env.read("out/Alice.pdf"));
    // 3 mm = 8.504 pt. A4 trim is 595.5 x 842.25 pt.
    let b = 8.504;
    let trim = array(&text, "TrimBox");
    let want = [b, b, 595.5 + b, 842.25 + b];
    for (got, want) in trim.iter().zip(want) {
        assert!((got - want).abs() < 1e-3, "{trim:?}");
    }
    let bleed = array(&text, "BleedBox");
    assert!((bleed[2] - (595.5 + 2.0 * b)).abs() < 1e-3, "{bleed:?}");
}

#[test]
fn missing_glyph_row_writes_no_pdf_and_combine_skips_it() {
    let env = Env::new(A4, "name\nA\n\u{E000}B\nCCCCC\n");
    let output = env.run(
        "merge",
        &[
            "--format",
            "pdf",
            "--combine",
            "all.pdf",
            "--json",
            "--manifest",
            "manifest.json",
        ],
    );
    assert_eq!(output.status.code(), Some(1));
    let report = report(&output);
    assert_eq!(report["rows"][1]["status"], "failed");
    assert_eq!(report["rows"][1]["outputs"], serde_json::json!([]));
    assert!(codes(&report["rows"][1]).contains(&"font.glyph_missing".to_owned()));
    assert!(!env.dir.path().join("out/row-0002.pdf").exists());
    assert_eq!(fs::read_dir(env.dir.path().join("out")).unwrap().count(), 2);

    let combined = env.read("all.pdf");
    assert_eq!(page_count(&combined), 2);
    // Row 1 ("A") then row 3 ("CCCCC"), in CSV order.
    assert_eq!(glyphs_per_text_object(&combined), [1, 5]);
    assert_eq!(
        report["combined"],
        serde_json::json!({
            "path": "all.pdf", "status": "ok", "pages": 2, "rows": [0, 2], "excluded_rows": [1]
        })
    );
    assert!(report.get("diagnostics").is_none());
    let manifest = env.json("manifest.json");
    assert_eq!(
        manifest["combined"],
        serde_json::json!({"path": "all.pdf", "rows": [0, 2]})
    );
    // The per-row PDFs keep their own pages.
    assert_eq!(glyphs_per_text_object(&env.read("out/row-0001.pdf")), [1]);
    assert_eq!(glyphs_per_text_object(&env.read("out/row-0003.pdf")), [5]);
}

#[test]
fn combine_with_no_successful_row_writes_nothing_and_reports_it() {
    let env = Env::new(A4, "name\n\u{E000}\n");
    let output = env.run(
        "merge",
        &["--format", "pdf", "--combine", "all.pdf", "--json"],
    );
    assert_eq!(output.status.code(), Some(1));
    let report = report(&output);
    assert_eq!(report["combined"]["status"], "failed");
    assert_eq!(report["combined"]["excluded_rows"], serde_json::json!([0]));
    assert_eq!(report["diagnostics"][0]["code"], "merge.combine_failed");
    assert!(!env.dir.path().join("all.pdf").exists());
}

#[test]
fn combine_requires_pdf_and_cannot_replace_inputs() {
    for format in ["png", "svg"] {
        let env = Env::new(DOC, "name\nAlice\n");
        let output = env.run("merge", &["--format", format, "--combine", "all.pdf"]);
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("requires PDF output"));
        assert!(!env.dir.path().join("out").exists());
    }
    for target in ["data.csv", "out/row-0001.pdf"] {
        let env = Env::new(DOC, "name\nAlice\n");
        let output = env.run("merge", &["--format", "pdf", "--combine", target]);
        assert_eq!(output.status.code(), Some(2), "{target}");
        assert_eq!(env.read("data.csv"), b"name\nAlice\n");
        assert!(!env.dir.path().join("out/row-0001.pdf").exists());
    }
}

#[test]
fn pdf_fallback_policy_governs_merge_and_variant_rows() {
    for verb in ["allow", "warn", "deny"] {
        for command in ["merge", "variant"] {
            let env = Env::new(DOC, "name\nAlice\n");
            fs::write(
                env.dir.path().join(".zenith.kdl"),
                format!("diagnostics {{ {verb} \"render.pdf_rasterized\"; }}\n"),
            )
            .unwrap();
            let output = env.run(command, &["--format", "pdf", "--json"]);
            assert_eq!(output.status.success(), verb != "deny", "{verb}/{command}");
            let report = report(&output);
            let records = report[if command == "merge" {
                "rows"
            } else {
                "variants"
            }]
            .as_array()
            .unwrap();
            let fallbacks: Vec<_> = records
                .iter()
                .flat_map(|r| r["diagnostics"].as_array().unwrap())
                .filter(|d| d["code"] == "render.pdf_rasterized")
                .collect();
            match verb {
                "allow" => assert!(fallbacks.is_empty()),
                _ => {
                    assert_eq!(fallbacks.len(), 1);
                    let severity = if verb == "deny" { "error" } else { "warning" };
                    assert_eq!(fallbacks[0]["severity"], severity);
                    assert!(fallbacks[0]["message"].as_str().unwrap().contains("page 2"));
                }
            }
            if verb == "deny" {
                if command == "merge" {
                    assert_eq!(fs::read_dir(env.dir.path().join("out")).unwrap().count(), 0);
                } else {
                    assert!(env.dir.path().join("out/doc-plain.pdf").exists());
                    assert!(!env.dir.path().join("out/doc-shadow.pdf").exists());
                    assert!(!env.dir.path().join("out/doc-shadow.zen").exists());
                }
            }
        }
    }
}

#[test]
fn variant_pdf_writes_one_page_per_variant() {
    let env = Env::new(DOC, "name\n");
    let output = env.run(
        "variant",
        &["--format", "pdf", "--json", "--manifest", "m.json"],
    );
    assert!(output.status.success());
    let report = report(&output);
    for (index, id) in [(0, "plain"), (1, "shadow")] {
        let record = &report["variants"][index];
        assert_eq!(record["outputs_pdf"], format!("doc-{id}.pdf"));
        assert!(record.get("outputs_png").is_none());
        assert!(record.get("outputs_svg").is_none());
        assert_eq!(page_count(&env.read(&format!("out/doc-{id}.pdf"))), 1);
    }
    assert!(codes(&report["variants"][1]).contains(&"render.pdf_rasterized".to_owned()));
    assert_eq!(
        env.json("m.json")["targets"][0],
        serde_json::json!({"id": "plain", "source": "first", "outputs_zen": "doc-plain.zen", "outputs_pdf": "doc-plain.pdf"})
    );
}

#[test]
fn raster_scale_sets_pdf_capture_resolution() {
    for command in ["merge", "variant"] {
        let env = Env::new(DOC, "name\nAlice\n");
        let file = if command == "merge" {
            "out/row-0001.pdf"
        } else {
            "out/doc-shadow.pdf"
        };
        let mut seen = Vec::new();
        for scale in ["1", "2"] {
            let output = env.run(command, &["--format", "pdf", "--raster-scale", scale]);
            assert!(output.status.success());
            // The shadowed page 2 capture matches `zenith render --pdf` at the same scale.
            let render = Command::new(env!("CARGO_BIN_EXE_zenith"))
                .args(["render", "doc.zen", "--page", "2", "--pdf", "page.pdf"])
                .args(["--raster-scale", scale])
                .current_dir(env.dir.path())
                .env("HOME", env.home.path())
                .output()
                .unwrap();
            assert!(render.status.success());
            let dimensions = image_dimensions(&env.read(file));
            assert!(!dimensions.is_empty());
            assert_eq!(
                dimensions,
                image_dimensions(&env.read("page.pdf")),
                "{command}"
            );
            seen.push(dimensions);
        }
        assert!(seen[1][0].0 > seen[0][0].0, "{command}: {seen:?}");
        let bad = env.run(command, &["--format", "pdf", "--raster-scale", "5"]);
        assert_eq!(bad.status.code(), Some(2));
    }
}

#[test]
fn pdf_outputs_are_deterministic() {
    let first = Env::new(A4, "name\nAlice\nBob\n");
    let second = Env::new(A4, "name\nAlice\nBob\n");
    for env in [&first, &second] {
        let output = env.run(
            "merge",
            &[
                "--format",
                "pdf",
                "--combine",
                "all.pdf",
                "--manifest",
                "m.json",
            ],
        );
        assert!(output.status.success());
    }
    for name in ["out/row-0001.pdf", "out/row-0002.pdf", "all.pdf", "m.json"] {
        assert_eq!(first.read(name), second.read(name), "{name}");
    }
}

/// A file of one batch run and its SHA-256.
type Hashed = (&'static str, &'static str);

/// SHA-256 of the files and reports of a PNG and SVG batch, recorded before PDF output existed.
const UNCHANGED: &[(&str, &str, &[Hashed])] = &[
    (
        "merge",
        "png",
        &[
            (
                "m.json",
                "f1e89fa2415cb5c80c415b51b4a1b11aca97618e7af828d13f9dc3728bb052b4",
            ),
            (
                "out/row-0001-page-1.png",
                "955329c14ba321d9af8138e62e43cdc5aa57135ad4a1a2e47c9b195db47db149",
            ),
            (
                "out/row-0001-page-2.png",
                "9da527824e8764f58009cd2682a05683c2949f543122b469db3fad92f1baf3bd",
            ),
            (
                "out/row-0002-page-1.png",
                "0407491d4ee47e2b35d25ea1b367252f59a23c2aee79939766c7c7b2f94f4a8d",
            ),
            (
                "report.json",
                "7500bcafe9aa86c4e7d68e13cd203493d0ac6219d8af7c84b9ce7c4d582b539d",
            ),
        ],
    ),
    (
        "merge",
        "svg",
        &[
            (
                "m.json",
                "0a4978cb4c1d71389f4d17b834bccc0f47484765dcc4352f39fd356fad7c3632",
            ),
            (
                "out/row-0001-page-1.svg",
                "4a1816ce9519acfea9443e5063f766a9bc1457e34c96aaf16567afdd8fa38bdf",
            ),
            (
                "out/row-0001-page-2.svg",
                "b13555f2b93f1455823c6ef3cc7753cc8903080633f9231ec86b4174f63cd425",
            ),
            (
                "out/row-0002-page-1.svg",
                "b578089846400be44f022573601f9db20531629730fd6ce5d5f3a3bbd94dbcce",
            ),
            (
                "report.json",
                "5256753e992b723e70c8a3df86329b2b329717c233f8d12ca87020ff3ac052ca",
            ),
        ],
    ),
    (
        "variant",
        "png",
        &[
            (
                "m.json",
                "29dcd3b3c90e2fc98e759624f1d03e66df676ac40855a0e1db7dba2b996b16f3",
            ),
            (
                "out/fixture-plain.png",
                "af2a4c96f5867b1f4bd0fe9c6a2d3c0bda4d2499fe170bc50cdba19389572486",
            ),
            (
                "out/fixture-shadow.png",
                "9da527824e8764f58009cd2682a05683c2949f543122b469db3fad92f1baf3bd",
            ),
            (
                "report.json",
                "1d4a4489068b7506b19a1eae69186d24361aec0a188787b1f1e0ec8ea8f4eeb8",
            ),
        ],
    ),
    (
        "variant",
        "svg",
        &[
            (
                "m.json",
                "960bbb1e9ed27120d19870700ec164eae0ef91e1b9cd6b56b96a3bfdde2d920e",
            ),
            (
                "out/fixture-plain.svg",
                "618161b364aa3e721cee91571553ec0ffc520ec1d4e6c8c0b152d3f6f613d1ba",
            ),
            (
                "out/fixture-shadow.svg",
                "b13555f2b93f1455823c6ef3cc7753cc8903080633f9231ec86b4174f63cd425",
            ),
            (
                "report.json",
                "e98bfd777c83925d1fcff31b08406d5f6df313c2ecf6e2af1c34d905a38d5d5b",
            ),
        ],
    ),
];

#[test]
fn png_and_svg_batches_stay_byte_identical() {
    for &(command, format, files) in UNCHANGED {
        let dir = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        fs::write(dir.path().join("fixture.zen"), DOC).unwrap();
        fs::write(dir.path().join("fixture.csv"), "name\nAlice\nBob\n").unwrap();
        let mut process = Command::new(env!("CARGO_BIN_EXE_zenith"));
        process.args([command, "fixture.zen"]);
        if command == "merge" {
            process.arg("fixture.csv");
        }
        let output = process
            .args(["--out-dir", "out", "--format", format, "--json"])
            .args(["--manifest", "m.json"])
            .current_dir(dir.path())
            .env("HOME", home.path())
            .output()
            .unwrap();
        assert!(output.status.success(), "{command}/{format}");
        fs::write(dir.path().join("report.json"), &output.stdout).unwrap();
        for &(name, want) in files {
            let bytes = fs::read(Path::new(dir.path()).join(name)).unwrap();
            let got = format!("{:x}", Sha256::digest(&bytes));
            assert_eq!(got, want, "{command}/{format}: {name}");
        }
    }
}
