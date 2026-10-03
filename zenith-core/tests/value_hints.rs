//! Integration tests: value-based hints for raw visual literals and
//! declared-prefix ranking for unknown token references.

use zenith_core::{Diagnostic, KdlAdapter, KdlSource, validate};

const COBALT_SUBSET: &str = r##"    token id="color.base.100" type="color" value="#f7f9fa"
    token id="color.base.content" type="color" value="#0d1529"
    token id="color.primary" type="color" value="#605dff"
    token id="color.primary.content" type="color" value="#edf1fe"
    token id="color.white" type="color" value="#ffffff"
    token id="radius.box" type="dimension" value=(px)32
    token id="radius.field" type="dimension" value=(px)4
    token id="border.width" type="dimension" value=(px)1
    token id="size.h1" type="dimension" value=(px)64
    token id="size.body" type="dimension" value=(px)28"##;

fn diagnostics(children: &str) -> Vec<Diagnostic> {
    let src = format!(
        r##"zenith version=1 {{
  project id="proj.h" name="Hints"
  tokens format="zenith-token-v1" {{
{COBALT_SUBSET}
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
    );
    let doc = KdlAdapter.parse(src.as_bytes()).expect("parses");
    validate(&doc).diagnostics
}

fn message(diags: &[Diagnostic], code: &str, needle: &str) -> String {
    diags
        .iter()
        .find(|d| d.code == code && d.message.contains(needle))
        .map(|d| d.message.clone())
        .unwrap_or_else(|| panic!("no {code} mentioning {needle}: {diags:#?}"))
}

const GEOM: &str = "x=(px)0 y=(px)0 w=(px)100 h=(px)40";

#[test]
fn raw_color_names_token_with_same_value() {
    let d = diagnostics(&format!(r##"      rect id="r" {GEOM} fill="#ffffff""##));
    let m = message(&d, "token.raw_visual_literal", "'fill'");
    assert!(m.contains(r##"use (token)"color.white" (#ffffff)"##), "{m}");
}

#[test]
fn raw_color_without_exact_names_nearest() {
    let d = diagnostics(&format!(r##"      rect id="r" {GEOM} fill="#f8f8f8""##));
    let m = message(&d, "token.raw_visual_literal", "'fill'");
    assert!(
        m.contains(r##"nearest is (token)"color.base.100" (#f7f9fa)"##),
        "{m}"
    );
}

#[test]
fn raw_font_size_names_nearest_size_token_not_border() {
    let d = diagnostics(&format!(
        r#"      text id="t" {GEOM} font-size=(px)96 {{ span "a" }}"#
    ));
    let m = message(&d, "token.raw_visual_literal", "'font-size'");
    assert!(m.contains(r#"nearest is (token)"size.h1" (64px)"#), "{m}");
    assert!(!m.contains("e.g. (token)\"border.width\""), "{m}");
}

#[test]
fn raw_radius_names_radius_token() {
    let d = diagnostics(&format!(r#"      rect id="r" {GEOM} radius=(px)4"#));
    let m = message(&d, "token.raw_visual_literal", "'radius'");
    assert!(m.contains(r#"use (token)"radius.field" (4px)"#), "{m}");
}

#[test]
fn unknown_reference_prefers_declared_prefix() {
    let d = diagnostics(&format!(
        r#"      rect id="r" {GEOM} fill=(token)"color.primary.500""#
    ));
    let m = message(&d, "token.unknown_reference", "color.primary.500");
    assert!(m.contains("did you mean 'color.primary'?"), "{m}");
}
