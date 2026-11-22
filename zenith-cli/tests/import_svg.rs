//! CLI integration tests for `zenith import svg`.

use std::fs;
use std::process::{Command, Output};
use tempfile::TempDir;

const LOGO_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="120" viewBox="0 0 200 120">
  <rect id="bg" x="0" y="0" width="200" height="120" fill="#1e3a5f"/>
  <g transform="translate(20, 10)" opacity="0.5">
    <circle id="dot" cx="40" cy="40" r="24" fill="#f59e0b" stroke="#1e3a5f" stroke-width="2.5"/>
  </g>
  <path d="M10 100 L190 100" fill="none" stroke="#1e3a5f" stroke-width="2.5" stroke-linecap="round"/>
</svg>"##;

fn run_cli(dir: &TempDir, home: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_zenith"))
        .args(args)
        .current_dir(dir.path())
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path())
        .output()
        .expect("run zenith binary")
}

#[test]
fn import_svg_produces_valid_renderable_deterministic_zen_document() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("logo.svg"), LOGO_SVG).unwrap();

    // 1. Import SVG -> logo.zen with JSON report.
    let imp = run_cli(
        &dir,
        &home,
        &[
            "import",
            "svg",
            "logo.svg",
            "--out",
            "logo.zen",
            "--name",
            "Brand Logo",
            "--json",
        ],
    );
    assert!(
        imp.status.success(),
        "import svg failed: {}",
        String::from_utf8_lossy(&imp.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&imp.stdout).expect("json report");
    assert_eq!(report["schema"], "zenith-import-svg-v1");
    assert_eq!(report["paths_imported"], 3);
    assert_eq!(report["color_tokens"], 3);
    assert_eq!(report["stroke_tokens"], 1);

    let zen_first = fs::read(dir.path().join("logo.zen")).unwrap();

    // 2. Imported .zen must validate cleanly with zero hard errors.
    let val = run_cli(&dir, &home, &["validate", "logo.zen"]);
    assert!(
        val.status.success(),
        "validate failed on imported .zen: {}",
        String::from_utf8_lossy(&val.stderr)
    );

    // 3. Imported .zen must render cleanly to PNG.
    let ren = run_cli(&dir, &home, &["render", "logo.zen", "--png", "logo.png"]);
    assert!(
        ren.status.success(),
        "render failed on imported .zen: {}",
        String::from_utf8_lossy(&ren.stderr)
    );
    let png = fs::read(dir.path().join("logo.png")).unwrap();
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "valid PNG signature");

    // 4. Refuses to overwrite without --force; --force overwrites deterministically.
    let no_force = run_cli(
        &dir,
        &home,
        &["import", "svg", "logo.svg", "-o", "logo.zen"],
    );
    assert!(
        !no_force.status.success(),
        "must refuse overwrite without --force"
    );

    let forced = run_cli(
        &dir,
        &home,
        &[
            "import",
            "svg",
            "logo.svg",
            "-o",
            "logo.zen",
            "--name",
            "Brand Logo",
            "--force",
        ],
    );
    assert!(forced.status.success());
    let zen_second = fs::read(dir.path().join("logo.zen")).unwrap();
    assert_eq!(zen_first, zen_second, "import svg must be deterministic");
}
