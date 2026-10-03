//! Integration tests for `format="markdown"` on `text` nodes.
//!
//! Tests that:
//! 1. `format="markdown"` parses inline marks into styled spans (bold →
//!    heavier glyph run; highlight → FillRect before glyph run).
//! 2. A `text` WITHOUT `format` emitting the same literal `**bold**` source
//!    renders it verbatim — no styling, byte-identical to before.
//! 3. `format="markdown"` with a data-bound span (substituted by the data
//!    pre-pass) is also parsed as markdown.

mod common;
use common::*;
use zenith_core::{DataContext, default_provider};
use zenith_scene::{compile, compile_page, ir::SceneCommand};

// ── Fixture helpers ─────────────────────────────────────────────────

/// Minimal KDL fixture for a single-span `text` node with an optional
/// `format` attribute and configurable span text.
fn fixture(format_attr: &str, span_text: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.md" name="MD"
  tokens format="zenith-token-v1" {{
token id="color.ink"  type="color"      value="#111827"
token id="font.body"  type="fontFamily" value="Noto Sans"
token id="size.body"  type="dimension"  value=(px)24
  }}
  styles {{}}
  document id="doc.md" title="MD" {{
page id="page.md" w=(px)400 h=(px)200 {{
  text id="t.md" x=(px)10 y=(px)20 w=(px)380 h=(px)80 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body"{format_attr} {{
    span "{span_text}"
  }}
}}
  }}
}}"##
    )
}

// ── Test 1: format="markdown" produces multiple styled glyph runs ──

/// `**bold** and _italic_ and ==hi==` with `format="markdown"` must produce
/// at least two glyph runs (the styled segments are distinct spans) AND at
/// least one FillRect for the `==hi==` highlight.
///
/// Without `format="markdown"` the same text would render as ONE glyph run
/// containing the literal asterisks.
#[test]
fn markdown_format_parses_inline_marks_into_styled_spans() {
    let src = fixture(r#" format="markdown""#, r"**bold** and _italic_ and ==hi==");
    let doc = parse(&src);
    let result = compile(&doc, &default_provider());

    // No compilation errors.
    assert!(
        result.diagnostics.iter().all(|d| !d.is_error()),
        "expected no errors; got: {:?}",
        result.diagnostics
    );

    // Collect DrawGlyphRun and FillRect commands (exclude PushClip/PopClip).
    let glyph_runs: Vec<_> = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }))
        .collect();
    let fill_rects: Vec<_> = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
        .collect();

    // Parsed markdown must produce MORE than one glyph run (at least bold,
    // plain, italic, plain, highlight segments — ≥ 3 distinct spans).
    assert!(
        glyph_runs.len() >= 3,
        "expected >= 3 glyph runs for 'bold/italic/highlight' spans; got {}",
        glyph_runs.len()
    );

    // ==hi== must have emitted at least one FillRect (the highlight background).
    assert!(
        !fill_rects.is_empty(),
        "expected at least 1 FillRect for the ==hi== highlight; got 0"
    );
}

// ── Test 2: plain text (no format) renders literal asterisks verbatim ──

/// A `text` node WITHOUT `format` (or any format) using the same string
/// `**bold** and _italic_` must render the literal characters — no mark
/// parsing, one glyph run, no FillRect. This is the byte-identical control.
#[test]
fn no_format_attribute_renders_literal_asterisks_verbatim() {
    let src = fixture("", r"**bold** and _italic_ and ==hi==");
    let doc = parse(&src);
    let result = compile(&doc, &default_provider());

    // No errors.
    assert!(
        result.diagnostics.iter().all(|d| !d.is_error()),
        "expected no errors; got: {:?}",
        result.diagnostics
    );

    let glyph_runs: Vec<_> = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }))
        .collect();

    let fill_rects: Vec<_> = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
        .collect();

    // Literal rendering: one single glyph run (the whole string is one plain span).
    assert_eq!(
        glyph_runs.len(),
        1,
        "expected exactly 1 glyph run for literal text without format; got {}",
        glyph_runs.len()
    );

    // No FillRect — `==hi==` is literal text when not parsed as markdown.
    assert!(
        fill_rects.is_empty(),
        "expected 0 FillRects for literal text without format; got {}",
        fill_rects.len()
    );
}

// ── Test 3: format="markdown" with a data-bound span ─────────────────────

/// A `text format="markdown"` node with a `data-ref` span: after the
/// data-binding pre-pass substitutes the span text, the markdown pass should
/// re-parse the substituted text as markdown. In this test the data field
/// holds `**Hi** there`, so after parsing we expect >= 2 glyph runs (the
/// bold "Hi" and the plain " there").
#[test]
fn markdown_format_with_data_ref_parses_substituted_text() {
    let src = r##"zenith version=1 {
  project id="proj.mddr" name="MDDR"
  tokens format="zenith-token-v1" {
token id="color.ink"  type="color"      value="#111827"
token id="font.body"  type="fontFamily" value="Noto Sans"
token id="size.body"  type="dimension"  value=(px)24
  }
  styles {}
  document id="doc.mddr" title="MDDR" {
page id="page.mddr" w=(px)400 h=(px)200 {
  text id="t.mddr" x=(px)10 y=(px)20 w=(px)380 h=(px)80 fill=(token)"color.ink" font-family=(token)"font.body" font-size=(token)"size.body" format="markdown" {
    span "" data-ref="article.body"
  }
}
  }
}"##;
    let doc = parse(src);

    // Build a data context supplying the markdown-formatted field.
    let mut data = DataContext::default();
    data.fields
        .insert("article.body".to_owned(), "**Hi** there".to_owned());

    let result = compile_page(&doc, &default_provider(), 0, Some(&data));

    // No errors.
    assert!(
        result.diagnostics.iter().all(|d| !d.is_error()),
        "expected no errors; got: {:?}",
        result.diagnostics
    );

    // After data substitution + markdown parsing, "**Hi** there" should
    // produce at least 2 distinct glyph runs: the bold "Hi" and " there".
    let glyph_runs: Vec<_> = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::DrawGlyphRun { .. }))
        .collect();

    assert!(
        glyph_runs.len() >= 2,
        "expected >= 2 glyph runs after data+markdown parse of '**Hi** there'; got {}",
        glyph_runs.len()
    );
}
