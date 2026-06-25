//! Integration tests for data-binding foundation (Unit U8a).
//!
//! Covers:
//! - parse + format round-trip for `(data)"field.path"` properties
//! - validate: a doc with `(data)` refs passes with no error
//! - diag_catalog: `data.missing_field` and `data.no_context` are catalogued
//!   and are governable

mod common;
use common::*;

use zenith_core::Severity;
use zenith_core::diag_catalog::{DIAGNOSTIC_CODES, lookup};
use zenith_core::format::format_document;

// ── Parse + format round-trip ─────────────────────────────────────────────────

/// A `fill=(data)"revenue.total"` property parses to `DataRef` and formats back
/// to the identical string.
#[test]
fn data_ref_parse_format_roundtrip() {
    let src = r##"zenith version=1 {
  project id="proj.rt" name="RT"
  assets {
  }
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.rt" title="RT" {
    page id="page.rt" w=(px)640 h=(px)480 {
      rect id="rect.rt" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(data)"revenue.total"
    }
  }
}"##;

    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("doc must parse");

    // Verify the parsed value is a DataRef
    let page = &doc.body.pages[0];
    let node = &page.children[0];
    match node {
        Node::Rect(r) => {
            assert_eq!(
                r.fill,
                Some(PropertyValue::DataRef("revenue.total".to_owned())),
                "fill should be DataRef"
            );
        }
        other => panic!("expected Rect, got {other:?}"),
    }

    // Format back and check round-trip
    let formatted = format_document(&doc).expect("format must succeed");
    let formatted_str = String::from_utf8(formatted).expect("utf8");
    assert!(
        formatted_str.contains(r#"fill=(data)"revenue.total""#),
        "formatted output must contain the data ref: {formatted_str}"
    );

    // Parse again to confirm full round-trip
    let doc2 = adapter
        .parse(formatted_str.as_bytes())
        .expect("re-parse must succeed");
    let page2 = &doc2.body.pages[0];
    let node2 = &page2.children[0];
    match node2 {
        Node::Rect(r) => {
            assert_eq!(
                r.fill,
                Some(PropertyValue::DataRef("revenue.total".to_owned())),
                "DataRef must survive re-parse"
            );
        }
        other => panic!("expected Rect on re-parse, got {other:?}"),
    }
}

// ── Validate: data refs pass cleanly ─────────────────────────────────────────

/// A document with `fill=(data)"x"` must pass validation with no Error-level
/// diagnostics.
#[test]
fn validate_data_ref_no_error() {
    let src = r##"zenith version=1 {
  project id="proj.vd" name="VD"
  assets {
  }
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.vd" title="VD" {
    page id="page.vd" w=(px)640 h=(px)480 {
      rect id="rect.vd" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(data)"x"
    }
  }
}"##;

    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("must parse");
    let report = validate(&doc);
    let errors: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "no Error-level diagnostics expected for data ref doc; got: {errors:?}"
    );
}

// ── Diag catalog: new codes are catalogued and governable ─────────────────────

#[test]
fn data_missing_field_is_catalogued_and_advisory() {
    let entry =
        lookup("data.missing_field").expect("data.missing_field must be in the diagnostic catalog");
    assert_eq!(
        entry.severity,
        Severity::Advisory,
        "data.missing_field must be Advisory"
    );
    assert!(
        entry.is_governable(),
        "data.missing_field must be governable"
    );
}

#[test]
fn data_no_context_is_catalogued_and_advisory() {
    let entry =
        lookup("data.no_context").expect("data.no_context must be in the diagnostic catalog");
    assert_eq!(
        entry.severity,
        Severity::Advisory,
        "data.no_context must be Advisory"
    );
    assert!(entry.is_governable(), "data.no_context must be governable");
}

#[test]
fn data_codes_are_in_main_catalog() {
    // Both codes must appear in DIAGNOSTIC_CODES (drift guard)
    let codes: Vec<&str> = DIAGNOSTIC_CODES.iter().map(|e| e.code).collect();
    assert!(
        codes.contains(&"data.missing_field"),
        "data.missing_field must be in DIAGNOSTIC_CODES"
    );
    assert!(
        codes.contains(&"data.no_context"),
        "data.no_context must be in DIAGNOSTIC_CODES"
    );
}
