//! PDF page units: one document px is 0.75 pt, so an A4 design is an A4 PDF.
//!
//! The fixture holds a gradient, an image, text with a link, and a bottom bar.
//! Page boxes and the link `/Rect` are in points.
//! Content stream operands stay in px under one scale `cm`.

use tempfile::TempDir;
use zenith_cli::commands::render::to_pdf_with_dir;
use zenith_pipeline::PolicyFlags;

const DOC: &str = r##"zenith version=1 {
  project id="proj.units" name="Units"
  assets {
    asset id="asset.photo" kind="image" src="photo.png"
  }
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.ink" type="color" value="#1e293b"
    token id="color.a" type="color" value="#1e3a8a"
    token id="color.b" type="color" value="#60a5fa"
    token id="size.title" type="dimension" value=(px)48
    token id="grad.sky" type="gradient" angle=(deg)90 {
      stop offset=0.0 color=(token)"color.a"
      stop offset=1.0 color=(token)"color.b"
    }
  }
  styles {}
  document id="doc.units" title="Units" {
    page id="page.a4" w=(px)794 h=(px)1123 background=(token)"color.bg"BLEED {
      rect id="sky" x=(px)40 y=(px)40 w=(px)714 h=(px)300 fill=(token)"grad.sky"
      image id="photo" asset="asset.photo" x=(px)40 y=(px)380 w=(px)360 h=(px)240
      text id="title" x=(px)40 y=(px)680 w=(px)700 h=(px)80 font-size=(token)"size.title" fill=(token)"color.ink" {
        span "Visit "
        span "zenith" link="https://zenith.example/"
      }
      rect id="edge" x=(px)0 y=(px)1083 w=(px)794 h=(px)40 fill=(token)"color.ink"
    }
  }
}
"##;

/// Render the fixture page to PDF text, with `bleed` spliced into the page node.
fn render(bleed: &str) -> String {
    let dir = TempDir::new().expect("tempdir");
    let pixmap = tiny_skia::Pixmap::new(6, 4).expect("pixmap");
    std::fs::write(
        dir.path().join("photo.png"),
        pixmap.encode_png().expect("png"),
    )
    .expect("write png");
    let src = DOC.replace("BLEED", bleed);
    let artifact = to_pdf_with_dir(
        &src,
        Some(dir.path()),
        1,
        false,
        true,
        &PolicyFlags::default(),
        None,
    )
    .expect("pdf render");
    String::from_utf8_lossy(&artifact.pdf).into_owned()
}

/// The numbers of the first `/<key> [ ... ]` array in `text`.
fn array(text: &str, key: &str) -> Vec<f64> {
    let needle = format!("/{key} [");
    let start = text.find(&needle).expect("key present") + needle.len();
    let end = start + text[start..].find(']').expect("array closes");
    text[start..end]
        .split_whitespace()
        .map(|v| v.parse::<f64>().expect("number"))
        .collect()
}

fn assert_close(got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "got {got:?}, want {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() < 1e-3, "got {got:?}, want {want:?}");
    }
}

#[test]
fn a4_page_boxes_are_a4_in_points() {
    let text = render("");
    // 794 px * 0.75 = 595.5 pt (210 mm). 1123 px * 0.75 = 842.25 pt (297 mm).
    for key in ["MediaBox", "TrimBox", "BleedBox", "CropBox"] {
        assert_close(&array(&text, key), &[0.0, 0.0, 595.5, 842.25]);
    }
}

#[test]
fn a4_with_3mm_bleed_has_a4_trim_box() {
    // 3 mm = 3 / 25.4 * 72 = 8.504 pt.
    let text = render(" bleed=(pt)8.504");
    let b = 8.504;
    let media = [0.0, 0.0, 595.5 + 2.0 * b, 842.25 + 2.0 * b];
    assert_close(&array(&text, "MediaBox"), &media);
    assert_close(&array(&text, "BleedBox"), &media);
    assert_close(&array(&text, "CropBox"), &media);
    assert_close(&array(&text, "TrimBox"), &[b, b, 595.5 + b, 842.25 + b]);
}

#[test]
fn content_keeps_px_geometry_under_one_scale() {
    let text = render("");
    let start = text.find("0.75 0 0 0.75 0 0 cm\n").expect("scale cm");
    let stream = &text[start..];
    assert!(
        stream.starts_with("0.75 0 0 0.75 0 0 cm\n1 0 0 -1 0 1123 cm\n"),
        "the scale cm must come first, then the px flip"
    );
    // Gradient, image, and bottom bar keep their authored px boxes.
    assert!(stream.contains("40 40 714 300 re\nW\nn\n/sh0 sh\n"));
    assert!(stream.contains("360 0 0 -240 40 620 cm\n/im0 Do\n"));
    assert!(stream.contains("0 1083 794 40 re\n"));
    assert!(text.contains("/ShadingType 2"));
    assert!(text.contains("/Subtype /Image"));
    // Text stays selectable: an embedded Type0 font with a ToUnicode map.
    assert!(text.contains("/Type0"));
    assert!(text.contains("/ToUnicode"));
    assert!(stream.contains("/f0 48 Tf\n1 0 0 -1 40 "));
}

#[test]
fn link_rect_is_in_points() {
    let text = render("");
    let rect = array(&text, "Rect");
    assert_eq!(rect.len(), 4);
    // The run's ascent top sits at the text box top: (1123 - 680) * 0.75 = 332.25 pt.
    assert!((rect[3] - 332.25).abs() < 1e-3, "{rect:?}");
    // The link starts after "Visit " and stays inside the 40..740 px text box.
    assert!(rect[0] > 40.0 * 0.75 && rect[2] < 740.0 * 0.75, "{rect:?}");
    assert!(rect[1] < rect[3] && rect[0] < rect[2], "{rect:?}");
}
