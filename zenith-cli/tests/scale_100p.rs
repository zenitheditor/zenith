//! Opt-in 100-page validate & PDF render scale benchmark.
//!
//! Ignored by default so `cargo test --workspace` stays fast.
//! Run on demand with:
//! `cargo test -p zenith-tool --test scale_100p --release -- --ignored --nocapture`

use std::fmt::Write;
use std::time::Instant;

use zenith_pipeline::render::render_pdf_pages;
use zenith_pipeline::{PolicyFlags, RenderOptions, validate_source};

fn generate_100_page_doc() -> String {
    let mut s = String::with_capacity(64 * 1024);
    s.push_str(
        r##"zenith version=1 {
  project id="proj.scale100" name="Scale 100"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
    token id="color.card" type="color" value="#f1f5f9"
    token id="color.ink" type="color" value="#0f172a"
    token id="color.accent" type="color" value="#2563eb"
    token id="font.body" type="fontFamily" value="Noto Sans"
    token id="size.title" type="dimension" value=(px)24
    token id="size.body" type="dimension" value=(px)12
  }
  styles {}
  document id="doc.scale100" title="Scale 100" {
"##,
    );
    for p in 1..=100 {
        let _ = writeln!(
            s,
            r##"    page id="page.{p}" w=(px)595 h=(px)842 background=(token)"color.bg" {{
      rect id="p{p}.bar" x=(px)0 y=(px)0 w=(px)595 h=(px)12 fill=(token)"color.accent"
      rect id="p{p}.card" x=(px)48 y=(px)96 w=(px)499 h=(px)180 fill=(token)"color.card"
      text id="p{p}.h1" x=(px)48 y=(px)40 w=(px)499 h=(px)36 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.title" {{ span "Section {p} — Production Report" }}
      text id="p{p}.body" x=(px)64 y=(px)116 w=(px)467 h=(px)140 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {{ span "Deterministic 100-page validation and vector PDF benchmark page {p}. Every page shares tokens and compiles through the standard pipeline." }}
    }}"##
        );
    }
    s.push_str("  }\n}\n");
    s
}

#[test]
#[ignore = "opt-in 100-page scale benchmark; run with -- --ignored --nocapture"]
fn benchmark_100_page_validate_and_pdf_render() {
    let src = generate_100_page_doc();
    let flags = PolicyFlags::default();

    let t0 = Instant::now();
    let val = validate_source(zenith_cli::native::host(), &src, None, &flags);
    let validate_ms = t0.elapsed().as_millis();
    assert!(
        !zenith_core::Diagnostic::has_errors(&val.diagnostics),
        "100-page doc must have zero errors"
    );

    let t1 = Instant::now();
    let pdf = render_pdf_pages(
        zenith_cli::native::host(),
        &src,
        None,
        RenderOptions::new(&flags),
    )
    .expect("100-page PDF render must succeed");
    let render_ms = t1.elapsed().as_millis();

    eprintln!(
        "100-page benchmark: source={} B, validate={} ms, pdf_render={} ms, pdf={} B",
        src.len(),
        validate_ms,
        render_ms,
        pdf.pdf.len()
    );
    assert!(pdf.pdf.starts_with(b"%PDF-"));
}
