//! CLI integration tests for PDF `--colorspace`, `--print`, and `--impose`.

use std::fs;
use std::process::{Command, Output};
use tempfile::TempDir;

const FOUR_PAGE_CMYK_DOC: &str = r##"zenith version=1 {
  project id="proj.leaflet" name="Leaflet"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.ink" type="color" value="#112233"
    token id="color.accent" type="color" value="#2563eb"
    token id="shadow.card" type="shadow" { layer dx=(px)1 dy=(px)2 blur=(px)2 color=(token)"color.ink"; }
  }
  styles {}
  document id="doc.leaflet" colorspace="cmyk" title="Leaflet" {
    page id="page.1" w=(px)100 h=(px)140 background=(token)"color.bg" {
      rect id="p1.r" x=(px)10 y=(px)10 w=(px)40 h=(px)40 fill=(token)"color.accent" shadow=(token)"shadow.card"
    }
    page id="page.2" w=(px)100 h=(px)140 background=(token)"color.bg" {
      rect id="p2.r" x=(px)10 y=(px)10 w=(px)40 h=(px)40 fill=(token)"color.ink"
    }
    page id="page.3" w=(px)100 h=(px)140 background=(token)"color.bg" {
      rect id="p3.r" x=(px)10 y=(px)10 w=(px)40 h=(px)40 fill=(token)"color.accent"
    }
    page id="page.4" w=(px)100 h=(px)140 background=(token)"color.bg" {
      rect id="p4.r" x=(px)10 y=(px)10 w=(px)40 h=(px)40 fill=(token)"color.ink"
    }
  }
}
"##;

fn run_cli(dir: &TempDir, home: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_zenith"))
        .args(args)
        .current_dir(dir.path())
        .env("HOME", home.path())
        .env("ZENITH_DATA_DIR", dir.path().join("store"))
        .output()
        .unwrap()
}

fn count_matches(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|w| *w == needle)
        .count()
}

#[test]
fn pdf_colorspace_print_and_impose_modes() {
    let dir = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    fs::write(dir.path().join("leaflet.zen"), FOUR_PAGE_CMYK_DOC).unwrap();

    // 1. Digital sharing: --colorspace srgb overrides document colorspace="cmyk" and emits 4 pages.
    let share = run_cli(
        &dir,
        &home,
        &[
            "render",
            "leaflet.zen",
            "--pdf",
            "share.pdf",
            "--colorspace",
            "srgb",
        ],
    );
    assert!(
        share.status.success(),
        "{}",
        String::from_utf8_lossy(&share.stderr)
    );
    let share_bytes = fs::read(dir.path().join("share.pdf")).unwrap();
    assert!(count_matches(&share_bytes, b"/DeviceRGB") > 0);
    assert_eq!(count_matches(&share_bytes, b"/DeviceCMYK"), 0);
    assert_eq!(
        count_matches(&share_bytes, b"/MediaBox [0 0 75 105]"),
        4,
        "4 reading-order pages at 75x105 pt"
    );

    // 2. Print preset + booklet imposition: 2 double-wide CMYK sheets, zero DeviceRGB.
    let print_booklet = run_cli(
        &dir,
        &home,
        &[
            "render",
            "leaflet.zen",
            "--pdf",
            "print-booklet.pdf",
            "--print",
            "--impose",
            "booklet",
        ],
    );
    assert!(
        print_booklet.status.success(),
        "{}",
        String::from_utf8_lossy(&print_booklet.stderr)
    );
    let booklet_bytes = fs::read(dir.path().join("print-booklet.pdf")).unwrap();
    assert!(count_matches(&booklet_bytes, b"/DeviceCMYK") > 0);
    assert!(count_matches(&booklet_bytes, b"/GTS_PDFX") > 0);
    assert!(count_matches(&booklet_bytes, b"FOGRA39") > 0);
    assert_eq!(count_matches(&booklet_bytes, b"/DeviceRGB"), 0);
    assert_eq!(
        count_matches(&booklet_bytes, b"/MediaBox [0 0 150 105]"),
        2,
        "2 imposed booklet sheets at 150x105 pt"
    );

    // 2b. Print booklet with --marks + --duplex long-edge adds slug margin, TrimBox, and marks.
    let marked = run_cli(
        &dir,
        &home,
        &[
            "render",
            "leaflet.zen",
            "--pdf",
            "print-marked.pdf",
            "--print",
            "--impose",
            "booklet",
            "--marks",
            "--duplex",
            "long-edge",
        ],
    );
    assert!(
        marked.status.success(),
        "{}",
        String::from_utf8_lossy(&marked.stderr)
    );
    let marked_bytes = fs::read(dir.path().join("print-marked.pdf")).unwrap();
    // 24 px slug = 18 pt on each side -> MediaBox 186x141, TrimBox [18 18 168 123].
    assert_eq!(count_matches(&marked_bytes, b"/MediaBox [0 0 186 141]"), 2);
    assert_eq!(count_matches(&marked_bytes, b"/TrimBox [18 18 168 123]"), 2);

    // 3. 2-up reading-order spread alias (`--impose 2up`) produces 2 double-wide sheets.
    let spread = run_cli(
        &dir,
        &home,
        &[
            "render",
            "leaflet.zen",
            "--pdf",
            "spread.pdf",
            "--print",
            "--impose",
            "2up",
        ],
    );
    assert!(
        spread.status.success(),
        "{}",
        String::from_utf8_lossy(&spread.stderr)
    );
    let spread_bytes = fs::read(dir.path().join("spread.pdf")).unwrap();
    assert_eq!(count_matches(&spread_bytes, b"/MediaBox [0 0 150 105]"), 2);
    assert_ne!(
        booklet_bytes, spread_bytes,
        "booklet [4|1],[2|3] must differ from spread [1|2],[3|4]"
    );

    // 4. Flag validation guards.
    for bad_args in [
        vec!["render", "leaflet.zen", "--png", "out.png", "--print"],
        vec![
            "render",
            "leaflet.zen",
            "--pdf",
            "bad.pdf",
            "--print",
            "--colorspace",
            "srgb",
        ],
        vec![
            "render",
            "leaflet.zen",
            "--pdf",
            "bad.pdf",
            "--impose",
            "booklet",
            "--page",
            "1",
        ],
    ] {
        let out = run_cli(&dir, &home, &bad_args);
        assert!(
            !out.status.success(),
            "expected usage error for {bad_args:?}"
        );
    }
}
