//! Integration tests: contrast judged on the defaults-lowered document, text
//! without a fill judged as black, and labels left to the compile stage.

use zenith_core::{Diagnostic, KdlAdapter, KdlSource, validate};

fn src(tokens: &str, styles: &str, top: &str, page_body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="p" name="P"
  tokens format="zenith-token-v1" {{
    token id="color.page" type="color" value="#ffffff"
    token id="color.primary" type="color" value="#605dff"
{tokens}
  }}
  styles {{
{styles}
  }}
  {top}
  document id="d" title="D" {{
    page id="pg" w=(px)400 h=(px)400 background=(token)"color.page" {{
      {page_body}
    }}
  }}
}}
"##
    )
}

fn diags(source: &str) -> Vec<Diagnostic> {
    let doc = KdlAdapter
        .parse(source.as_bytes())
        .expect("test document must parse");
    validate(&doc).diagnostics
}

fn contrast_for<'a>(all: &'a [Diagnostic], subject: &str) -> Vec<&'a Diagnostic> {
    all.iter()
        .filter(|d| d.code.starts_with("contrast.") && d.subject_id.as_deref() == Some(subject))
        .collect()
}

#[test]
fn defaults_text_style_fill_is_judged() {
    let tokens = r##"token id="color.faint" type="color" value="#f4f4f4""##;
    let styles = r#"style id="body" { fill (token)"color.faint" }"#;
    let body = r#"text id="t" x=(px)10 y=(px)10 w=(px)200 h=(px)30 { span "Hi" }"#;
    let with = diags(&src(
        tokens,
        styles,
        r#"defaults { text style="body" }"#,
        body,
    ));
    let found = contrast_for(&with, "t");
    assert_eq!(found.len(), 1, "{with:?}");
    assert_eq!(found[0].code, "contrast.invisible");
    assert!(
        found[0].span.is_some(),
        "the span points at the authored text"
    );
}

#[test]
fn text_without_fill_is_judged_as_black() {
    let body = r#"rect id="r" x=(px)0 y=(px)0 w=(px)400 h=(px)400 fill=(token)"color.primary"
      text id="t" x=(px)10 y=(px)10 w=(px)200 h=(px)30 { span "Hi" }"#;
    let all = diags(&src("", "", "", body));
    let found = contrast_for(&all, "t");
    assert_eq!(found.len(), 1, "{all:?}");
    assert_eq!(found[0].code, "contrast.low");
}

#[test]
fn black_text_on_white_page_passes() {
    let body = r#"text id="t" x=(px)10 y=(px)10 w=(px)200 h=(px)30 { span "Hi" }"#;
    let all = diags(&src("", "", "", body));
    assert!(contrast_for(&all, "t").is_empty(), "{all:?}");
}

#[test]
fn defaults_pairing_makes_text_legible() {
    let tokens = r##"token id="color.primary.content" type="color" value="#ffffff""##;
    let body = r#"frame id="f" x=(px)0 y=(px)0 w=(px)400 h=(px)400 fill=(token)"color.primary" {
        text id="t" x=(px)10 y=(px)10 w=(px)200 h=(px)30 { span "Hi" }
      }"#;
    let without = diags(&src(tokens, "", "", body));
    assert_eq!(contrast_for(&without, "t").len(), 1, "{without:?}");
    let with = diags(&src(tokens, "", "defaults {}", body));
    assert!(contrast_for(&with, "t").is_empty(), "{with:?}");
}

#[test]
fn validation_leaves_labels_to_the_compile_stage() {
    let body = r#"shape id="btn" x=(px)20 y=(px)20 w=(px)160 h=(px)48 fill=(token)"color.primary" { span "Go" }"#;
    let all = diags(&src("", "", "", body));
    assert!(contrast_for(&all, "btn").is_empty(), "{all:?}");
}
