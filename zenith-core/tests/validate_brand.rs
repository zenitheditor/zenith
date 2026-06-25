//! Integration tests for the brand-contract validation feature.
//!
//! Covers:
//! - Parse: full block, empty `brand {}`, absent block, type errors.
//! - Format round-trip: parse→format→parse preserves the contract; absent brand
//!   block emits nothing.
//! - Validate: on-palette / off-palette, case-insensitive color, CMYK,
//!   font/weight, unconstrained categories, empty contract, governable policy.
//! - Diag-catalog: all 3 brand codes catalogued + governable.

mod common;

use common::*;
use zenith_core::format::format_document;

// ---------------------------------------------------------------------------
// Helper: minimal valid KDL document source with a brand block
// ---------------------------------------------------------------------------

/// Wrap KDL inner content in a minimal valid zenith document structure.
///
/// `tokens_block` is the inner content of the `tokens { … }` block (e.g. one
/// or more `token` lines). `brand_block` is the full `brand { … }` KDL text to
/// inject (or empty string to omit). The document always has exactly one page
/// with no children.
fn make_doc(tokens_inner: &str, brand_block: &str) -> String {
    format!(
        r##"zenith version=1 {{
  {brand_block}
  assets {{
  }}
  tokens format="zenith-token-v1" {{
    {tokens_inner}
  }}
  styles {{
  }}
  document id="doc.brand-test" {{
    page id="page.one" w=(px)1280 h=(px)720 {{
    }}
  }}
}}
"##
    )
}

fn parse_doc(src: &str) -> zenith_core::Document {
    let adapter = KdlAdapter;
    adapter.parse(src.as_bytes()).expect("parse must succeed")
}

// ---------------------------------------------------------------------------
// Parse tests
// ---------------------------------------------------------------------------

/// A full `brand { colors … fonts … weights … }` block round-trips through parse.
#[test]
fn parse_full_brand_block() {
    let src = make_doc(
        r##"token id="c.navy" type="color" value="#0b1f33""##,
        r##"brand {
    colors "#0b1f33" "#1b6cf0" "#ffffff"
    fonts "Noto Sans"
    weights 400 700
  }"##,
    );
    let doc = parse_doc(&src);
    let contract = &doc.brand_contract;
    assert_eq!(
        contract.allowed_colors,
        Some(vec![
            "#0b1f33".to_owned(),
            "#1b6cf0".to_owned(),
            "#ffffff".to_owned()
        ]),
        "colors should be parsed correctly"
    );
    assert_eq!(
        contract.allowed_fonts,
        Some(vec!["Noto Sans".to_owned()]),
        "fonts should be parsed correctly"
    );
    assert_eq!(
        contract.allowed_weights,
        Some(vec![400u32, 700u32]),
        "weights should be parsed correctly"
    );
}

/// An empty `brand {}` block (no children) → all categories remain `None`.
/// Absent children mean the `colors`/`fonts`/`weights` nodes never appear, so
/// all three fields remain `None` and `is_empty()` is true.
#[test]
fn parse_empty_brand_block_no_children() {
    let src = make_doc(
        "",
        r##"brand {
  }"##,
    );
    let doc = parse_doc(&src);
    assert!(
        doc.brand_contract.is_empty(),
        "empty brand block (no children) must produce an empty contract"
    );
}

/// Absent `brand` block → default empty contract.
#[test]
fn parse_absent_brand_block_gives_empty_contract() {
    // No brand block at all.
    let src = make_doc("", "");
    let doc = parse_doc(&src);
    assert!(
        doc.brand_contract.is_empty(),
        "absent brand block must produce an empty contract"
    );
    assert!(doc.brand_contract.allowed_colors.is_none());
    assert!(doc.brand_contract.allowed_fonts.is_none());
    assert!(doc.brand_contract.allowed_weights.is_none());
}

/// A `colors` child with a non-string argument (e.g. an integer) → ParseError.
#[test]
fn parse_invalid_color_type_is_parse_error() {
    let src = make_doc(
        "",
        r##"brand {
    colors 42
  }"##,
    );
    let adapter = KdlAdapter;
    let result = adapter.parse(src.as_bytes());
    assert!(
        result.is_err(),
        "non-string color argument must produce a ParseError"
    );
}

/// A `weights` child with a non-integer argument → ParseError.
#[test]
fn parse_invalid_weight_type_is_parse_error() {
    let src = make_doc(
        "",
        r##"brand {
    weights "bold"
  }"##,
    );
    let adapter = KdlAdapter;
    let result = adapter.parse(src.as_bytes());
    assert!(
        result.is_err(),
        "non-integer weight argument must produce a ParseError"
    );
}

/// A `weights` child with an out-of-range value (e.g. 50) → ParseError.
#[test]
fn parse_out_of_range_weight_is_parse_error() {
    let src = make_doc(
        "",
        r##"brand {
    weights 50
  }"##,
    );
    let adapter = KdlAdapter;
    let result = adapter.parse(src.as_bytes());
    assert!(result.is_err(), "weight < 100 must produce a ParseError");
}

/// A `weights` child with an out-of-range value (e.g. 950) → ParseError.
#[test]
fn parse_weight_above_900_is_parse_error() {
    let src = make_doc(
        "",
        r##"brand {
    weights 950
  }"##,
    );
    let adapter = KdlAdapter;
    let result = adapter.parse(src.as_bytes());
    assert!(result.is_err(), "weight > 900 must produce a ParseError");
}

/// Unknown children inside `brand { … }` are silently ignored (forward-compat).
#[test]
fn parse_unknown_brand_child_ignored() {
    let src = make_doc(
        "",
        r##"brand {
    colors "#ffffff"
    unknown-future-field "some value"
  }"##,
    );
    let doc = parse_doc(&src);
    assert_eq!(
        doc.brand_contract.allowed_colors,
        Some(vec!["#ffffff".to_owned()]),
        "known children must still be parsed; unknown children are ignored"
    );
}

/// A `fonts` child with a non-string argument → ParseError.
#[test]
fn parse_invalid_font_type_is_parse_error() {
    let src = make_doc(
        "",
        r##"brand {
    fonts 42
  }"##,
    );
    let adapter = KdlAdapter;
    let result = adapter.parse(src.as_bytes());
    assert!(
        result.is_err(),
        "non-string font argument must produce a ParseError"
    );
}

// ---------------------------------------------------------------------------
// Format round-trip tests
// ---------------------------------------------------------------------------

/// parse → format → parse preserves the brand contract.
#[test]
fn format_roundtrip_brand_block_preserved() {
    let src = make_doc(
        r##"token id="c.navy" type="color" value="#0b1f33""##,
        r##"brand {
    colors "#0b1f33" "#ffffff"
    fonts "Noto Sans"
    weights 400 700
  }"##,
    );
    let doc = parse_doc(&src);
    let formatted = format_document(&doc).expect("format must succeed");
    let formatted_str = String::from_utf8(formatted).expect("utf8");
    let doc2 = parse_doc(&formatted_str);

    // Compare sans spans (spans shift after reformat).
    let contract1 = {
        let mut c = doc.brand_contract.clone();
        c.source_span = None;
        c
    };
    let contract2 = {
        let mut c = doc2.brand_contract.clone();
        c.source_span = None;
        c
    };
    assert_eq!(
        contract1, contract2,
        "brand contract must survive a format round-trip"
    );
}

/// Absent brand block → formatter emits NO brand block.
#[test]
fn format_absent_brand_no_output() {
    let src = make_doc("", "");
    let doc = parse_doc(&src);
    let formatted = format_document(&doc).expect("format must succeed");
    let formatted_str = String::from_utf8(formatted).expect("utf8");
    assert!(
        !formatted_str.lines().any(|l| l.trim_start().starts_with("brand ")
            || l.trim() == "brand"
            || l.trim_start().starts_with("brand{")),
        "absent brand contract must not emit a brand block; got:\n{formatted_str}"
    );
}

/// Format round-trip with all three categories present is byte-stable on second pass.
#[test]
fn format_is_idempotent() {
    let src = make_doc(
        r##"token id="c.navy" type="color" value="#0b1f33""##,
        r##"brand {
    colors "#0b1f33"
    fonts "Noto Sans"
    weights 400
  }"##,
    );
    let doc1 = parse_doc(&src);
    let fmt1 = String::from_utf8(format_document(&doc1).unwrap()).unwrap();
    let doc2 = parse_doc(&fmt1);
    let fmt2 = String::from_utf8(format_document(&doc2).unwrap()).unwrap();
    assert_eq!(fmt1, fmt2, "format must be idempotent");
}

/// Colors are emitted in lowercase in canonical form (the parser normalises them).
#[test]
fn format_colors_emitted_lowercase() {
    let src = make_doc(
        "",
        r##"brand {
    colors "#0B1F33"
  }"##,
    );
    let doc = parse_doc(&src);
    let formatted = String::from_utf8(format_document(&doc).unwrap()).unwrap();
    assert!(
        formatted.contains("\"#0b1f33\""),
        "formatted colors should be lowercase; got:\n{formatted}"
    );
    assert!(
        !formatted.contains("\"#0B1F33\""),
        "uppercase hex should not appear in formatted output; got:\n{formatted}"
    );
}

// ---------------------------------------------------------------------------
// Validate tests
// ---------------------------------------------------------------------------

/// On-palette color → no `brand.color_off_palette` diagnostic.
#[test]
fn validate_color_on_palette_no_diagnostic() {
    let src = make_doc(
        r##"token id="c.navy" type="color" value="#0b1f33""##,
        r##"brand {
    colors "#0b1f33"
  }"##,
    );
    let doc = parse_doc(&src);
    let report = validate(&doc);
    assert!(
        !has_code(&report, "brand.color_off_palette"),
        "on-palette color must not fire brand.color_off_palette; got: {:?}",
        codes(&report)
    );
}

/// Off-palette color → `brand.color_off_palette` Warning.
#[test]
fn validate_color_off_palette_fires_warning() {
    let src = make_doc(
        r##"token id="c.red" type="color" value="#ff0000""##,
        r##"brand {
    colors "#0b1f33"
  }"##,
    );
    let doc = parse_doc(&src);
    let report = validate(&doc);
    assert!(
        has_code(&report, "brand.color_off_palette"),
        "off-palette color must fire brand.color_off_palette; got: {:?}",
        codes(&report)
    );
}

/// Color comparison is case-insensitive across both palette and token value.
#[test]
fn validate_color_case_insensitive() {
    // Token value is uppercase; palette entry is lowercase.
    let src = make_doc(
        r##"token id="c.upper" type="color" value="#0B1F33""##,
        r##"brand {
    colors "#0b1f33"
  }"##,
    );
    let doc = parse_doc(&src);
    let report = validate(&doc);
    assert!(
        !has_code(&report, "brand.color_off_palette"),
        "color match must be case-insensitive; got: {:?}",
        codes(&report)
    );
}

/// Absent `colors` category → no color diagnostic regardless of token values.
#[test]
fn validate_unconstrained_color_no_diagnostic() {
    let src = make_doc(
        r##"token id="c.any" type="color" value="#deadbe""##,
        r##"brand {
    weights 400
  }"##,
    );
    let doc = parse_doc(&src);
    let report = validate(&doc);
    assert!(
        !has_code(&report, "brand.color_off_palette"),
        "unconstrained color category must not fire diagnostic; got: {:?}",
        codes(&report)
    );
}

/// Empty contract (no brand block) → zero brand diagnostics.
#[test]
fn validate_empty_contract_no_diagnostics() {
    let src = make_doc(r##"token id="c.navy" type="color" value="#0b1f33""##, "");
    let doc = parse_doc(&src);
    let report = validate(&doc);
    let brand_diags: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.code.starts_with("brand."))
        .collect();
    assert!(
        brand_diags.is_empty(),
        "empty contract must not fire brand diagnostics; got: {brand_diags:?}"
    );
}

/// Font not in allowed list → `brand.font_not_allowed` Warning.
#[test]
fn validate_font_off_list_fires_warning() {
    let src = make_doc(
        r##"token id="font.body" type="fontFamily" value="Comic Sans MS""##,
        r##"brand {
    fonts "Noto Sans"
  }"##,
    );
    let doc = parse_doc(&src);
    let report = validate(&doc);
    assert!(
        has_code(&report, "brand.font_not_allowed"),
        "off-list font must fire brand.font_not_allowed; got: {:?}",
        codes(&report)
    );
}

/// Font in allowed list → no diagnostic.
#[test]
fn validate_font_on_list_no_diagnostic() {
    let src = make_doc(
        r##"token id="font.body" type="fontFamily" value="Noto Sans""##,
        r##"brand {
    fonts "Noto Sans"
  }"##,
    );
    let doc = parse_doc(&src);
    let report = validate(&doc);
    assert!(
        !has_code(&report, "brand.font_not_allowed"),
        "on-list font must not fire brand.font_not_allowed; got: {:?}",
        codes(&report)
    );
}

/// Weight not in allowed list → `brand.weight_not_allowed` Warning.
#[test]
fn validate_weight_off_list_fires_warning() {
    let src = make_doc(
        r##"token id="weight.thin" type="fontWeight" value=100"##,
        r##"brand {
    weights 400 700
  }"##,
    );
    let doc = parse_doc(&src);
    let report = validate(&doc);
    assert!(
        has_code(&report, "brand.weight_not_allowed"),
        "off-list weight must fire brand.weight_not_allowed; got: {:?}",
        codes(&report)
    );
}

/// Weight in allowed list → no diagnostic.
#[test]
fn validate_weight_on_list_no_diagnostic() {
    let src = make_doc(
        r##"token id="weight.regular" type="fontWeight" value=400"##,
        r##"brand {
    weights 400 700
  }"##,
    );
    let doc = parse_doc(&src);
    let report = validate(&doc);
    assert!(
        !has_code(&report, "brand.weight_not_allowed"),
        "on-list weight must not fire brand.weight_not_allowed; got: {:?}",
        codes(&report)
    );
}

/// Brand diagnostics are Warnings (not Errors or Advisories).
#[test]
fn validate_brand_diagnostics_are_warnings() {
    let src = make_doc(
        r##"token id="c.bad" type="color" value="#ff0000""##,
        r##"brand {
    colors "#0b1f33"
  }"##,
    );
    let doc = parse_doc(&src);
    let report = validate(&doc);
    for d in &report.diagnostics {
        if d.code.starts_with("brand.") {
            assert_eq!(
                d.severity,
                Severity::Warning,
                "brand diagnostic must be a Warning, got {:?}",
                d.severity
            );
        }
    }
}

/// A `deny "brand.color_off_palette"` entry in `diagnostics { … }` elevates the
/// Warning to Error (governable).
#[test]
fn validate_brand_color_is_governable_by_deny() {
    let src = r##"zenith version=1 {
  diagnostics {
    deny "brand.color_off_palette"
  }
  brand {
    colors "#0b1f33"
  }
  assets {
  }
  tokens format="zenith-token-v1" {
    token id="c.bad" type="color" value="#ff0000"
  }
  styles {
  }
  document id="doc.gov" {
    page id="page.one" w=(px)1280 h=(px)720 {
    }
  }
}
"##;
    let doc = parse_doc(src);
    let report = validate(&doc);
    let brand_diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "brand.color_off_palette");
    let d = brand_diag.expect("brand.color_off_palette must be present");
    assert_eq!(
        d.severity,
        Severity::Error,
        "deny on brand.color_off_palette must elevate to Error"
    );
}

/// An `allow "brand.color_off_palette"` entry suppresses the diagnostic.
#[test]
fn validate_brand_color_is_suppressable_by_allow() {
    let src = r##"zenith version=1 {
  diagnostics {
    allow "brand.color_off_palette"
  }
  brand {
    colors "#0b1f33"
  }
  assets {
  }
  tokens format="zenith-token-v1" {
    token id="c.bad" type="color" value="#ff0000"
  }
  styles {
  }
  document id="doc.gov" {
    page id="page.one" w=(px)1280 h=(px)720 {
    }
  }
}
"##;
    let doc = parse_doc(src);
    let report = validate(&doc);
    assert!(
        !has_code(&report, "brand.color_off_palette"),
        "allow on brand.color_off_palette must suppress the diagnostic; got: {:?}",
        codes(&report)
    );
}

// ---------------------------------------------------------------------------
// Diagnostic catalog tests
// ---------------------------------------------------------------------------

/// All three brand codes are in the catalog and governable (Warning severity).
#[test]
fn diag_catalog_brand_codes_are_catalogued_and_governable() {
    use zenith_core::diag_catalog::lookup;

    let cases = [
        "brand.color_off_palette",
        "brand.font_not_allowed",
        "brand.weight_not_allowed",
    ];

    for code in cases {
        let entry =
            lookup(code).unwrap_or_else(|| panic!("{code} must be in the diagnostic catalog"));
        assert_eq!(
            entry.severity,
            zenith_core::Severity::Warning,
            "{code} must have Warning severity in the catalog"
        );
        assert!(
            entry.is_governable(),
            "{code} must be governable (Warning severity)"
        );
    }
}
