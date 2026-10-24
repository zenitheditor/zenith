//! Integration tests: variant overrides that re-layout.
//!
//! Covers the override keys `font-size`, `w`/`h` `hug`/`fill`, `layout`, and
//! the frame container keys: parse, canonical format round trip, parse
//! errors, and validation.

#[path = "common/codes.rs"]
mod codes;

use codes::codes;
use zenith_core::format::format_document;
use zenith_core::{
    KdlAdapter, KdlSource, LayoutAlign, LayoutJustify, LayoutKind, PropertyValue, SizeKeyword,
    ValidationReport, strip_spans, validate,
};

/// A document whose `variants` block holds `overrides` in one variant. The
/// source page holds a column frame `root` with a text `title`, a row frame
/// `cards`, and a `line`.
fn doc_with(overrides: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.rl" name="RL"
  tokens format="zenith-token-v1" {{
    token id="color.ink" type="color" value="#111111"
    token id="font.sans" type="fontFamily" value="Noto Sans"
    token id="size.body" type="dimension" value=(px)24
    token id="size.story" type="dimension" value=(px)48
    token id="space.lg" type="dimension" value=(px)32
  }}
  styles {{
  }}
  variants {{
    variant id="story" source="page.main" w=(px)1080 h=(px)1920 {{
      {overrides}
    }}
  }}
  document id="doc.rl" title="RL" {{
    page id="page.main" w=(px)1080 h=(px)1080 {{
      frame id="root" x=(px)0 y=(px)0 w=(px)1080 h=(px)1080 layout="column" padding=(px)80 gap=(px)24 {{
        text id="title" fill=(token)"color.ink" font-family=(token)"font.sans" font-size=(token)"size.body" {{
          span "Title"
        }}
        frame id="cards" layout="row" gap=(px)16 {{
          frame id="card" w="fill" h=(px)120 layout="column"
        }}
      }}
      line id="rule" x1=(px)0 y1=(px)1000 x2=(px)1080 y2=(px)1000 stroke=(token)"color.ink"
    }}
  }}
}}
"##
    )
}

fn check(overrides: &str) -> ValidationReport {
    let src = doc_with(overrides);
    let doc = KdlAdapter
        .parse(src.as_bytes())
        .expect("parse must succeed");
    validate(&doc)
}

fn messages<'a>(report: &'a ValidationReport, code: &str) -> Vec<&'a str> {
    report
        .diagnostics
        .iter()
        .filter(|d| d.code == code)
        .map(|d| d.message.as_str())
        .collect()
}

const FULL: &str = r#"override node="root" w=(px)1080 h=(px)1920 padding=(token)"space.lg" padding-x=(px)96 padding-y=(px)120 padding-top=(px)1 padding-right=(px)2 padding-bottom=(px)3 padding-left=(px)4 justify="space-between"
      override node="title" font-size=(token)"size.story"
      override node="cards" layout="column" gap=(px)24 wrap-gap=(px)12 align="center" wrap=#true
      override node="card" w="hug" h="fill""#;

// ── Parse ─────────────────────────────────────────────────────────────────────

#[test]
fn new_override_keys_parse_into_typed_fields() {
    let src = doc_with(FULL);
    let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
    let ovs = &doc.variants[0].overrides;
    assert_eq!(ovs.len(), 4);

    let root = &ovs[0];
    assert_eq!(root.w.as_ref().map(|d| d.value), Some(1080.0));
    assert_eq!(
        root.container.padding,
        Some(PropertyValue::TokenRef("space.lg".to_owned()))
    );
    assert_eq!(
        root.container.padding_left.as_ref(),
        Some(&PropertyValue::Dimension(zenith_core::Dimension {
            value: 4.0,
            unit: zenith_core::Unit::Px,
        }))
    );
    assert_eq!(root.container.justify, Some(LayoutJustify::SpaceBetween));
    assert!(root.unknown_props.is_empty(), "{:?}", root.unknown_props);

    let title = &ovs[1];
    assert_eq!(
        title.font_size,
        Some(PropertyValue::TokenRef("size.story".to_owned()))
    );
    assert!(!title.sets_frame_layout());

    let cards = &ovs[2];
    assert_eq!(cards.layout, Some(LayoutKind::Column));
    assert_eq!(cards.container.align, Some(LayoutAlign::Center));
    assert_eq!(cards.container.wrap, Some(true));
    assert!(cards.container.wrap_gap.is_some());

    let card = &ovs[3];
    assert_eq!(card.w_keyword, Some(SizeKeyword::Hug));
    assert_eq!(card.h_keyword, Some(SizeKeyword::Fill));
    assert_eq!((card.w.as_ref(), card.h.as_ref()), (None, None));
}

#[test]
fn bad_override_size_string_is_a_parse_error() {
    let src = doc_with(r#"override node="card" w="huge""#);
    let err = KdlAdapter
        .parse(src.as_bytes())
        .expect_err("w=\"huge\" must not parse");
    assert!(
        err.message.contains(r#"override w="huge" is not a size"#),
        "{}",
        err.message
    );
    assert!(
        err.message.contains(r#""hug" or "fill""#),
        "{}",
        err.message
    );
}

#[test]
fn unitless_override_position_is_a_parse_error() {
    let src = doc_with(r#"override node="title" x=40"#);
    let err = KdlAdapter
        .parse(src.as_bytes())
        .expect_err("x=40 must not parse");
    assert!(
        err.message.contains("override x") && err.message.contains("unit annotation"),
        "{}",
        err.message
    );
}

// ── Format ────────────────────────────────────────────────────────────────────

#[test]
fn new_override_keys_round_trip_canonically() {
    let src = doc_with(FULL);
    let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
    let once = format_document(&doc).expect("format");
    let text = String::from_utf8(once.clone()).expect("utf8");
    for line in [
        r#"override node="root" w=(px)1080 h=(px)1920 padding=(token)"space.lg" padding-x=(px)96 padding-y=(px)120 padding-top=(px)1 padding-right=(px)2 padding-bottom=(px)3 padding-left=(px)4 justify="space-between""#,
        r#"override node="title" font-size=(token)"size.story""#,
        r#"override node="cards" layout="column" gap=(px)24 wrap-gap=(px)12 align="center" wrap=#true"#,
        r#"override node="card" w="hug" h="fill""#,
    ] {
        assert!(text.contains(line), "missing `{line}` in:\n{text}");
    }
    let reparsed = KdlAdapter.parse(&once).expect("re-parse");
    assert_eq!(
        strip_spans(doc).variants,
        strip_spans(reparsed.clone()).variants
    );
    let twice = format_document(&reparsed).expect("format again");
    assert_eq!(once, twice, "format must be idempotent");
}

#[test]
fn canonical_order_does_not_depend_on_source_order() {
    let src = doc_with(r#"override node="cards" wrap=#false gap=(px)8 layout="row" w="fill""#);
    let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
    let text = String::from_utf8(format_document(&doc).expect("format")).expect("utf8");
    assert!(
        text.contains(r#"override node="cards" w="fill" layout="row" gap=(px)8 wrap=#false"#),
        "{text}"
    );
}

// ── Validate ──────────────────────────────────────────────────────────────────

#[test]
fn full_retarget_overrides_are_clean() {
    let report = check(FULL);
    assert!(
        report.diagnostics.is_empty(),
        "valid overrides must produce no diagnostics; got {:?}",
        report.diagnostics
    );
}

#[test]
fn token_used_only_by_an_override_is_not_unused() {
    let report = check(r#"override node="title" font-size=(token)"size.story""#);
    let unused = messages(&report, "token.unused");
    assert!(
        unused.iter().all(|m| !m.contains("size.story")),
        "size.story is referenced by an override; got {unused:?}"
    );
}

#[test]
fn frame_keys_on_a_text_node_are_unsupported() {
    let report = check(r#"override node="title" layout="row" gap=(px)8 padding-x=(px)4"#);
    let found = messages(&report, "variant.override_unsupported_property");
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(found[0].contains("'layout', which a text node does not support"));
    assert!(found[0].contains("target a frame"));
    assert!(found[1].contains("'gap'"));
    assert!(found[2].contains("'padding-x'"));
}

#[test]
fn font_size_on_a_frame_is_unsupported() {
    let report = check(r#"override node="cards" font-size=(token)"size.story""#);
    let found = messages(&report, "variant.override_unsupported_property");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("'font-size', which a frame node does not support"));
}

#[test]
fn size_keyword_on_a_line_is_unsupported() {
    let report = check(r#"override node="rule" w="fill""#);
    let found = messages(&report, "variant.override_unsupported_property");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("'w', which a line node does not support"));
}

#[test]
fn unknown_layout_words_are_invalid_values() {
    let report = check(r#"override node="cards" layout="flow" justify="around" align="middle""#);
    let found = messages(&report, "node.invalid_value");
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(found[0].contains("override for node 'cards'") && found[0].contains("flow"));
    assert!(found[1].contains("around"));
    assert!(found[2].contains("middle"));
}

#[test]
fn bad_layout_dimensions_are_invalid_geometry() {
    let report = check(r#"override node="cards" gap="wide" padding=(em)2"#);
    let found = messages(&report, "node.invalid_geometry");
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found.iter().any(|m| m.contains("'gap'")));
    assert!(found.iter().any(|m| m.contains("'padding'")));
}

#[test]
fn font_size_literal_and_unknown_token_are_token_errors() {
    let literal = check(r#"override node="title" font-size=(px)48"#);
    assert!(
        codes(&literal).contains(&"token.raw_visual_literal"),
        "{:?}",
        literal.diagnostics
    );
    let unknown = check(r#"override node="title" font-size=(token)"size.huge""#);
    assert!(
        codes(&unknown).contains(&"token.unknown_reference"),
        "{:?}",
        unknown.diagnostics
    );
    let wrong_type = check(r#"override node="title" font-size=(token)"color.ink""#);
    assert!(
        codes(&wrong_type).contains(&"token.incompatible_property"),
        "{:?}",
        wrong_type.diagnostics
    );
}

#[test]
fn unknown_node_suppresses_kind_checks() {
    let report = check(r#"override node="ghost" gap=(px)8"#);
    assert!(codes(&report).contains(&"variant.override_unknown_node"));
    assert!(
        messages(&report, "variant.override_unsupported_property").is_empty(),
        "{:?}",
        report.diagnostics
    );
}
