//! Print preflight on the validate and render paths: `image.effective_ppi`,
//! `ink.rich_black`, `ink.total_coverage`, and `font.not_embeddable`.
//!
//! The fixture in `tests/fixtures/print_preflight/` holds a 300×200 px PNG
//! and a document with one print page (`bleed`) and one screen page.

use std::path::Path;

use zenith_core::{Diagnostic, Severity, bundled_faces};
use zenith_pipeline::render::render_png_pages;
use zenith_pipeline::{FsConfig, Host, MemFs, PolicyFlags, RenderOptions, validate_source};

const FIXTURE_DIR: &str = "tests/fixtures/print_preflight";
const PRINT_CODES: [&str; 4] = [
    "image.effective_ppi",
    "image.effective_ppi_target",
    "ink.rich_black",
    "ink.total_coverage",
];

/// The fixture document source.
fn fixture_source() -> String {
    std::fs::read_to_string(Path::new(FIXTURE_DIR).join("print.zen")).expect("read print.zen")
}

/// A memory file system that holds the fixture PNG under `fx/`.
fn fixture_fs() -> MemFs {
    let png = std::fs::read(Path::new(FIXTURE_DIR).join("photo-300x200.png")).expect("read png");
    MemFs::new().with("fx/photo-300x200.png", png)
}

/// Validate `src` with project directory `fx/` over `fs`.
fn validate_with(fs: &MemFs, src: &str) -> (Vec<Diagnostic>, u8) {
    let config = FsConfig::new(fs, None);
    let host = Host::new(fs, &config);
    let v = validate_source(host, src, Some(Path::new("fx")), &PolicyFlags::default());
    (v.diagnostics, v.exit_code)
}

fn validate(src: &str) -> (Vec<Diagnostic>, u8) {
    validate_with(&fixture_fs(), src)
}

/// The diagnostics with `code` and subject `id`.
fn find<'a>(diags: &'a [Diagnostic], code: &str, id: &str) -> Vec<&'a Diagnostic> {
    diags
        .iter()
        .filter(|d| d.code == code && d.subject_id.as_deref() == Some(id))
        .collect()
}

/// The one diagnostic with `code` and subject `id`.
fn one<'a>(diags: &'a [Diagnostic], code: &str, id: &str) -> &'a Diagnostic {
    let found = find(diags, code, id);
    assert_eq!(found.len(), 1, "{code} on {id}: {diags:#?}");
    found[0]
}

#[test]
fn effective_ppi_warns_below_150_and_advises_below_300() {
    let (diags, exit) = validate(&fixture_source());
    assert_eq!(exit, 0, "print warnings must not fail validate: {diags:#?}");

    let low = one(&diags, "image.effective_ppi", "img.low");
    assert_eq!(low.severity, Severity::Warning);
    assert_eq!(
        low.message,
        "image 'img.low' prints at 100 ppi < 150 ppi floor (300×200 px source in a 3×2 in box, \
         fit=stretch) — supply a source of at least 450×300 px, or shrink the box to 2×1.33 in."
    );

    let mid = one(&diags, "image.effective_ppi_target", "img.mid");
    assert_eq!(mid.severity, Severity::Advisory);
    assert!(
        mid.message.contains("prints at 200 ppi < 300 ppi target"),
        "{}",
        mid.message
    );
    assert!(
        mid.message
            .contains("at least 450×300 px, or shrink the box to 1×0.67 in"),
        "{}",
        mid.message
    );

    assert!(
        find(&diags, "image.effective_ppi", "img.mid").is_empty(),
        "one band per image: {diags:#?}"
    );
    for code in ["image.effective_ppi", "image.effective_ppi_target"] {
        assert!(
            find(&diags, code, "img.high").is_empty(),
            "400 ppi is silent: {diags:#?}"
        );
    }
}

#[test]
fn cropped_image_counts_only_the_visible_source_region() {
    let (diags, _) = validate(&fixture_source());
    // Uncropped, a 144×96 px box is 200 ppi. A 150×100 px crop is 100 ppi.
    let crop = one(&diags, "image.effective_ppi", "img.crop");
    assert_eq!(crop.severity, Severity::Warning);
    assert!(
        crop.message
            .contains("prints at 100 ppi < 150 ppi floor (150×100 px crop of a 300×200 px source"),
        "{}",
        crop.message
    );
}

#[test]
fn print_page_reports_ppi_instead_of_upscale() {
    let (diags, _) = validate(&fixture_source());
    let up = one(&diags, "image.effective_ppi", "img.upscaled");
    assert_eq!(up.severity, Severity::Warning);
    assert!(up.message.contains("prints at 48 ppi"), "{}", up.message);
    assert!(
        up.message.contains("replaces image.upscale"),
        "{}",
        up.message
    );
    assert!(
        find(&diags, "image.upscale", "img.upscaled").is_empty(),
        "no double report on a print page: {diags:#?}"
    );
}

#[test]
fn screen_page_stays_silent() {
    let (diags, _) = validate(&fixture_source());
    for id in ["img.screen", "img.screen_up", "text.screen", "rect.screen"] {
        for code in PRINT_CODES {
            assert!(
                find(&diags, code, id).is_empty(),
                "{code} on {id}: {diags:#?}"
            );
        }
    }
    // The screen page keeps its existing upscale advisory.
    one(&diags, "image.upscale", "img.screen_up");
}

#[test]
fn small_rich_black_text_warns() {
    let (diags, _) = validate(&fixture_source());
    let small = one(&diags, "ink.rich_black", "text.small");
    assert_eq!(small.severity, Severity::Warning);
    assert_eq!(
        small.message,
        "text 'text.small' is 12px rich black cmyk(60,40,40,100) under the 24px (18pt) floor — \
         fill it with K only, cmyk(0,0,0,100), or set font-size to at least 24px"
    );
    assert!(
        find(&diags, "ink.rich_black", "text.large").is_empty(),
        "32 px text is silent: {diags:#?}"
    );
}

#[test]
fn total_coverage_over_300_warns() {
    let (diags, _) = validate(&fixture_source());
    let heavy = one(&diags, "ink.total_coverage", "rect.heavy");
    assert_eq!(heavy.severity, Severity::Warning);
    assert_eq!(
        heavy.message,
        "rect 'rect.heavy' fill cmyk(80,70,70,100) totals 320% ink > 300% limit — lower C, M, \
         or Y, for example cmyk(72,63,63,100) at 298%"
    );
    assert!(find(&diags, "ink.total_coverage", "rect.ok").is_empty());
    // 240% rich black text is within the limit.
    assert!(find(&diags, "ink.total_coverage", "text.small").is_empty());
}

#[test]
fn allow_suppresses_every_print_code() {
    let src = fixture_source().replace(
        "  styles {}",
        "  styles {}\n  diagnostics {\n    allow \"image.effective_ppi\"\n    \
         allow \"image.effective_ppi_target\"\n    allow \"ink.rich_black\"\n    allow \"ink.total_coverage\"\n  }",
    );
    let (diags, exit) = validate(&src);
    assert_eq!(exit, 0, "{diags:#?}");
    for code in PRINT_CODES {
        assert!(
            diags.iter().all(|d| d.code != code),
            "{code} must be allowed away: {diags:#?}"
        );
    }
}

#[test]
fn cmyk_colorspace_makes_every_page_a_print_page() {
    let src = fixture_source().replace(
        "zenith version=1 {",
        "zenith version=1 colorspace=\"cmyk\" {",
    );
    let (diags, _) = validate(&src);
    one(&diags, "image.effective_ppi", "img.screen");
    one(&diags, "ink.rich_black", "text.screen");
    one(&diags, "ink.total_coverage", "rect.screen");
    assert!(find(&diags, "image.upscale", "img.screen_up").is_empty());
}

#[test]
fn render_reports_the_same_print_diagnostics() {
    let fs = fixture_fs();
    let config = FsConfig::new(&fs, None);
    let host = Host::new(&fs, &config);
    let flags = PolicyFlags::default();
    let out = render_png_pages(
        host,
        &fixture_source(),
        Some(Path::new("fx")),
        RenderOptions::new(&flags),
    )
    .expect("render");
    one(&out.diagnostics, "image.effective_ppi", "img.low");
    one(&out.diagnostics, "ink.rich_black", "text.small");
    one(&out.diagnostics, "ink.total_coverage", "rect.heavy");
}

/// `bytes` with the OS/2 `fsType` field set to `fs_type`.
fn with_fs_type(bytes: &[u8], fs_type: u16) -> Vec<u8> {
    let mut out = bytes.to_vec();
    let num_tables = u16::from_be_bytes([out[4], out[5]]) as usize;
    for i in 0..num_tables {
        let rec = 12 + i * 16;
        if &out[rec..rec + 4] == b"OS/2" {
            let off = u32::from_be_bytes([out[rec + 8], out[rec + 9], out[rec + 10], out[rec + 11]])
                as usize;
            out[off + 8..off + 10].copy_from_slice(&fs_type.to_be_bytes());
            return out;
        }
    }
    panic!("face has no OS/2 table");
}

/// A screen document whose text uses a project font asset `licensed.ttf`.
const FONT_DOC: &str = r##"zenith version=1 {
  project id="proj.font" name="Font license"
  assets {
    asset id="font.licensed" kind="font" src="licensed.ttf"
  }
  tokens format="zenith-token-v1" {
    token id="size.body" type="dimension" value=(px)24
  }
  styles {}
  document id="doc.font" title="Font license" {
    page id="page.font" w=(px)400 h=(px)200 {
      text id="text.body" x=(px)10 y=(px)10 w=(px)380 h=(px)80 font-size=(token)"size.body" { span "Licensed" }
    }
  }
}
"##;

/// The bundled Noto Sans regular face.
fn noto_sans() -> &'static [u8] {
    bundled_faces()
        .iter()
        .find(|f| f.family == "Noto Sans" && f.weight == 400)
        .and_then(|f| f.bytes)
        .expect("bundled Noto Sans")
}

#[test]
fn restricted_font_reports_not_embeddable_at_validate() {
    let fs = MemFs::new().with("fx/licensed.ttf", with_fs_type(noto_sans(), 0x0002));
    let (diags, exit) = validate_with(&fs, FONT_DOC);
    assert_eq!(exit, 0, "{diags:#?}");
    let d = one(&diags, "font.not_embeddable", "text.body");
    assert_eq!(d.severity, Severity::Warning);
    assert!(
        d.message
            .contains("has a restricted license (fsType 0x0002)"),
        "{}",
        d.message
    );
    assert!(
        d.message.contains("used by text 'text.body'"),
        "{}",
        d.message
    );
}

#[test]
fn installable_font_reports_nothing() {
    let fs = MemFs::new().with("fx/licensed.ttf", noto_sans().to_vec());
    let (diags, _) = validate_with(&fs, FONT_DOC);
    assert!(
        diags.iter().all(|d| d.code != "font.not_embeddable"),
        "{diags:#?}"
    );
}
