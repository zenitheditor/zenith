//! Text contrast at the compile stage: each `text` node is judged where its
//! drawn glyphs land, on the defaults-lowered document, on every page.

mod common;
use common::parse;
use zenith_core::Diagnostic;
use zenith_core::default_provider;
use zenith_scene::compile;

fn src(tokens: &str, styles: &str, top: &str, page_body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="p" name="P"
  tokens format="zenith-token-v1" {{
    token id="color.page" type="color" value="#ffffff"
    token id="color.primary" type="color" value="#605dff"
    token id="color.navy" type="color" value="#0b1f3a"
    token id="color.white" type="color" value="#ffffff"
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
    compile(&parse(source), &default_provider()).diagnostics
}

fn contrast_for<'a>(all: &'a [Diagnostic], subject: &str) -> Vec<&'a Diagnostic> {
    all.iter()
        .filter(|d| d.code.starts_with("contrast.") && d.subject_id.as_deref() == Some(subject))
        .collect()
}

/// A 120×120 text box centred on an 88 px circular badge. The box corners
/// sit on the page, but every glyph sits on the badge.
const BADGE_TEXT: &str = r#"text id="t" x=(px)40 y=(px)40 w=(px)120 h=(px)120 align="center" v-align="middle" fill=(token)"color.white" font-size=(px)28 { span "VS" }"#;

#[test]
fn glyphs_inside_a_badge_judge_the_badge_not_the_box_corners() {
    // Box sampling read the page at the box corners and reported white on
    // white. The glyph ink sits on the navy badge.
    let body = format!(
        r#"rect id="badge" x=(px)56 y=(px)56 w=(px)88 h=(px)88 radius=(px)44 fill=(token)"color.navy"
      {BADGE_TEXT}"#
    );
    let all = diags(&src("", "", "", &body));
    assert!(contrast_for(&all, "t").is_empty(), "{all:?}");
}

#[test]
fn the_same_glyphs_over_the_page_background_are_reported() {
    let all = diags(&src("", "", "", BADGE_TEXT));
    let found = contrast_for(&all, "t");
    assert_eq!(found.len(), 1, "{all:?}");
    assert_eq!(found[0].code, "contrast.invisible");
    assert!(found[0].span.is_some(), "points at the authored text");
}

#[test]
fn rotated_glyphs_are_sampled_where_they_draw() {
    // Turned a quarter, the glyphs fill the narrow navy bar. The unrotated
    // box spans 0..220 across the page.
    let body = r#"rect id="bar" x=(px)80 y=(px)0 w=(px)60 h=(px)400 fill=(token)"color.navy"
      text id="t" x=(px)0 y=(px)130 w=(px)220 h=(px)40 align="center" v-align="middle" rotate=(deg)90 fill=(token)"color.white" font-size=(px)20 { span "Rotated" }"#;
    let all = diags(&src("", "", "", body));
    assert!(contrast_for(&all, "t").is_empty(), "{all:?}");

    let off = body.replace(
        r#"x=(px)80 y=(px)0 w=(px)60"#,
        r#"x=(px)300 y=(px)0 w=(px)60"#,
    );
    let all = diags(&src("", "", "", &off));
    assert_eq!(contrast_for(&all, "t").len(), 1, "{all:?}");
}

#[test]
fn glyphs_straddling_a_backdrop_edge_use_the_worst_sample() {
    // The text runs off the right edge of the badge onto the page.
    let body = r#"rect id="b" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.navy"
      text id="t" x=(px)10 y=(px)30 w=(px)300 h=(px)40 fill=(token)"color.white" font-size=(px)24 { span "Spills over the edge" }"#;
    let all = diags(&src("", "", "", body));
    let found = contrast_for(&all, "t");
    assert_eq!(found.len(), 1, "{all:?}");
    assert_eq!(found[0].code, "contrast.invisible");
}

#[test]
fn empty_text_draws_no_glyph_and_is_not_judged() {
    let body =
        r#"text id="t" x=(px)10 y=(px)10 w=(px)200 h=(px)30 fill=(token)"color.white" { span "" }"#;
    let all = diags(&src("", "", "", body));
    assert!(contrast_for(&all, "t").is_empty(), "{all:?}");
}

#[test]
fn validation_leaves_text_contrast_to_the_compile_stage() {
    let doc = parse(&src("", "", "", BADGE_TEXT));
    let report = zenith_core::validate(&doc);
    assert!(
        contrast_for(&report.diagnostics, "t").is_empty(),
        "{:?}",
        report.diagnostics
    );
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
    let doc = parse(&src("", "", "", body));
    let report = zenith_core::validate(&doc);
    assert!(
        contrast_for(&report.diagnostics, "btn").is_empty(),
        "{:?}",
        report.diagnostics
    );
}
