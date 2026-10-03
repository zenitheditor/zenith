//! `zenith render --scale` and `--contact-sheet`: exact sizes, byte identity
//! at scale 1, determinism, the JSON envelope, and argument errors.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

fn zenith(args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_zenith"))
        .args(args)
        .output()
        .expect("run zenith");
    Run {
        code: output.status.code().expect("exit code"),
        stdout: String::from_utf8(output.stdout).expect("stdout utf8"),
        stderr: String::from_utf8(output.stderr).expect("stderr utf8"),
    }
}

fn json(run: &Run) -> Value {
    serde_json::from_str(&run.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is not JSON ({e}); stdout:\n{}\nstderr:\n{}",
            run.stdout, run.stderr
        )
    })
}

fn s(path: &Path) -> &str {
    path.to_str().expect("utf8 path")
}

/// `(width, height)` from a PNG's IHDR chunk.
fn png_size(path: &Path) -> (u32, u32) {
    let bytes = fs::read(path).expect("read png");
    let be = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().expect("4 bytes"));
    (be(16), be(20))
}

/// An `n`-page document of `w`×`h` pages, each with a distinct fill and text.
fn doc(dir: &Path, n: usize, w: u32, h: u32) -> PathBuf {
    let mut tokens = String::new();
    let mut pages = String::new();
    for i in 0..n {
        let shade = 30 + i * 20;
        tokens.push_str(&format!(
            "    token id=\"color.p{i}\" type=\"color\" value=\"#{shade:02x}6090\"\n"
        ));
        pages.push_str(&format!(
            r##"    page id="page.{i}" w=(px){w} h=(px){h} {{
      rect id="rect.{i}" x=(px)0 y=(px)0 w=(px){w} h=(px){h} fill=(token)"color.p{i}"
      rect id="mark.{i}" x=(px)20 y=(px)20 w=(px)60 h=(px)40 fill=(token)"color.ink" stroke=(token)"color.ink" stroke-width=(token)"size.stroke"
      text id="title.{i}" x=(px)20 y=(px)80 w=(px)200 h=(px)40 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {{
        span "Slide {i}"
      }}
    }}
"##
        ));
    }
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.sc" name="SC"
  tokens format="zenith-token-v1" {{
    token id="color.ink" type="color" value="#111827"
    token id="font.body" type="fontFamily" value="Noto Sans"
    token id="size.body" type="dimension" value=(px)18
    token id="size.stroke" type="dimension" value=(px)4
{tokens}  }}
  styles {{}}
  document id="doc.sc" title="SC" {{
{pages}  }}
}}
"##
    );
    let path = dir.join("deck.zen");
    fs::write(&path, src).expect("write doc");
    path
}

fn render_ok(args: &[&str]) {
    let run = zenith(args);
    assert_eq!(
        run.code, 0,
        "args {args:?}\nstdout {}\nstderr {}",
        run.stdout, run.stderr
    );
}

#[test]
fn scale_one_is_byte_identical_to_no_flag() {
    let tmp = TempDir::new().expect("tempdir");
    let d = doc(tmp.path(), 2, 240, 160);
    let a = tmp.path().join("a.png");
    let b = tmp.path().join("b.png");
    render_ok(&["render", s(&d), "--png", s(&a)]);
    render_ok(&["render", s(&d), "--png", s(&b), "--scale", "1"]);
    assert_eq!(fs::read(&a).expect("a"), fs::read(&b).expect("b"));

    let da = tmp.path().join("pa");
    let db = tmp.path().join("pb");
    render_ok(&["render", s(&d), "--all-pages", s(&da)]);
    render_ok(&["render", s(&d), "--all-pages", s(&db), "--scale", "1.0"]);
    for p in ["page-1.png", "page-2.png"] {
        assert_eq!(
            fs::read(da.join(p)).expect("pa"),
            fs::read(db.join(p)).expect("pb")
        );
    }
}

#[test]
fn half_scale_png_has_exact_size_and_is_deterministic() {
    let tmp = TempDir::new().expect("tempdir");
    let d = doc(tmp.path(), 1, 241, 161);
    let a = tmp.path().join("a.png");
    let b = tmp.path().join("b.png");
    render_ok(&["render", s(&d), "--png", s(&a), "--scale", "0.5"]);
    render_ok(&["render", s(&d), "--png", s(&b), "--scale", "0.5"]);
    // round(241 × 0.5) = 121, round(161 × 0.5) = 81 (half away from zero).
    assert_eq!(png_size(&a), (121, 81));
    assert_eq!(fs::read(&a).expect("a"), fs::read(&b).expect("b"));
}

#[test]
fn scaled_all_pages_and_spread_sizes() {
    let tmp = TempDir::new().expect("tempdir");
    let d = doc(tmp.path(), 2, 240, 160);
    let dir = tmp.path().join("pages");
    render_ok(&["render", s(&d), "--all-pages", s(&dir), "--scale", "0.25"]);
    assert_eq!(png_size(&dir.join("page-1.png")), (60, 40));
    assert_eq!(png_size(&dir.join("page-2.png")), (60, 40));

    let spread = tmp.path().join("spread.png");
    render_ok(&[
        "render",
        s(&d),
        "--spread",
        "1-2",
        "--png",
        s(&spread),
        "--gutter",
        "20",
        "--scale",
        "0.5",
    ]);
    // 120 + round(20 × 0.5) + 120.
    assert_eq!(png_size(&spread), (250, 80));
}

/// Expected sheet size for `n` pages of `w`×`h` at scale 1 (see `sheet.rs`).
fn sheet_size(n: u32, w: u32, h: u32) -> (u32, u32) {
    let cols = (1..).find(|c| c * c >= n).expect("columns");
    let rows = n.div_ceil(cols);
    (
        cols * w + (cols + 1) * 16,
        rows * (h + 28) + (rows + 1) * 16,
    )
}

#[test]
fn contact_sheet_sizes_for_1_3_7_pages() {
    for n in [1usize, 3, 7] {
        let tmp = TempDir::new().expect("tempdir");
        let d = doc(tmp.path(), n, 240, 160);
        let sheet = tmp.path().join("sheet.png");
        render_ok(&["render", s(&d), "--contact-sheet", s(&sheet)]);
        let expected = sheet_size(n as u32, 240, 160);
        assert_eq!(png_size(&sheet), expected, "n = {n}");
    }
    // Pinned values for the documented rule.
    assert_eq!(sheet_size(1, 240, 160), (272, 220));
    assert_eq!(sheet_size(3, 240, 160), (528, 424));
    assert_eq!(sheet_size(7, 240, 160), (784, 628));
}

#[test]
fn contact_sheet_auto_scale_fits_2048_wide() {
    let tmp = TempDir::new().expect("tempdir");
    let d = doc(tmp.path(), 7, 1920, 1080);
    let sheet = tmp.path().join("sheet.png");
    let run = zenith(&["render", s(&d), "--contact-sheet", s(&sheet), "--json"]);
    assert_eq!(run.code, 0, "stderr {}", run.stderr);
    let (w, _) = png_size(&sheet);
    // 3 columns: cell = floor((2048 − 4 × 16) / 3) = 661 → 3 × 661 + 64.
    assert_eq!(w, 3 * 661 + 64);
    assert!(w <= 2048);
    let v = json(&run);
    let scale = v["images"][0]["scale"].as_f64().expect("scale");
    assert!((scale - 661.0 / 1920.0).abs() < 1e-12, "scale {scale}");
}

#[test]
fn contact_sheet_is_deterministic_and_respects_scale_and_page() {
    let tmp = TempDir::new().expect("tempdir");
    let d = doc(tmp.path(), 3, 240, 160);
    let a = tmp.path().join("a.png");
    let b = tmp.path().join("b.png");
    render_ok(&["render", s(&d), "--contact-sheet", s(&a), "--scale", "0.5"]);
    render_ok(&["render", s(&d), "--contact-sheet", s(&b), "--scale", "0.5"]);
    assert_eq!(fs::read(&a).expect("a"), fs::read(&b).expect("b"));
    assert_eq!(png_size(&a), sheet_size(3, 120, 80));

    let one = tmp.path().join("one.png");
    render_ok(&["render", s(&d), "--contact-sheet", s(&one), "--page", "2"]);
    assert_eq!(png_size(&one), sheet_size(1, 240, 160));
}

#[test]
fn json_envelope_lists_sheet_and_scaled_png() {
    let tmp = TempDir::new().expect("tempdir");
    let d = doc(tmp.path(), 3, 240, 160);
    let png = tmp.path().join("p.png");
    let sheet = tmp.path().join("sheet.png");
    let run = zenith(&[
        "render",
        s(&d),
        "--png",
        s(&png),
        "--contact-sheet",
        s(&sheet),
        "--scale",
        "0.5",
        "--json",
    ]);
    assert_eq!(run.code, 0, "stderr {}", run.stderr);
    let v = json(&run);
    assert_eq!(v["schema"], "zenith-render-v1");
    assert_eq!(v["status"], "ok");
    let outputs: Vec<&str> = v["outputs"]
        .as_array()
        .expect("outputs")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(outputs, vec![s(&png), s(&sheet)]);
    let images = v["images"].as_array().expect("images");
    assert_eq!(images.len(), 2);
    assert_eq!(images[0]["kind"], "png");
    assert_eq!(images[0]["width"], 120);
    assert_eq!(images[0]["height"], 80);
    assert_eq!(images[0]["scale"], 0.5);
    let sh = &images[1];
    assert_eq!(sh["kind"], "contact_sheet");
    assert_eq!(sh["path"], s(&sheet));
    let (w, h) = sheet_size(3, 120, 80);
    assert_eq!(sh["width"], w);
    assert_eq!(sh["height"], h);
    assert_eq!(sh["scale"], 0.5);
    assert_eq!(sh["columns"], 2);
    assert_eq!(sh["rows"], 2);
    assert_eq!(sh["pages"], serde_json::json!([1, 2, 3]));
}

#[test]
fn human_output_prints_sheet_size() {
    let tmp = TempDir::new().expect("tempdir");
    let d = doc(tmp.path(), 3, 240, 160);
    let sheet = tmp.path().join("sheet.png");
    let run = zenith(&["render", s(&d), "--contact-sheet", s(&sheet)]);
    assert_eq!(run.code, 0, "stderr {}", run.stderr);
    assert!(
        run.stdout.contains("contact sheet written to") && run.stdout.contains("528x424 px"),
        "stdout {}",
        run.stdout
    );
}

#[test]
fn invalid_scale_is_invalid_argument_envelope() {
    let tmp = TempDir::new().expect("tempdir");
    let d = doc(tmp.path(), 1, 240, 160);
    let png = tmp.path().join("p.png");
    for bad in ["0", "-1", "-0.5", "4.5", "NaN", "inf", "abc"] {
        let run = zenith(&["render", s(&d), "--png", s(&png), "--scale", bad, "--json"]);
        assert_eq!(run.code, 2, "scale {bad}: stdout {}", run.stdout);
        let v = json(&run);
        assert_eq!(v["schema"], "zenith-error-v1", "scale {bad}");
        assert_eq!(
            v["diagnostics"][0]["code"], "cli.invalid_argument",
            "scale {bad}"
        );
        assert_eq!(v["diagnostics"][0]["severity"], "error");
        let msg = v["diagnostics"][0]["message"].as_str().unwrap_or("");
        assert!(msg.contains(bad), "message names the input: {msg}");
        assert!(!png.exists(), "no output for scale {bad}");
    }
    // `--scale 4` is the inclusive maximum.
    render_ok(&["render", s(&d), "--png", s(&png), "--scale", "4"]);
    assert_eq!(png_size(&png), (960, 640));
}

#[test]
fn scale_without_png_output_is_invalid_argument() {
    let tmp = TempDir::new().expect("tempdir");
    let d = doc(tmp.path(), 1, 240, 160);
    let pdf = tmp.path().join("p.pdf");
    let run = zenith(&[
        "render",
        s(&d),
        "--pdf",
        s(&pdf),
        "--scale",
        "0.5",
        "--json",
    ]);
    assert_eq!(run.code, 2);
    let v = json(&run);
    assert_eq!(v["diagnostics"][0]["code"], "cli.invalid_argument");
}
