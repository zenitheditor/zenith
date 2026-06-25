//! Integration tests for data-binding scene resolution (Unit U8a).
//!
//! Covers:
//! - compile_page with a DataContext that has the field → fill resolves to a color
//! - compile_page with a DataContext that is missing the field → data.missing_field advisory
//! - compile_page with data: None and no data refs → byte-identical scene (no regression)
//! - compile_page with data: None and a data ref → data.no_context advisory

mod common;
use common::*;
use zenith_core::{DataContext, default_provider};
use zenith_scene::compile_page;
use zenith_scene::ir::Paint;

// A minimal document with a rect whose fill is a data ref.
const DATA_SRC: &str = r##"zenith version=1 {
  project id="proj.db" name="DB"
  assets {
  }
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.db" title="DB" {
    page id="page.db" w=(px)100 h=(px)100 {
      rect id="rect.db" x=(px)0 y=(px)0 w=(px)50 h=(px)50 fill=(data)"color.hex"
    }
  }
}"##;

// A minimal document with no data refs (byte-identical baseline).
const NODATAREF_SRC: &str = r##"zenith version=1 {
  project id="proj.nd" name="ND"
  assets {
  }
  tokens format="zenith-token-v1" {
    token id="color.solid" type="color" value="#ff0000"
  }
  styles {
  }
  document id="doc.nd" title="ND" {
    page id="page.nd" w=(px)100 h=(px)100 {
      rect id="rect.nd" x=(px)0 y=(px)0 w=(px)50 h=(px)50 fill=(token)"color.solid"
    }
  }
}"##;

/// When DataContext has the field, the fill resolves and emits a FillRect.
#[test]
fn data_ref_resolves_to_color_when_field_present() {
    let doc = parse(DATA_SRC);
    let mut ctx = DataContext::default();
    ctx.fields
        .insert("color.hex".to_owned(), "#ff0000".to_owned());

    let result = compile_page(&doc, &default_provider(), 0, Some(&ctx));

    // No data advisory expected when field is present
    let data_diags: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code.starts_with("data."))
        .collect();
    assert!(
        data_diags.is_empty(),
        "no data diagnostics expected when field resolves; got: {data_diags:?}"
    );

    // The rect fill should have emitted a FillRect (the hex string parses as a literal color)
    let rects: Vec<_> = result
        .scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
        .collect();
    assert!(
        !rects.is_empty(),
        "expected at least one FillRect when field resolves"
    );
}

/// When DataContext is Some but the field is missing, emits data.missing_field advisory.
#[test]
fn data_ref_missing_field_emits_advisory() {
    let doc = parse(DATA_SRC);
    let ctx = DataContext::default(); // empty — no fields

    let result = compile_page(&doc, &default_provider(), 0, Some(&ctx));

    let missing: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "data.missing_field")
        .collect();
    assert_eq!(
        missing.len(),
        1,
        "expected exactly 1 data.missing_field advisory; got: {missing:?}"
    );
}

/// When data is None and the doc has a data ref, emits data.no_context advisory.
#[test]
fn data_ref_no_context_emits_advisory() {
    let doc = parse(DATA_SRC);

    let result = compile_page(&doc, &default_provider(), 0, None);

    let no_ctx: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "data.no_context")
        .collect();
    assert_eq!(
        no_ctx.len(),
        1,
        "expected exactly 1 data.no_context advisory; got: {no_ctx:?}"
    );
}

// A document exercising data binding across ALL property kinds: page background
// (color), rect fill (color) + stroke (color), text font-size (dimension), and a
// text span data-ref (text content, currency-formatted).
const ALL_PROPS_SRC: &str = r##"zenith version=1 {
  project id="proj.ap" name="AP"
  assets {
  }
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.ap" title="AP" {
    page id="page.ap" w=(px)200 h=(px)200 background=(data)"bg" {
      rect id="rect.ap" x=(px)0 y=(px)0 w=(px)80 h=(px)80 fill=(data)"c" stroke=(data)"s" stroke-width=(px)4
      text id="text.ap" x=(px)10 y=(px)100 w=(px)180 h=(px)60 font-size=(data)"sz" {
        span "fallback" data-ref="amt" format="currency"
      }
    }
  }
}"##;

/// Build the full data context resolving every ALL_PROPS_SRC ref.
fn all_props_ctx() -> DataContext {
    let mut ctx = DataContext::default();
    ctx.fields.insert("bg".to_owned(), "#0000ff".to_owned()); // blue background
    ctx.fields.insert("c".to_owned(), "#ff0000".to_owned()); // red fill
    ctx.fields.insert("s".to_owned(), "#00ff00".to_owned()); // green stroke
    ctx.fields.insert("sz".to_owned(), "40".to_owned()); // 40px font-size
    ctx.fields.insert("amt".to_owned(), "1234.56".to_owned()); // → "$1,234.56"
    ctx
}

/// Whether the scene contains a solid `FillRect` with the given (r,g,b).
fn has_solid_fill_rect(commands: &[SceneCommand], r: u8, g: u8, b: u8) -> bool {
    commands.iter().any(|c| {
        matches!(
            c,
            SceneCommand::FillRect { paint: Paint::Solid { color }, .. }
                if color.r == r && color.g == g && color.b == b
        )
    })
}

/// Every property kind resolves through the pre-pass: background, fill, stroke,
/// font-size, and span text — all visible in the rendered scene.
#[test]
fn all_property_kinds_resolve_through_prepass() {
    let doc = parse(ALL_PROPS_SRC);
    let ctx = all_props_ctx();
    let result = compile_page(&doc, &default_provider(), 0, Some(&ctx));

    // No data diagnostics when every field resolves.
    let data_diags: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code.starts_with("data."))
        .collect();
    assert!(
        data_diags.is_empty(),
        "no data diagnostics expected; got: {data_diags:?}"
    );

    let cmds = &result.scene.commands;

    // Background → blue FillRect covering the page.
    assert!(
        has_solid_fill_rect(cmds, 0, 0, 255),
        "page background data-ref must resolve to a blue FillRect"
    );
    // Rect fill → red FillRect.
    assert!(
        has_solid_fill_rect(cmds, 255, 0, 0),
        "rect fill data-ref must resolve to a red FillRect"
    );
    // Rect stroke → green StrokeRect.
    let green_stroke = cmds.iter().any(|c| {
        matches!(c, SceneCommand::StrokeRect { color, .. }
            if color.r == 0 && color.g == 255 && color.b == 0)
    });
    assert!(
        green_stroke,
        "rect stroke data-ref must resolve to a green StrokeRect"
    );

    // font-size → a DrawGlyphRun shaped at 40px.
    let has_40px = cmds.iter().any(|c| {
        matches!(c, SceneCommand::DrawGlyphRun { font_size, .. } if (*font_size - 40.0).abs() < 0.01)
    });
    assert!(
        has_40px,
        "text font-size data-ref must resolve to a 40px glyph run"
    );

    // span data-ref → glyphs for the currency-formatted "$1,234.56" (9 chars).
    let expected_glyphs = "$1,234.56".chars().count();
    let span_glyphs = cmds.iter().find_map(|c| match c {
        SceneCommand::DrawGlyphRun { glyphs, .. } => Some(glyphs.len()),
        _ => None,
    });
    assert_eq!(
        span_glyphs,
        Some(expected_glyphs),
        "span data-ref must render the currency-formatted value '$1,234.56'"
    );
}

/// A span data-ref whose field is missing emits `data.missing_field` and the span
/// keeps its authored fallback text.
#[test]
fn span_data_ref_missing_field_emits_advisory_and_keeps_fallback() {
    let doc = parse(ALL_PROPS_SRC);
    let mut ctx = all_props_ctx();
    ctx.fields.remove("amt"); // drop the span's field only

    let result = compile_page(&doc, &default_provider(), 0, Some(&ctx));

    let missing: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "data.missing_field")
        .collect();
    assert_eq!(
        missing.len(),
        1,
        "exactly one data.missing_field for the missing span field; got: {missing:?}"
    );

    // The span keeps "fallback" (8 chars) — its authored text.
    let span_glyphs = result.scene.commands.iter().find_map(|c| match c {
        SceneCommand::DrawGlyphRun { glyphs, .. } => Some(glyphs.len()),
        _ => None,
    });
    assert_eq!(
        span_glyphs,
        Some("fallback".chars().count()),
        "a missing span field must leave the authored fallback text"
    );
}

/// Compiling ALL_PROPS_SRC with `data: None` succeeds (no panic) and emits a
/// single `data.no_context` advisory.
#[test]
fn all_props_no_context_succeeds_with_advisory() {
    let doc = parse(ALL_PROPS_SRC);
    let result = compile_page(&doc, &default_provider(), 0, None);

    let no_ctx: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| d.code == "data.no_context")
        .collect();
    assert_eq!(
        no_ctx.len(),
        1,
        "exactly one data.no_context advisory; got: {no_ctx:?}"
    );
    // Render still succeeds: the page clip is always emitted.
    assert!(
        !result.scene.commands.is_empty(),
        "the scene must still be produced even with unresolved refs"
    );
}

/// A document with no data refs compiled with data: None is byte-identical to
/// the same document compiled without data: None (regression/byte-identity test).
#[test]
fn no_data_ref_doc_byte_identical_with_data_none() {
    let doc = parse(NODATAREF_SRC);

    let result_a = compile_page(&doc, &default_provider(), 0, None);
    let result_b = compile_page(&doc, &default_provider(), 0, None);

    // Commands must be identical (deterministic — same bytes same output).
    let json_a = result_a.scene.to_json().expect("serialize a");
    let json_b = result_b.scene.to_json().expect("serialize b");
    assert_eq!(
        json_a, json_b,
        "byte-identical: two compiles of the same doc must produce the same scene"
    );

    // No data diagnostics expected.
    let data_diags: Vec<_> = result_a
        .diagnostics
        .iter()
        .filter(|d| d.code.starts_with("data."))
        .collect();
    assert!(
        data_diags.is_empty(),
        "no data diagnostics expected for doc with no data refs"
    );
}

/// A no-data-ref doc compiled with `data: Some(empty)` is byte-identical to the
/// same doc compiled with `data: None`. The pre-pass clones + substitutes but
/// finds nothing to change, so the rendered scene is unchanged.
#[test]
fn no_data_ref_doc_byte_identical_data_some_vs_none() {
    let doc = parse(NODATAREF_SRC);
    let ctx = DataContext::default();

    let with_none = compile_page(&doc, &default_provider(), 0, None);
    let with_some = compile_page(&doc, &default_provider(), 0, Some(&ctx));

    let json_none = with_none.scene.to_json().expect("serialize none");
    let json_some = with_some.scene.to_json().expect("serialize some");
    assert_eq!(
        json_none, json_some,
        "data Some with no refs must produce the same scene as data None"
    );

    // Neither path emits data diagnostics.
    assert!(
        with_some
            .diagnostics
            .iter()
            .all(|d| !d.code.starts_with("data.")),
        "no data diagnostics expected for a no-ref doc even with data Some"
    );
}
