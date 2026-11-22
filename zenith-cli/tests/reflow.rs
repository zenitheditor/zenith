//! CLI integration tests for `zenith reflow` (automatic overflow pagination).

use std::fs;
use std::process::{Command, Output};
use tempfile::TempDir;

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
fn reflow_spawns_continuation_pages_preserving_master_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();

    // Long text (~40 lines at 16px in a 200x120 box -> needs multiple continuation pages).
    let long_copy = (1..=36)
        .map(|i| format!("Sentence {i} of the long manuscript that flows across pages."))
        .collect::<Vec<_>>()
        .join(" ");

    let doc = format!(
        r##"zenith version=1 {{
  project id="proj.book" name="Book"
  tokens format="zenith-token-v1" {{
    token id="color.bg" type="color" value="#ffffff"
    token id="color.ink" type="color" value="#111827"
    token id="font.body" type="fontFamily" value="Noto Sans"
    token id="size.body" type="dimension" value=(px)14
    token id="size.hdr" type="dimension" value=(px)10
  }}
  styles {{}}
  masters {{
    master id="m.body" {{
      text id="m.hdr" x=(px)20 y=(px)10 w=(px)200 h=(px)16 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.hdr" {{ span "Chapter One" }}
    }}
  }}
  document id="doc.book" title="Book" {{
    page id="page.1" master="m.body" w=(px)240 h=(px)180 background=(token)"color.bg" {{
      text id="body" chain="story" x=(px)20 y=(px)36 w=(px)200 h=(px)120 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {{ span "{long_copy}" }}
    }}
  }}
}}
"##
    );
    fs::write(dir.path().join("book.zen"), &doc).unwrap();

    // 1. Dry-run reports spawned pages without touching disk.
    let dry = run_cli(&dir, &home, &["reflow", "book.zen", "--json"]);
    assert!(
        dry.status.success(),
        "dry-run reflow failed: {}",
        String::from_utf8_lossy(&dry.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&dry.stdout).expect("json report");
    assert_eq!(report["schema"], "zenith-reflow-v1");
    assert_eq!(report["applied"], false);
    assert_eq!(report["pages_before"], 1);
    let pages_after = report["pages_after"].as_u64().unwrap();
    assert!(pages_after >= 3, "expected >= 3 pages, got {pages_after}");
    assert_eq!(report["still_overflowing"], false);
    assert_eq!(
        fs::read_to_string(dir.path().join("book.zen")).unwrap(),
        doc,
        "dry-run must not modify file on disk"
    );

    // 2. Apply spawns continuation pages on disk.
    let applied = run_cli(&dir, &home, &["reflow", "book.zen", "--apply", "--json"]);
    assert!(
        applied.status.success(),
        "apply reflow failed: {}",
        String::from_utf8_lossy(&applied.stderr)
    );
    let applied_report: serde_json::Value = serde_json::from_slice(&applied.stdout).unwrap();
    assert_eq!(applied_report["applied"], true);
    assert_eq!(applied_report["pages_after"], pages_after);

    let after_src = fs::read_to_string(dir.path().join("book.zen")).unwrap();
    assert!(after_src.contains("id=\"page.2\""));
    assert!(after_src.contains("master=\"m.body\""));

    // 3. Resulting document validates cleanly with zero errors and renders to imposed booklet PDF!
    let val = run_cli(&dir, &home, &["validate", "book.zen"]);
    assert!(
        val.status.success(),
        "validate failed after reflow: {}",
        String::from_utf8_lossy(&val.stderr)
    );
    let pdf = run_cli(
        &dir,
        &home,
        &[
            "render",
            "book.zen",
            "--pdf",
            "booklet.pdf",
            "--impose",
            "booklet",
        ],
    );
    assert!(
        pdf.status.success(),
        "booklet render failed after reflow: {}",
        String::from_utf8_lossy(&pdf.stderr)
    );

    // 4. Re-running reflow is idempotent (0 pages added, file byte-identical).
    let second = run_cli(&dir, &home, &["reflow", "book.zen", "--apply", "--json"]);
    assert!(second.status.success());
    let second_report: serde_json::Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(second_report["pages_added"], 0);
    assert_eq!(
        fs::read_to_string(dir.path().join("book.zen")).unwrap(),
        after_src
    );
}
