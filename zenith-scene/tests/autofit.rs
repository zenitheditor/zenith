mod common;
use common::parse;
use zenith_core::default_provider;
use zenith_scene::ir::SceneCommand;
use zenith_scene::{CompileResult, compile};

/// Collect the font_size of every DrawGlyphRun; returns the first run's size.
fn first_glyph_font_size(result: &CompileResult) -> Option<f32> {
    result.scene.commands.iter().find_map(|c| match c {
        SceneCommand::DrawGlyphRun { font_size, .. } => Some(*font_size),
        _ => None,
    })
}

/// A long single-span title in a box too short at the declared 96px size but
/// fittable when shrunk, with a low font-size-min → NO `text.fit_failed`, and
/// the emitted DrawGlyphRun font_size is SMALLER than declared and ≥ the floor.
#[test]
fn autofit_shrinks_long_title_to_fit() {
    let src = r##"zenith version=1 {
  project id="proj.af1" name="AF1"
  tokens format="zenith-token-v1" {
token id="font.body"     type="fontFamily" value="Noto Sans"
token id="size.title"    type="dimension"  value=(px)96
token id="size.title.min" type="dimension" value=(px)12
  }
  styles {}
  document id="doc.af1" title="AF1" {
page id="page.af1" w=(px)1920 h=(px)1080 {
  text id="slide.title" x=(px)160 y=(px)60 w=(px)1600 h=(px)200 overflow="autofit" font-family=(token)"font.body" font-size=(token)"size.title" font-size-min=(token)"size.title.min" {
    span "Comprehensive Analysis of Cross-Regional Supply Chain Disruption Impacts on Quarterly Revenue"
  }
}
  }
}
"##;
    let result = compile(&parse(src), &default_provider());

    let fit_failed: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "text.fit_failed")
        .collect();
    assert!(
        fit_failed.is_empty(),
        "autofit must shrink to fit, not fail; got {:?}",
        fit_failed
    );

    let fs = first_glyph_font_size(&result).expect("a DrawGlyphRun must be emitted");
    assert!(
        fs < 96.0,
        "autofit must reduce the font below the declared 96px; got {fs}"
    );
    assert!(
        fs >= 12.0,
        "autofit must not shrink below the 12px floor; got {fs}"
    );
}

/// A short title that already fits at its declared size renders at that size
/// (emitted font_size == declared), with no fit_failed.
#[test]
fn autofit_that_fits_at_declared_size_keeps_it() {
    let src = r##"zenith version=1 {
  project id="proj.af2" name="AF2"
  tokens format="zenith-token-v1" {
token id="font.body"     type="fontFamily" value="Noto Sans"
token id="size.title"    type="dimension"  value=(px)48
token id="size.title.min" type="dimension" value=(px)12
  }
  styles {}
  document id="doc.af2" title="AF2" {
page id="page.af2" w=(px)1920 h=(px)1080 {
  text id="slide.title" x=(px)160 y=(px)60 w=(px)1600 h=(px)400 overflow="autofit" font-family=(token)"font.body" font-size=(token)"size.title" font-size-min=(token)"size.title.min" {
    span "Short Title"
  }
}
  }
}
"##;
    let result = compile(&parse(src), &default_provider());

    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.code != "text.fit_failed"),
        "a fitting title must not fail; got {:?}",
        result.diagnostics
    );
    let fs = first_glyph_font_size(&result).expect("a DrawGlyphRun must be emitted");
    assert_eq!(
        fs, 48.0,
        "a title that fits at its declared size keeps that size; got {fs}"
    );
}

/// Content so long it overflows even at the font-size-min → exactly one
/// `text.fit_failed` for the node, emitted at the floor size.
#[test]
fn autofit_floor_still_overflows_emits_fit_failed() {
    // Box only 30px tall: even a 12px-min title of many words overflows.
    let src = r##"zenith version=1 {
  project id="proj.af3" name="AF3"
  tokens format="zenith-token-v1" {
token id="font.body"     type="fontFamily" value="Noto Sans"
token id="size.title"    type="dimension"  value=(px)96
token id="size.title.min" type="dimension" value=(px)40
  }
  styles {}
  document id="doc.af3" title="AF3" {
page id="page.af3" w=(px)1920 h=(px)1080 {
  text id="slide.title" x=(px)160 y=(px)60 w=(px)300 h=(px)30 overflow="autofit" font-family=(token)"font.body" font-size=(token)"size.title" font-size-min=(token)"size.title.min" {
    span "Comprehensive Analysis of Cross-Regional Supply Chain Disruption Impacts on Quarterly Revenue Across Every Market"
  }
}
  }
}
"##;
    let result = compile(&parse(src), &default_provider());

    let fit_failed: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "text.fit_failed" && d.subject_id.as_deref() == Some("slide.title"))
        .collect();
    assert_eq!(
        fit_failed.len(),
        1,
        "exactly one fit_failed expected at the floor; got {:?}",
        result.diagnostics
    );
}

/// An `overflow="clip"` node and an `overflow="fit"` node emit the SAME command
/// vec whether or not the autofit wrapper exists (pass-through proof): the
/// wrapper forwards non-autofit nodes to `compile_text_sized` unchanged.
#[test]
fn non_autofit_is_byte_identical() {
    // Two compiles of the same clip node must be byte-identical (determinism),
    // and the presence of font-size-min on a non-autofit node must NOT change
    // the emitted command stream vs. an identical node without it.
    let clip_src = r##"zenith version=1 {
  project id="proj.af4" name="AF4"
  tokens format="zenith-token-v1" {
token id="font.body"  type="fontFamily" value="Noto Sans"
token id="size.title" type="dimension"  value=(px)96
token id="size.min"   type="dimension"  value=(px)12
  }
  styles {}
  document id="doc.af4" title="AF4" {
page id="page.af4" w=(px)1920 h=(px)1080 {
  text id="slide.title" x=(px)160 y=(px)60 w=(px)1600 h=(px)200 overflow="clip" font-family=(token)"font.body" font-size=(token)"size.title" {
    span "Comprehensive Analysis of Cross-Regional Supply Chain Disruption Impacts"
  }
}
  }
}
"##;
    // Same node but with font-size-min added — a non-autofit node must ignore
    // it and emit the identical command stream.
    let clip_with_min_src = clip_src.replace(
        r#"font-size=(token)"size.title""#,
        r#"font-size=(token)"size.title" font-size-min=(token)"size.min""#,
    );

    let a = compile(&parse(clip_src), &default_provider());
    let b = compile(&parse(&clip_with_min_src), &default_provider());
    assert_eq!(
        a.scene.commands, b.scene.commands,
        "a non-autofit (clip) node must emit an identical command stream with or \
         without font-size-min"
    );

    // The overflow="fit" variant likewise passes through unchanged.
    let fit_src = clip_src.replace(r#"overflow="clip""#, r#"overflow="fit""#);
    let fit_with_min_src = clip_with_min_src.replace(r#"overflow="clip""#, r#"overflow="fit""#);
    let fa = compile(&parse(&fit_src), &default_provider());
    let fb = compile(&parse(&fit_with_min_src), &default_provider());
    assert_eq!(
        fa.scene.commands, fb.scene.commands,
        "a non-autofit (fit) node must emit an identical command stream with or \
         without font-size-min"
    );
}
