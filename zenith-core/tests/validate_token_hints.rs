//! Integration tests: one-round-fix diagnostics.
//!
//! An agent must be able to fix every error from a single `validate` run, so
//! these diagnostics carry the next action: a did-you-mean or the declared
//! candidates (`token.unknown_reference`, `token.raw_visual_literal`), the
//! allowed values (`node.invalid_value`), and a source span.

#[path = "common/codes.rs"]
mod codes;
#[path = "common/has_code.rs"]
mod has_code;

use codes::codes;
use has_code::has_code;
use zenith_core::{Diagnostic, KdlAdapter, KdlSource, Severity, ValidationReport, validate};

fn doc_src(tokens: &str, children: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.h" name="Hints"
  tokens format="zenith-token-v1" {{
{tokens}
  }}
  styles {{
  }}
  document id="doc.h" title="Hints" {{
    page id="page.h" w=(px)800 h=(px)600 {{
{children}
    }}
  }}
}}
"##
    )
}

fn validate_src(src: &str) -> ValidationReport {
    let doc = KdlAdapter
        .parse(src.as_bytes())
        .expect("parse must succeed");
    validate(&doc)
}

fn find<'a>(report: &'a ValidationReport, code: &str) -> &'a Diagnostic {
    report
        .diagnostics
        .iter()
        .find(|d| d.code == code)
        .unwrap_or_else(|| panic!("expected {code}; got {:?}", codes(report)))
}

const RECT_PREFIX: &str = "x=(px)0 y=(px)0 w=(px)10 h=(px)10";

#[test]
fn unknown_token_near_miss_suggests_declared_token() {
    let src = doc_src(
        r##"    token id="color.base.content" type="color" value="#112233""##,
        &format!(r#"      rect id="r.1" {RECT_PREFIX} fill=(token)"color.base.contnt""#),
    );
    let report = validate_src(&src);
    let d = find(&report, "token.unknown_reference");
    assert_eq!(d.severity, Severity::Error);
    assert!(
        d.message.contains("did you mean 'color.base.content'?"),
        "got: {}",
        d.message
    );
    assert!(
        !d.message.contains("failed resolution"),
        "got: {}",
        d.message
    );
}

#[test]
fn unknown_token_dotted_fallback_suggests_same_group() {
    // Full-string distance is 3 (> 2), but the `color.base` prefix matches and
    // the last segments are within the per-segment budget.
    let src = doc_src(
        r##"    token id="color.base.content-primary" type="color" value="#112233""##,
        &format!(r#"      rect id="r.1" {RECT_PREFIX} fill=(token)"color.base.contnt-primry""#),
    );
    let report = validate_src(&src);
    let d = find(&report, "token.unknown_reference");
    assert!(
        d.message
            .contains("did you mean 'color.base.content-primary'?"),
        "got: {}",
        d.message
    );
}

#[test]
fn unknown_token_without_match_lists_first_eight_and_remainder() {
    let tokens: String = (0..10)
        .map(|i| format!("    token id=\"color.t{i:02}\" type=\"color\" value=\"#112233\"\n"))
        .collect();
    let src = doc_src(
        &tokens,
        &format!(r#"      rect id="r.1" {RECT_PREFIX} fill=(token)"color.zzz""#),
    );
    let report = validate_src(&src);
    let d = find(&report, "token.unknown_reference");
    assert!(!d.message.contains("did you mean"), "got: {}", d.message);
    assert!(
        d.message.contains(
            "declared color or gradient tokens: color.t00, color.t01, color.t02, \
             color.t03, color.t04, color.t05, color.t06, color.t07 (+2 more)"
        ),
        "got: {}",
        d.message
    );
}

#[test]
fn unknown_token_excludes_wrong_type_candidates() {
    let src = doc_src(
        "    token id=\"color.ok\" type=\"color\" value=\"#112233\"\n    \
         token id=\"size.xx\" type=\"dimension\" value=(px)4",
        &format!(r#"      rect id="r.1" {RECT_PREFIX} fill=(token)"size.x""#),
    );
    let report = validate_src(&src);
    let d = find(&report, "token.unknown_reference");
    // `size.xx` is one edit away but is a dimension: a fill never accepts it.
    assert!(!d.message.contains("did you mean"), "got: {}", d.message);
    assert!(d.message.contains("color.ok"), "got: {}", d.message);
    assert!(!d.message.contains("size.xx"), "got: {}", d.message);
}

#[test]
fn unknown_token_with_no_token_of_that_type_says_so() {
    let src = doc_src(
        r##"    token id="size.xx" type="dimension" value=(px)4"##,
        &format!(r#"      rect id="r.1" {RECT_PREFIX} fill=(token)"color.nope""#),
    );
    let report = validate_src(&src);
    let d = find(&report, "token.unknown_reference");
    assert!(
        d.message.contains("no color or gradient tokens declared"),
        "got: {}",
        d.message
    );
}

#[test]
fn unknown_token_carries_source_span() {
    let src = doc_src(
        r##"    token id="color.ok" type="color" value="#112233""##,
        &format!(r#"      rect id="r.1" {RECT_PREFIX} fill=(token)"color.nope""#),
    );
    let report = validate_src(&src);
    let d = find(&report, "token.unknown_reference");
    assert!(d.span.is_some(), "unknown_reference must carry a span");
}

#[test]
fn raw_font_weight_names_type_and_candidates() {
    let src = doc_src(
        "    token id=\"weight.bold\" type=\"fontWeight\" value=700\n    \
         token id=\"weight.thin\" type=\"fontWeight\" value=100\n    \
         token id=\"color.ok\" type=\"color\" value=\"#112233\"",
        r#"      text id="t.1" x=(px)0 y=(px)0 w=(px)100 h=(px)20 font-weight=700 { span "hi" }"#,
    );
    let report = validate_src(&src);
    let d = find(&report, "token.raw_visual_literal");
    assert!(
        d.message
            .contains("expects a fontWeight token; use (token)\"weight.bold\" (700)"),
        "got: {}",
        d.message
    );
    assert!(
        d.message
            .contains("declared fontWeight tokens: weight.bold, weight.thin"),
        "got: {}",
        d.message
    );
    assert!(!d.message.contains("color.ok"), "got: {}", d.message);
    assert!(d.span.is_some(), "raw_visual_literal must carry a span");
}

#[test]
fn raw_literal_with_no_candidates_says_to_declare_one() {
    let src = doc_src(
        "",
        &format!(r##"      rect id="r.1" {RECT_PREFIX} fill="#ff0000""##),
    );
    let report = validate_src(&src);
    let d = find(&report, "token.raw_visual_literal");
    assert!(
        d.message.contains("no color or gradient tokens declared"),
        "got: {}",
        d.message
    );
}

#[test]
fn token_alias_to_unknown_token_suggests_same_type_token() {
    let src = doc_src(
        "    token id=\"color.base\" type=\"color\" value=\"#112233\"\n    \
         token id=\"size.base\" type=\"dimension\" value=(px)4\n    \
         token id=\"color.alias\" type=\"color\" value=(token)\"color.bsae\"",
        &format!(r#"      rect id="r.1" {RECT_PREFIX} fill=(token)"color.base""#),
    );
    let report = validate_src(&src);
    let d = find(&report, "token.unknown_reference");
    assert!(
        d.message.contains("did you mean 'color.base'?"),
        "got: {}",
        d.message
    );
    assert!(
        d.span.is_some(),
        "alias unknown_reference must carry a span"
    );
}

#[test]
fn token_alias_to_unknown_token_lists_same_type_tokens() {
    let src = doc_src(
        "    token id=\"color.base\" type=\"color\" value=\"#112233\"\n    \
         token id=\"size.base\" type=\"dimension\" value=(px)4\n    \
         token id=\"color.alias\" type=\"color\" value=(token)\"brand\"",
        &format!(r#"      rect id="r.1" {RECT_PREFIX} fill=(token)"color.base""#),
    );
    let report = validate_src(&src);
    let d = find(&report, "token.unknown_reference");
    assert!(
        d.message.contains("declared color tokens: color.base"),
        "got: {}",
        d.message
    );
    assert!(!d.message.contains("size.base"), "got: {}", d.message);
}

#[test]
fn unknown_property_is_error_without_newer_schema_excuse() {
    let src = doc_src(
        "",
        &format!("      rect id=\"r.1\" {RECT_PREFIX} quantum_flux=1"),
    );
    let report = validate_src(&src);
    let d = find(&report, "node.unknown_property");
    assert_eq!(d.severity, Severity::Error);
    assert!(report.has_errors());
    assert!(
        !d.message.contains("newer-schema") && !d.message.contains("version-relative"),
        "got: {}",
        d.message
    );
    assert!(d.span.is_some());
}

#[test]
fn invalid_blend_mode_is_invalid_value_error_with_allowed_values() {
    let src = doc_src(
        "",
        &format!(r#"      rect id="r.1" {RECT_PREFIX} blend-mode="multipy""#),
    );
    let report = validate_src(&src);
    let d = find(&report, "node.invalid_value");
    assert_eq!(d.severity, Severity::Error);
    assert!(
        d.message.contains("blend-mode") && d.message.contains("'multipy'"),
        "got: {}",
        d.message
    );
    assert!(
        d.message.contains("did you mean 'multiply'?"),
        "got: {}",
        d.message
    );
    assert!(d.message.contains("Allowed values:"), "got: {}", d.message);
    assert!(
        !has_code(&report, "node.unknown_property"),
        "a bad enum value is not an unknown property; got {:?}",
        codes(&report)
    );
}

#[test]
fn invalid_stroke_linecap_without_near_match_lists_allowed_values() {
    let src = doc_src(
        "",
        &format!(r#"      ellipse id="e.1" {RECT_PREFIX} stroke-linecap="pointy""#),
    );
    let report = validate_src(&src);
    let d = find(&report, "node.invalid_value");
    assert!(!d.message.contains("did you mean"), "got: {}", d.message);
    assert!(
        d.message
            .contains("invalid stroke-linecap 'pointy' — allowed values: butt, round, square"),
        "got: {}",
        d.message
    );
}

#[test]
fn image_invalid_fit_is_error_with_did_you_mean() {
    let src = doc_src(
        "    ",
        "      image id=\"i.1\" asset=\"a.1\" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fit=\"covr\"",
    );
    let report = validate_src(&src);
    let d = find(&report, "image.invalid_fit");
    assert_eq!(d.severity, Severity::Error);
    assert!(
        d.message.contains("did you mean 'cover'?"),
        "got: {}",
        d.message
    );
    assert!(
        d.message
            .contains("Allowed values: contain, cover, stretch, none"),
        "got: {}",
        d.message
    );
    assert!(
        !d.message.contains("version-relative"),
        "got: {}",
        d.message
    );
}

#[test]
fn chart_invalid_orientation_is_error_naming_allowed_values() {
    let src = doc_src(
        "",
        "      chart id=\"c.1\" kind=\"bar\" x=(px)0 y=(px)0 w=(px)100 h=(px)100 orientation=\"diagonal\"",
    );
    let report = validate_src(&src);
    let d = find(&report, "chart.invalid_orientation");
    assert_eq!(d.severity, Severity::Error);
    assert!(
        d.message
            .contains("invalid orientation 'diagonal' — allowed values: vertical, horizontal"),
        "got: {}",
        d.message
    );
}

#[test]
fn unknown_token_type_is_error_naming_allowed_types() {
    let src = doc_src(r##"    token id="x.1" type="colour" value="#112233""##, "");
    let report = validate_src(&src);
    let d = find(&report, "token.unknown_type");
    assert_eq!(d.severity, Severity::Error);
    assert!(
        d.message.contains("did you mean 'color'?"),
        "got: {}",
        d.message
    );
    assert!(d.span.is_some());
}

#[test]
fn asset_unknown_property_suggests_known_property() {
    let src = r##"zenith version=1 {
  project id="proj.h" name="Hints"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  assets {
    asset id="a.1" kind="image" src="x.png" sha26="abc"
  }
  document id="doc.h" title="Hints" {
    page id="page.h" w=(px)800 h=(px)600 {
    }
  }
}
"##;
    let report = validate_src(src);
    let d = find(&report, "asset.unknown_property");
    assert_eq!(d.severity, Severity::Error);
    assert!(
        d.message.contains("did you mean 'sha256'?"),
        "got: {}",
        d.message
    );
    assert!(
        !d.message.contains("version-relative"),
        "got: {}",
        d.message
    );
}

#[test]
fn style_unknown_property_is_error_with_schema_hint() {
    let src = r##"zenith version=1 {
  project id="proj.h" name="Hints"
  tokens format="zenith-token-v1" {
  }
  styles {
    style id="s.1" {
      bogus-prop "x"
    }
  }
  document id="doc.h" title="Hints" {
    page id="page.h" w=(px)800 h=(px)600 {
    }
  }
}
"##;
    let report = validate_src(src);
    let d = find(&report, "style.unknown_property");
    assert_eq!(d.severity, Severity::Error);
    assert!(d.message.contains("zenith schema"), "got: {}", d.message);
}
