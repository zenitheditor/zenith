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

#[test]
fn merge_pdf_honors_document_cmyk_colorspace() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();

    let doc = r##"zenith version=1 colorspace="cmyk" {
  project id="proj.cmyk_merge" name="CMYK Merge"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.ink" type="color" value="#1e3a5f"
    token id="font.body" type="fontFamily" value="Noto Sans"
    token id="size.body" type="dimension" value=(px)16
  }
  styles {}
  document id="doc.cmyk_merge" title="CMYK Merge" {
    page id="page.1" w=(px)200 h=(px)100 {
      rect id="bg" x=(px)0 y=(px)0 w=(px)200 h=(px)100 fill=(token)"color.bg"
      text id="label" role="data.name" x=(px)16 y=(px)24 w=(px)168 h=(px)40 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" { span "Placeholder" }
    }
  }
}
"##;
    fs::write(dir.path().join("card.zen"), doc).unwrap();
    fs::write(dir.path().join("rows.csv"), "name\nAlice\nBob\n").unwrap();

    let out = run_cli(
        &dir,
        &home,
        &[
            "merge",
            "card.zen",
            "rows.csv",
            "--out-dir",
            "out",
            "--format",
            "pdf",
            "--combine",
            "combined.pdf",
        ],
    );
    assert!(
        out.status.success(),
        "merge failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let combined = fs::read(dir.path().join("combined.pdf")).unwrap();
    assert!(
        count_matches(&combined, b"/GTS_PDFX") > 0 && count_matches(&combined, b"FOGRA39") > 0,
        "combined merge PDF for colorspace=\"cmyk\" must emit PDF/X FOGRA39 OutputIntent"
    );
    assert_eq!(
        count_matches(&combined, b" rg\n"),
        0,
        "CMYK merge PDF must not emit DeviceRGB fill operators"
    );
    assert_eq!(
        count_matches(&combined, b"/Type /Page\n"),
        2,
        "combined merge PDF must contain 2 pages"
    );
}

#[test]
fn pdf_icc_profile_embeds_dest_output_profile_and_rejects_invalid_headers() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("leaflet.zen"), FOUR_PAGE_CMYK_DOC).unwrap();

    // Minimal valid 128-byte ISO 15076-1 CMYK ICC header ('CMYK' at 16..20, 'acsp' at 36..40).
    let mut valid_icc = vec![0u8; 128];
    valid_icc[0..4].copy_from_slice(&128u32.to_be_bytes());
    valid_icc[16..20].copy_from_slice(b"CMYK");
    valid_icc[36..40].copy_from_slice(b"acsp");
    fs::write(dir.path().join("fogra39.icc"), &valid_icc).unwrap();

    // Valid ICC profile embeds /DestOutputProfile with /N 4 and /FlateDecode, deterministically.
    let out1 = run_cli(
        &dir,
        &home,
        &[
            "render",
            "leaflet.zen",
            "--pdf",
            "with-icc-1.pdf",
            "--icc-profile",
            "fogra39.icc",
        ],
    );
    assert!(
        out1.status.success(),
        "render with --icc-profile failed: {}",
        String::from_utf8_lossy(&out1.stderr)
    );
    let out2 = run_cli(
        &dir,
        &home,
        &[
            "render",
            "leaflet.zen",
            "--pdf",
            "with-icc-2.pdf",
            "--icc-profile",
            "fogra39.icc",
        ],
    );
    assert!(out2.status.success());
    let pdf1 = fs::read(dir.path().join("with-icc-1.pdf")).unwrap();
    let pdf2 = fs::read(dir.path().join("with-icc-2.pdf")).unwrap();
    assert_eq!(pdf1, pdf2, "ICC-embedded PDF must be deterministic");
    assert!(count_matches(&pdf1, b"/GTS_PDFX") > 0);
    assert!(count_matches(&pdf1, b"/DestOutputProfile ") > 0);
    assert!(count_matches(&pdf1, b"/N 4") > 0);
    assert_eq!(count_matches(&pdf1, b" rg\n"), 0);

    // Invalid ICC (RGB instead of CMYK) must fail cleanly.
    let mut rgb_icc = valid_icc.clone();
    rgb_icc[16..20].copy_from_slice(b"RGB ");
    fs::write(dir.path().join("srgb.icc"), &rgb_icc).unwrap();
    let bad_rgb = run_cli(
        &dir,
        &home,
        &[
            "render",
            "leaflet.zen",
            "--pdf",
            "bad-rgb.pdf",
            "--icc-profile",
            "srgb.icc",
        ],
    );
    assert!(
        !bad_rgb.status.success(),
        "RGB ICC profile must be rejected"
    );

    // Guard: --icc-profile cannot be combined with --colorspace srgb.
    let bad_combo = run_cli(
        &dir,
        &home,
        &[
            "render",
            "leaflet.zen",
            "--pdf",
            "bad-combo.pdf",
            "--colorspace",
            "srgb",
            "--icc-profile",
            "fogra39.icc",
        ],
    );
    assert!(
        !bad_combo.status.success(),
        "--icc-profile with --colorspace srgb must fail"
    );
}

#[test]
fn pdf_impose_grid_and_cut_and_stack() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("leaflet.zen"), FOUR_PAGE_CMYK_DOC).unwrap();

    // 4 pages of 100x140 px on a 2x2 grid -> 1 sheet of 200x280 px = 150x210 pt.
    let grid_out = run_cli(
        &dir,
        &home,
        &[
            "render",
            "leaflet.zen",
            "--pdf",
            "grid.pdf",
            "--impose",
            "grid",
            "--grid",
            "2x2",
        ],
    );
    assert!(
        grid_out.status.success(),
        "grid impose failed: {}",
        String::from_utf8_lossy(&grid_out.stderr)
    );
    let grid_bytes = fs::read(dir.path().join("grid.pdf")).unwrap();
    assert_eq!(count_matches(&grid_bytes, b"/MediaBox [0 0 150 210]"), 1);

    // 4 pages on a 2x1 cut-and-stack -> 2 sheets: [1, 3] and [2, 4], distinct from spread [1, 2], [3, 4].
    let stack_out = run_cli(
        &dir,
        &home,
        &[
            "render",
            "leaflet.zen",
            "--pdf",
            "stack.pdf",
            "--impose",
            "cut-and-stack",
            "--grid",
            "2x1",
            "--marks",
            "--color-bars",
        ],
    );
    assert!(
        stack_out.status.success(),
        "cut-and-stack impose failed: {}",
        String::from_utf8_lossy(&stack_out.stderr)
    );
    let stack_bytes = fs::read(dir.path().join("stack.pdf")).unwrap();
    // 200x140 trim + 24px slug on each side = 248x188 px = 186x141 pt.
    assert_eq!(count_matches(&stack_bytes, b"/MediaBox [0 0 186 141]"), 2);
    // Registration black (1 1 1 1 K) emitted for 4-plate registration targets.
    assert!(
        count_matches(&stack_bytes, b"1 1 1 1 K") > 0,
        "--color-bars must emit 4-plate registration targets"
    );

    // Guard: --grid on booklet must fail.
    let bad_grid = run_cli(
        &dir,
        &home,
        &[
            "render",
            "leaflet.zen",
            "--pdf",
            "bad.pdf",
            "--impose",
            "booklet",
            "--grid",
            "2x2",
        ],
    );
    assert!(!bad_grid.status.success(), "--grid with booklet must fail");
}

#[test]
fn merge_combine_cut_and_stack_impose_and_print_preset() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();

    let doc = r##"zenith version=1 {
  project id="proj.tickets" name="Tickets"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.ink" type="color" value="#1e3a5f"
    token id="font.body" type="fontFamily" value="Noto Sans"
    token id="size.body" type="dimension" value=(px)14
  }
  styles {}
  document id="doc.tickets" title="Tickets" {
    page id="page.1" w=(px)100 h=(px)60 {
      rect id="bg" x=(px)0 y=(px)0 w=(px)100 h=(px)60 fill=(token)"color.bg"
      text id="label" role="data.code" x=(px)8 y=(px)16 w=(px)84 h=(px)28 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" { span "T-000" }
    }
  }
}
"##;
    fs::write(dir.path().join("ticket.zen"), doc).unwrap();
    fs::write(
        dir.path().join("rows.csv"),
        "code\nT-001\nT-002\nT-003\nT-004\nT-005\n",
    )
    .unwrap();

    // 5 rows on a 2x2 cut-and-stack sheet with --print --marks -> 2 CMYK press sheets.
    // Trim = 200x120 px + 24px slug each side = 248x168 px = 186x126 pt.
    let out = run_cli(
        &dir,
        &home,
        &[
            "merge",
            "ticket.zen",
            "rows.csv",
            "--out-dir",
            "out",
            "--format",
            "pdf",
            "--combine",
            "press-sheets.pdf",
            "--print",
            "--impose",
            "cut-and-stack",
            "--grid",
            "2x2",
            "--marks",
        ],
    );
    assert!(
        out.status.success(),
        "merge --combine --impose failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let pdf = fs::read(dir.path().join("press-sheets.pdf")).unwrap();
    assert_eq!(count_matches(&pdf, b"/MediaBox [0 0 186 126]"), 2);
    assert!(count_matches(&pdf, b"/GTS_PDFX") > 0 && count_matches(&pdf, b"FOGRA39") > 0);
    assert_eq!(count_matches(&pdf, b" rg\n"), 0);

    // Guard: --impose on merge without --combine must fail.
    let bad = run_cli(
        &dir,
        &home,
        &[
            "merge",
            "ticket.zen",
            "rows.csv",
            "--out-dir",
            "out2",
            "--format",
            "pdf",
            "--impose",
            "cut-and-stack",
        ],
    );
    assert!(
        !bad.status.success(),
        "merge --impose without --combine must fail"
    );
}
