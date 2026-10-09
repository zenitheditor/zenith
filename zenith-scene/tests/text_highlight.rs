mod common;
use common::parse;
use zenith_core::default_provider;
use zenith_scene::compile;
use zenith_scene::ir::{Paint, SceneCommand};

// ── Control: span WITHOUT highlight emits no background FillRect ─────────────

/// A plain span (no `highlight` attribute) must compile to exactly one
/// DrawGlyphRun (inside the PushClip/PopClip bracket) with no FillRect. This
/// proves the highlight machinery is additive and byte-identical when absent.
#[test]
fn span_without_highlight_emits_no_fill_rect() {
    let src = r##"zenith version=1 {
  project id="proj.hl0" name="HL0"
  tokens format="zenith-token-v1" {
token id="color.ink"  type="color"      value="#111827"
token id="font.body"  type="fontFamily" value="Noto Sans"
token id="size.body"  type="dimension"  value=(px)24
  }
  styles {}
  document id="doc.hl0" title="HL0" {
page id="page.hl0" w=(px)400 h=(px)200 {
  text id="t.hl0" x=(px)10 y=(px)20 w=(px)380 h=(px)60 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
    span "Hello"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    // No unshaped diagnostics.
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.code != "scene.text_unshaped"),
        "no text_unshaped diagnostics expected; got: {:?}",
        result.diagnostics
    );

    // No FillRect at all — the plain span must not emit a background box.
    let rects: Vec<_> = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
        .collect();
    assert!(
        rects.is_empty(),
        "span without highlight must emit no FillRect; got {} rect(s)",
        rects.len()
    );

    // Exactly one DrawGlyphRun (inside PushClip/PopClip).
    let runs: Vec<_> = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }))
        .collect();
    assert_eq!(
        runs.len(),
        1,
        "expected exactly one DrawGlyphRun; got {}",
        runs.len()
    );
}

// ── Span WITH highlight emits a background FillRect before the glyph run ─────

/// A span with `highlight=(token)"color.mark"` must emit:
///   1. A `FillRect` with the resolved highlight color (before the glyph run).
///   2. A `DrawGlyphRun` with the resolved text color.
///
/// The FillRect must appear BEFORE the DrawGlyphRun in the command stream so
/// that glyphs paint on top of the background.
#[test]
fn span_with_highlight_emits_fill_rect_before_glyph_run() {
    // color.mark = #FFFF00 (bright yellow): r=255, g=255, b=0.
    // color.ink  = #111827 (near-black): r=17, g=24, b=39.
    let src = r##"zenith version=1 {
  project id="proj.hl1" name="HL1"
  tokens format="zenith-token-v1" {
token id="color.ink"  type="color"      value="#111827"
token id="color.mark" type="color"      value="#FFFF00"
token id="font.body"  type="fontFamily" value="Noto Sans"
token id="size.body"  type="dimension"  value=(px)24
  }
  styles {}
  document id="doc.hl1" title="HL1" {
page id="page.hl1" w=(px)400 h=(px)200 {
  text id="t.hl1" x=(px)10 y=(px)20 w=(px)380 h=(px)60 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
    span "Hello" highlight=(token)"color.mark"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    // No unshaped diagnostics.
    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.code != "scene.text_unshaped"),
        "no text_unshaped diagnostics expected; got: {:?}",
        result.diagnostics
    );

    // Collect command positions, ignoring PushClip/PopClip wrappers.
    let significant: Vec<&SceneCommand> = result
        .scene
        .commands
        .iter()
        .filter(|c| {
            matches!(
                c,
                SceneCommand::FillRect { .. } | SceneCommand::DrawGlyphRun { .. }
            )
        })
        .collect();

    assert_eq!(
        significant.len(),
        2,
        "expected exactly 1 FillRect + 1 DrawGlyphRun; got: {:?}",
        significant
    );

    // First significant command must be a FillRect with the highlight color.
    match significant[0] {
        SceneCommand::FillRect {
            paint: Paint::Solid { color },
            x,
            w,
            h,
            ..
        } => {
            // Highlight color: #FFFF00 → r=255, g=255, b=0.
            assert_eq!(color.r, 0xFF, "highlight rect r must be 0xFF (yellow)");
            assert_eq!(color.g, 0xFF, "highlight rect g must be 0xFF (yellow)");
            assert_eq!(color.b, 0x00, "highlight rect b must be 0x00 (yellow)");
            // Rect must be positioned at the text-box x origin.
            assert_eq!(*x, 10.0, "highlight rect x must be text-box origin (10px)");
            // Must have positive width and height.
            assert!(*w > 0.0, "highlight rect width must be > 0; got {w}");
            assert!(*h > 0.0, "highlight rect height must be > 0; got {h}");
        }
        other => panic!("expected FillRect (highlight), got {other:?}"),
    }

    // Second significant command must be a DrawGlyphRun with the text fill color.
    match significant[1] {
        SceneCommand::DrawGlyphRun { color, x, .. } => {
            // Text color: #111827 → r=0x11=17, g=0x18=24, b=0x27=39.
            assert_eq!(color.r, 0x11, "glyph color.r must be 0x11 (ink)");
            assert_eq!(color.g, 0x18, "glyph color.g must be 0x18 (ink)");
            assert_eq!(color.b, 0x27, "glyph color.b must be 0x27 (ink)");
            // Glyph run must start at the same x origin as the highlight rect.
            assert_eq!(*x, 10.0, "glyph run x must match text-box origin (10px)");
        }
        other => panic!("expected DrawGlyphRun after highlight FillRect; got {other:?}"),
    }
}

// ── Mixed spans: only the highlighted span gets a background rect ─────────────

/// Two spans on the same text node — first plain, second highlighted. Exactly
/// one FillRect must appear (for the second span), positioned after the first
/// glyph run (which has non-zero advance) and before the second glyph run.
#[test]
fn only_highlighted_span_gets_fill_rect() {
    // color.hi = #00FF00 (green): r=0, g=255, b=0.
    let src = r##"zenith version=1 {
  project id="proj.hl2" name="HL2"
  tokens format="zenith-token-v1" {
token id="color.ink" type="color"      value="#111827"
token id="color.hi"  type="color"      value="#00FF00"
token id="font.body" type="fontFamily" value="Noto Sans"
token id="size.body" type="dimension"  value=(px)24
  }
  styles {}
  document id="doc.hl2" title="HL2" {
page id="page.hl2" w=(px)800 h=(px)200 {
  text id="t.hl2" x=(px)10 y=(px)20 w=(px)780 h=(px)60 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" {
    span "plain "
    span "highlighted" highlight=(token)"color.hi"
  }
}
  }
}
"##;
    let doc = parse(src);
    let result = compile(&doc, &default_provider());

    assert!(
        result
            .diagnostics
            .iter()
            .all(|d| d.code != "scene.text_unshaped"),
        "no text_unshaped diagnostics; got: {:?}",
        result.diagnostics
    );

    // Collect in emission order (excluding clip wrappers).
    let significant: Vec<&SceneCommand> = result
        .scene
        .commands
        .iter()
        .filter(|c| {
            matches!(
                c,
                SceneCommand::FillRect { .. } | SceneCommand::DrawGlyphRun { .. }
            )
        })
        .collect();

    // Expected order: DrawGlyphRun (plain), FillRect (highlight), DrawGlyphRun (highlighted).
    assert_eq!(
        significant.len(),
        3,
        "expected 2 DrawGlyphRun + 1 FillRect; got: {:?}",
        significant
    );

    // [0] first glyph run (plain span — no FillRect before it).
    assert!(
        matches!(significant[0], SceneCommand::DrawGlyphRun { .. }),
        "first significant command must be DrawGlyphRun (plain span); got {:?}",
        significant[0]
    );

    // [1] highlight FillRect for the second span.
    match significant[1] {
        SceneCommand::FillRect {
            paint: Paint::Solid { color },
            x,
            ..
        } => {
            // Highlight color: #00FF00 → r=0, g=255, b=0.
            assert_eq!(color.r, 0x00, "highlight r must be 0x00 (green)");
            assert_eq!(color.g, 0xFF, "highlight g must be 0xFF (green)");
            assert_eq!(color.b, 0x00, "highlight b must be 0x00 (green)");
            // The highlight rect must start to the right of the text-box origin
            // because the first (plain) span has non-zero advance.
            assert!(
                *x > 10.0,
                "highlight rect x must be > 10 (after plain span advance); got {x}"
            );
        }
        other => panic!("expected FillRect (highlight) at index 1; got {other:?}"),
    }

    // [2] second glyph run (highlighted span).
    assert!(
        matches!(significant[2], SceneCommand::DrawGlyphRun { .. }),
        "third significant command must be DrawGlyphRun (highlighted span); got {:?}",
        significant[2]
    );
}
