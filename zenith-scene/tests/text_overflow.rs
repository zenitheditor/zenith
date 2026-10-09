mod common;
use common::parse;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::SceneCommand;

// ── overflow="fit" tests ──────────────────────────────────────────────────

/// A text node with `overflow="fit"` whose long text overflows the small box
/// height must produce a `text.fit_failed` Error diagnostic, AND must still
/// emit glyph run commands (the scene is not suppressed).
#[test]
fn overflow_fit_height_exceeded_emits_fit_failed_and_still_draws() {
    // A tiny 60×20 px box. Font size 16 px → line_height ≈ 18–20 px.
    // The text has many words that will wrap into multiple lines, so
    // content_height will exceed 20 px.
    let src = r##"zenith version=1 {
  project id="proj.fit1" name="Fit Overflow"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.fit1" title="Fit Overflow" {
page id="page.fit1" w=(px)400 h=(px)400 {
  text id="text.overflow" x=(px)10 y=(px)10 w=(px)60 h=(px)20 overflow="fit" {
    span "The quick brown fox jumps over the lazy dog and keeps on going"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    // Must have exactly one `text.fit_failed` Error diagnostic.
    let fit_errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "text.fit_failed")
        .collect();
    assert_eq!(
        fit_errors.len(),
        1,
        "expected exactly one text.fit_failed diagnostic; got: {:?}",
        result.diagnostics
    );
    assert_eq!(
        fit_errors[0].severity,
        zenith_core::Severity::Error,
        "text.fit_failed must be Error severity"
    );
    assert!(
        fit_errors[0]
            .subject_id
            .as_deref()
            .map(|s| s.contains("text.overflow"))
            .unwrap_or(false),
        "subject_id must name the overflowing text node; got {:?}",
        fit_errors[0].subject_id
    );

    // Glyph runs must still be emitted — the scene is not suppressed.
    let has_glyphs = result
        .scene
        .commands
        .iter()
        .any(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }));
    assert!(
        has_glyphs,
        "glyph runs must still be emitted even when fit fails"
    );
}

/// A text node with `overflow="clip"` whose long text overflows the small box
/// must produce a `text.overflow` Warning (clipping silently truncates ink, so
/// the author is told) — but still draw, and NOT hard-fail.
#[test]
fn overflow_clip_height_exceeded_emits_overflow_warning() {
    let src = r##"zenith version=1 {
  project id="proj.clip1" name="Clip Overflow"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.clip1" title="Clip Overflow" {
page id="page.clip1" w=(px)400 h=(px)400 {
  text id="text.clipped" x=(px)10 y=(px)10 w=(px)60 h=(px)20 overflow="clip" {
    span "The quick brown fox jumps over the lazy dog and keeps on going"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let warns: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "text.overflow")
        .collect();
    assert_eq!(
        warns.len(),
        1,
        "expected exactly one text.overflow warning; got: {:?}",
        result.diagnostics
    );
    assert_eq!(
        warns[0].severity,
        zenith_core::Severity::Warning,
        "text.overflow must be Warning severity (not a hard fail)"
    );
    // No hard error from clip overflow.
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|d| d.code == "text.fit_failed"),
        "clip overflow must NOT produce text.fit_failed"
    );
    // Glyph runs still emitted.
    assert!(
        result
            .scene
            .commands
            .iter()
            .any(|c| matches!(c, SceneCommand::DrawGlyphRun { .. })),
        "glyph runs must still be emitted when clip overflows"
    );
}

/// A text node with `overflow="fit"` whose text FITS within the box must
/// produce NO `text.fit_failed` diagnostic.
#[test]
fn overflow_fit_text_fits_no_diagnostic() {
    // A wide, tall box that the short text will easily fit in.
    let src = r##"zenith version=1 {
  project id="proj.fit2" name="Fit OK"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.fit2" title="Fit OK" {
page id="page.fit2" w=(px)400 h=(px)400 {
  text id="text.fits" x=(px)10 y=(px)10 w=(px)300 h=(px)100 overflow="fit" {
    span "Hi"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let fit_errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "text.fit_failed")
        .collect();
    assert!(
        fit_errors.is_empty(),
        "text that fits must produce no text.fit_failed diagnostic; got: {:?}",
        fit_errors
    );
}

/// A text node with `overflow="clip"` (not "fit") must NEVER produce a
/// `text.fit_failed` diagnostic, even when the text clearly overflows.
#[test]
fn overflow_clip_overflowing_text_no_fit_diagnostic() {
    let src = r##"zenith version=1 {
  project id="proj.fit3" name="Clip Overflow"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.fit3" title="Clip Overflow" {
page id="page.fit3" w=(px)400 h=(px)400 {
  text id="text.clip" x=(px)10 y=(px)10 w=(px)60 h=(px)20 overflow="clip" {
    span "The quick brown fox jumps over the lazy dog and keeps on going"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let fit_errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "text.fit_failed")
        .collect();
    assert!(
        fit_errors.is_empty(),
        "overflow=\"clip\" must never produce text.fit_failed; got: {:?}",
        fit_errors
    );
}

/// A text node with no `overflow` property and overflowing text must NOT
/// produce a `text.fit_failed` diagnostic.
#[test]
fn overflow_absent_overflowing_text_no_fit_diagnostic() {
    let src = r##"zenith version=1 {
  project id="proj.fit4" name="No Overflow Prop"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.fit4" title="No Overflow Prop" {
page id="page.fit4" w=(px)400 h=(px)400 {
  text id="text.noov" x=(px)10 y=(px)10 w=(px)60 h=(px)20 {
    span "The quick brown fox jumps over the lazy dog and keeps on going"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    let fit_errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "text.fit_failed")
        .collect();
    assert!(
        fit_errors.is_empty(),
        "absent overflow must never produce text.fit_failed; got: {:?}",
        fit_errors
    );
}
