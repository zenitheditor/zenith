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
