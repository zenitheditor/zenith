//! Text in Chinese, Korean, Japanese, Arabic, Hebrew, and emoji renders
//! through the fallback chain with no `font.glyph_missing`, in PNG, SVG,
//! and PDF output.

use zenith_core::Diagnostic;
use zenith_fonts_fallback::fallback_faces;
use zenith_pipeline::render::{
    render_pdf_pages, render_png_pages, render_scene_json, render_svg_pages,
};
use zenith_pipeline::{FsConfig, Host, MemFs, PolicyFlags, RenderOptions};

const DOC: &str = r##"zenith version=1 {
  project id="proj.fallback" name="Fallback"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.fallback" title="Fallback" {
    page id="page.fallback" w=(px)400 h=(px)400 {
      text id="t.zh" x=(px)10 y=(px)10 w=(px)380 h=(px)50 { span "你好" }
      text id="t.ko" x=(px)10 y=(px)70 w=(px)380 h=(px)50 { span "안녕" }
      text id="t.ja" x=(px)10 y=(px)130 w=(px)380 h=(px)50 { span "こんにちは" }
      text id="t.ar" x=(px)10 y=(px)190 w=(px)380 h=(px)50 { span "عربى" }
      text id="t.he" x=(px)10 y=(px)250 w=(px)380 h=(px)50 { span "עברית" }
      text id="t.emoji" x=(px)10 y=(px)310 w=(px)380 h=(px)50 { span "😀" }
    }
  }
}
"##;

fn assert_no_missing_glyph(format: &str, diagnostics: &[Diagnostic]) {
    let missing: Vec<&Diagnostic> = diagnostics
        .iter()
        .filter(|d| d.code == "font.glyph_missing")
        .collect();
    assert!(missing.is_empty(), "{format}: {missing:?}");
}

#[test]
fn fallback_chain_covers_every_script_in_png_svg_and_pdf() {
    let fs = MemFs::new();
    let config = FsConfig::new(&fs, None);
    let host = Host::new(&fs, &config);
    let flags = PolicyFlags::default();
    let opts = RenderOptions::new(&flags);

    let png = render_png_pages(host, DOC, None, opts).expect("png renders");
    assert_no_missing_glyph("png", &png.diagnostics);
    let svg = render_svg_pages(host, DOC, None, opts).expect("svg renders");
    assert_no_missing_glyph("svg", &svg.diagnostics);
    let pdf = render_pdf_pages(host, DOC, None, opts).expect("pdf renders");
    assert_no_missing_glyph("pdf", &pdf.diagnostics);
}

#[test]
fn every_fallback_face_shapes_a_run() {
    let fs = MemFs::new();
    let config = FsConfig::new(&fs, None);
    let host = Host::new(&fs, &config);
    let flags = PolicyFlags::default();
    let opts = RenderOptions::new(&flags);

    let scene = render_scene_json(host, DOC, None, 1, opts).expect("scene");
    for face in fallback_faces() {
        assert!(
            scene.json.contains(face.id),
            "no run uses {} ({})",
            face.id,
            face.file
        );
    }
}
