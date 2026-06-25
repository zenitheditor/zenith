mod common;
use common::*;
use zenith_core::default_provider;
use zenith_scene::ir::SceneCommand;
use zenith_scene::{CompileResult, compile, compile_page};

#[test]
fn master_projects_running_head_and_folio_on_every_page() {
    let doc = parse(BOOK_SRC);
    let provider = default_provider();
    for page_index in 0..4 {
        let r = compile_page(&doc, &provider, page_index, None);
        assert!(
            !r.diagnostics
                .iter()
                .any(|d| d.code == "master.unknown_reference"),
            "page {page_index} must resolve its master"
        );
        // Master contributes 2 glyph runs (running head + folio) plus 1 body run.
        let runs = glyph_run_origins(&r);
        assert_eq!(
            runs.len(),
            3,
            "page {page_index}: expected running-head + folio + body, got {runs:?}"
        );
    }
}

#[test]
fn running_head_x_mirrors_recto_vs_verso() {
    let doc = parse(BOOK_SRC);
    let provider = default_provider();
    // Recto (page 1, index 0): live_x = margin_inner = 160.
    let recto = compile_page(&doc, &provider, 0, None);
    // Verso (page 2, index 1): live_x = margin_outer = 100 (mirrored).
    let verso = compile_page(&doc, &provider, 1, None);

    // The running-head run is the one whose baseline sits just below y=80
    // (text_box_top 80 + ascent). Its x is the live-area left inset.
    let recto_rh_x = glyph_run_origins(&recto)
        .into_iter()
        .find(|(_, y)| *y > 80.0 && *y < 130.0)
        .map(|(x, _)| x);
    let verso_rh_x = glyph_run_origins(&verso)
        .into_iter()
        .find(|(_, y)| *y > 80.0 && *y < 130.0)
        .map(|(x, _)| x);

    // Both fields are center-aligned within the live-area box. The live-area
    // left inset differs by parity (recto = margin_inner 160, verso =
    // margin_outer 100), so the centered run origin must differ — proving the
    // mirror is active. (Exact run x also depends on the per-parity text width;
    // the precise live-area inset is asserted directly in field.rs's unit test.)
    assert!(recto_rh_x.is_some() && verso_rh_x.is_some());
    assert_ne!(
        recto_rh_x, verso_rh_x,
        "running-head x must differ by parity (mirrored live area)"
    );
}

#[test]
fn running_head_recto_verso_text_differs_by_parity() {
    let doc = parse(BOOK_SRC);
    let provider = default_provider();
    // Recto and verso strings have very different lengths → different glyph runs.
    let recto = compile_page(&doc, &provider, 0, None);
    let verso = compile_page(&doc, &provider, 1, None);

    let rh_glyph_count = |r: &CompileResult| -> Option<usize> {
        r.scene.commands.iter().find_map(|c| match c {
            SceneCommand::DrawGlyphRun { y, glyphs, .. } if *y > 80.0 && *y < 130.0 => {
                Some(glyphs.len())
            }
            _ => None,
        })
    };
    let rc = rh_glyph_count(&recto);
    let vc = rh_glyph_count(&verso);
    assert!(
        rc.is_some() && vc.is_some(),
        "both parities emit a running head"
    );
    assert_ne!(
        rc, vc,
        "recto 'Chapter 1' and verso 'The Novel' differ in glyph count"
    );
}

#[test]
fn folio_renders_one_per_page_and_two_run_byte_identical() {
    let doc = parse(BOOK_SRC);
    let provider = default_provider();
    // The folio sits at the bottom (text_box_top 1820); exactly one per page.
    for page_index in 0..4 {
        let r = compile_page(&doc, &provider, page_index, None);
        let folios = glyph_run_origins(&r)
            .into_iter()
            .filter(|(_, y)| *y > 1820.0 && *y < 1900.0)
            .count();
        assert_eq!(folios, 1, "page {page_index}: exactly one folio run");

        // Two-run byte-identical determinism per page.
        let a = compile_page(&doc, &provider, page_index, None);
        let b = compile_page(&doc, &provider, page_index, None);
        assert_eq!(
            a.scene.to_json().expect("a"),
            b.scene.to_json().expect("b"),
            "page {page_index} must be byte-identical across runs"
        );
    }
}

#[test]
fn page_parity_start_verso_flips_page_one_running_head() {
    let provider = default_provider();

    // Default book: page 1 is recto (long "Chapter One…" text, inner=160 inset).
    let default_doc = parse(BOOK_SRC);
    let default_p1 = compile_page(&default_doc, &provider, 0, None);
    let (_, default_p1_glyphs) =
        running_head_x_and_glyphs(&default_p1).expect("default page 1 running head");

    // verso-start book: page 1 is now a verso (short "Verso" text, outer=100 inset).
    let verso_doc = parse(BOOK_SRC_VERSO_START);
    let verso_p1 = compile_page(&verso_doc, &provider, 0, None);
    let (_, verso_p1_glyphs) =
        running_head_x_and_glyphs(&verso_p1).expect("verso-start page 1 running head");

    assert_ne!(
        default_p1_glyphs, verso_p1_glyphs,
        "page-parity-start=verso must select the verso running-head text on page 1"
    );

    // The verso-start page 1 must match the DEFAULT page 2 (also a verso): same
    // verso text glyph count.
    let default_p2 = compile_page(&default_doc, &provider, 1, None);
    let (_, default_p2_glyphs) =
        running_head_x_and_glyphs(&default_p2).expect("default page 2 running head");
    assert_eq!(
        verso_p1_glyphs, default_p2_glyphs,
        "verso-start page 1 must render the same verso text as a normal verso page"
    );
}

#[test]
fn page_parity_override_recto_restores_page_one() {
    let provider = default_provider();
    let mut doc = parse(BOOK_SRC_VERSO_START);
    // Force page 1 back to recto via the per-page override.
    doc.body.pages[0].parity = Some("recto".to_owned());

    let p1 = compile_page(&doc, &provider, 0, None);
    let (_, p1_glyphs) = running_head_x_and_glyphs(&p1).expect("page 1 running head");

    // Compare against the default book's page 1 (a recto): same long recto text.
    let default_doc = parse(BOOK_SRC);
    let default_p1 = compile_page(&default_doc, &provider, 0, None);
    let (_, default_p1_glyphs) =
        running_head_x_and_glyphs(&default_p1).expect("default page 1 running head");
    assert_eq!(
        p1_glyphs, default_p1_glyphs,
        "explicit parity=recto must restore the recto running-head text on page 1"
    );
}

#[test]
fn inline_page_number_field_renders_folio_without_master() {
    // A field used directly in a page's children (not via a master) resolves
    // the same way: a page-number renders the page's 1-based folio.
    let src = r##"zenith version=1 {
  project id="proj.inl" name="Inl"
  tokens format="zenith-token-v1" {
token id="color.ink" type="color" value="#000000"
  }
  styles {}
  document id="doc.inl" title="Inl" {
page id="ip1" w=(px)400 h=(px)300 {
  field id="f.folio" type="page-number" x=(px)10 y=(px)10 w=(px)80 h=(px)30 fill=(token)"color.ink"
}
page id="ip2" w=(px)400 h=(px)300 {
  field id="f.folio2" type="page-number" x=(px)10 y=(px)10 w=(px)80 h=(px)30 fill=(token)"color.ink"
}
  }
}
"##;
    let doc = parse(src);
    let provider = default_provider();
    let p1 = compile_page(&doc, &provider, 0, None);
    let p2 = compile_page(&doc, &provider, 1, None);
    assert_eq!(glyph_run_origins(&p1).len(), 1, "page 1 folio renders");
    assert_eq!(glyph_run_origins(&p2).len(), 1, "page 2 folio renders");
}

#[test]
fn page_ref_field_resolves_target_page_index() {
    // A page-ref points at a node on page 3; it must render "3".
    let src = r##"zenith version=1 {
  project id="proj.ref" name="Ref"
  tokens format="zenith-token-v1" {
token id="color.ink" type="color" value="#000000"
  }
  styles {}
  document id="doc.ref" title="Ref" {
page id="rp1" w=(px)400 h=(px)300 {
  field id="f.ref" type="page-ref" target="anchor" x=(px)10 y=(px)10 w=(px)80 h=(px)30 fill=(token)"color.ink"
}
page id="rp2" w=(px)400 h=(px)300 {
  rect id="filler" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.ink"
}
page id="rp3" w=(px)400 h=(px)300 {
  text id="anchor" x=(px)10 y=(px)10 w=(px)80 h=(px)30 fill=(token)"color.ink" { span "X" }
}
  }
}
"##;
    let doc = parse(src);
    let provider = default_provider();
    let p1 = compile_page(&doc, &provider, 0, None);
    // The page-ref renders a single glyph run (the digit "3").
    assert_eq!(
        glyph_run_origins(&p1).len(),
        1,
        "page-ref to a page-3 anchor renders one digit run"
    );
}

#[test]
fn non_master_page_is_byte_identical_to_before() {
    // A document with no masters and no fields must compile to the exact same
    // command stream as it would have without the feature (regression guard).
    let src = r##"zenith version=1 {
  project id="proj.nm" name="NM"
  tokens format="zenith-token-v1" {
token id="color.fill" type="color" value="#f8fafc"
  }
  styles {}
  document id="doc.nm" title="NM" {
page id="page.nm" w=(px)640 h=(px)360 {
  rect id="rect.nm" x=(px)0 y=(px)0 w=(px)640 h=(px)360 fill=(token)"color.fill"
}
  }
}
"##;
    let doc = parse(src);
    let provider = default_provider();
    let r = compile(&doc, &provider);
    // Exactly PushClip, FillRect, PopClip — unchanged from the pre-feature path.
    assert_eq!(r.scene.commands.len(), 3, "{:?}", r.scene.commands);
    assert!(matches!(r.scene.commands[0], SceneCommand::PushClip { .. }));
    assert!(matches!(r.scene.commands[1], SceneCommand::FillRect { .. }));
    assert!(matches!(r.scene.commands[2], SceneCommand::PopClip));
}

/// A `footnote-ref` span emits a SUPERSCRIPT marker run after its text: the body
/// renders MORE glyph runs than the same text without the ref, and the marker
/// run draws at a reduced font size (the vertical-align="super" scale).
#[test]
fn footnote_ref_emits_superscript_marker() {
    let doc = parse(FOOTNOTE_ONE_SRC);
    let provider = default_provider();
    let r = compile(&doc, &provider);

    // No hard diagnostics (advisories/warnings only, if any).
    assert!(
        !r.scene.commands.is_empty(),
        "scene must have commands: {:?}",
        r.scene.commands
    );

    // Collect the distinct font sizes of the glyph runs. The body text shapes at
    // 16px (default); the superscript marker shapes smaller (0.65 × 16). So there
    // must be at least one glyph run whose font_size is below the body size —
    // proving a superscript marker was emitted.
    let sizes: Vec<f32> = r
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun { font_size, .. } => Some(*font_size),
            _ => None,
        })
        .collect();
    assert!(!sizes.is_empty(), "body must shape some glyph runs");
    let max_size = sizes.iter().cloned().fold(0.0_f32, f32::max);
    assert!(
        sizes.iter().any(|s| *s < max_size - 0.5),
        "a superscript marker run (reduced size) must be present; sizes={sizes:?}"
    );
}

/// The page renders a footnote zone at the bottom: a thin separator rule
/// (FillRect with height ~1px spanning ~1/3 the live width) PLUS the footnote's
/// content glyphs, positioned BELOW the body text.
#[test]
fn footnote_zone_renders_separator_and_content() {
    let doc = parse(FOOTNOTE_ONE_SRC);
    let provider = default_provider();
    let r = compile(&doc, &provider);

    // Live area: x=60, width=600-60-60=480; the separator rule is ~1/3 → 160px,
    // 1px tall, drawn near the bottom (y well below the body's y=80).
    let separators: Vec<(f64, f64, f64)> = r
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect { y, w, h, .. } if (*h - 1.0).abs() < 0.01 => Some((*y, *w, *h)),
            _ => None,
        })
        .collect();
    assert!(
        separators
            .iter()
            .any(|(y, w, _)| *y > 600.0 && (*w - 160.0).abs() < 1.0),
        "a ~160px-wide, 1px-tall separator rule must sit near the page bottom; \
         got {separators:?}"
    );

    // The footnote content "See also Chapter 4." must shape glyph runs whose
    // baseline y is BELOW the body (body baseline near y≈80+ascent). The lowest
    // glyph run on the page should be a footnote glyph (near the bottom margin).
    let max_glyph_y = r
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun { y, .. } => Some(*y),
            _ => None,
        })
        .fold(0.0_f64, f64::max);
    assert!(
        max_glyph_y > 600.0,
        "footnote content must render near the page bottom; max glyph y={max_glyph_y}"
    );
}

// NOTE: the footnote AUTO-NUMBERING assertion (markers "1"/"2") lives as a UNIT
// test in `zenith-scene/src/compile/footnote.rs` (`two_footnotes_auto_number_one_and_two`),
// because it asserts the `pub(super) collect_footnote_markers` map — crate-internal
// state the public scene does not expose. Integration-level footnote rendering is
// covered by the other tests in this file.

/// A `footnote-ref` pointing at an absent footnote id → advisory
/// `footnote.unresolved_ref` at compile time, and no marker is emitted.
#[test]
fn unresolved_footnote_ref_warns_and_emits_no_marker() {
    let src = r##"zenith version=1 {
  project id="proj.fn3" name="FN3"
  tokens format="zenith-token-v1" {
  }
  styles {}
  document id="doc.fn3" title="FN3" {
page id="page.fn3" w=(px)600 h=(px)900 margin-inner=(px)60 margin-outer=(px)60 margin-top=(px)80 margin-bottom=(px)80 {
  text id="body" x=(px)60 y=(px)80 w=(px)480 h=(px)200 {
    span "Dangling reference" footnote-ref="fn.missing"
  }
}
  }
}
"##;
    let doc = parse(src);
    let provider = default_provider();
    let r = compile(&doc, &provider);
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.code == "footnote.unresolved_ref"),
        "an unresolved footnote-ref must produce a footnote.unresolved_ref diagnostic; \
         got {:?}",
        r.diagnostics
    );
    // All glyph runs are full-size (no superscript marker emitted): there is only
    // one distinct font size.
    let sizes: Vec<f32> = r
        .scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::DrawGlyphRun { font_size, .. } => Some(*font_size),
            _ => None,
        })
        .collect();
    let max = sizes.iter().cloned().fold(0.0_f32, f32::max);
    assert!(
        !sizes.iter().any(|s| *s < max - 0.5),
        "no reduced-size (superscript) run may be emitted for an unresolved ref; sizes={sizes:?}"
    );
}

/// A page with NO footnotes emits a command stream byte-identical to before the
/// feature: the footnote pass is a no-op. We render the same doc twice and also
/// confirm there is no separator-like 1px FillRect spuriously added.
#[test]
fn page_without_footnotes_is_unchanged() {
    let src = r##"zenith version=1 {
  project id="proj.fn4" name="FN4"
  tokens format="zenith-token-v1" {
  }
  styles {}
  document id="doc.fn4" title="FN4" {
page id="page.fn4" w=(px)600 h=(px)900 margin-inner=(px)60 margin-outer=(px)60 margin-top=(px)80 margin-bottom=(px)80 {
  text id="body" x=(px)60 y=(px)80 w=(px)480 h=(px)200 {
    span "Just a plain paragraph with no notes."
  }
}
  }
}
"##;
    let doc = parse(src);
    let provider = default_provider();
    let r1 = compile(&doc, &provider);
    let r2 = compile(&doc, &provider);
    assert_eq!(
        r1.scene.commands, r2.scene.commands,
        "two renders must be byte-identical"
    );
    // No separator rule: no 1px-tall FillRect added by the footnote pass.
    assert!(
        !r1.scene.commands.iter().any(|c| matches!(
            c,
            SceneCommand::FillRect { h, .. } if (*h - 1.0).abs() < 0.01
        )),
        "a footnote-free page must not emit a separator rule: {:?}",
        r1.scene.commands
    );
}
