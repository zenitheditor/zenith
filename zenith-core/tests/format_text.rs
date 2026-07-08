//! Integration tests for the canonical writer: text.
//!
//! Text- and code-node typography attributes (font-weight, chain, exclusion,
//! drop-cap, hyphenate, tab-leader, bullets, footnotes, spans) — round-trip.
//!
//! Moved verbatim from the former in-`src` `format/writer/tests.rs`; the body of
//! every test is unchanged — only import paths were rewritten to the public
//! `zenith_core` surface. Span-stripping helpers live in `common`.

mod common;

use common::*;
use zenith_core::format::format_document;

/// **font-weight round-trip + ordering**: a text node with a `font-weight`
/// token must survive parse→format→parse, and the formatter must place
/// `font-weight` immediately AFTER `font-size` in the canonical output.
#[test]
fn test_text_font_weight_round_trip_and_order() {
    use zenith_core::{Node, PropertyValue};
    let src = r##"zenith version=1 {
  project id="proj.fw" name="FW"
  tokens format="zenith-token-v1" {
    token id="size.body" type="dimension" value=(px)16
    token id="weight.bold" type="fontWeight" value=700
  }
  styles {
  }
  document id="doc.fw" title="FW" {
    page id="p" w=(px)100 h=(px)100 {
      text id="t" x=(px)0 y=(px)0 w=(px)80 h=(px)40 font-size=(token)"size.body" font-weight=(token)"weight.bold" {
        span "Bold"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let out = format_document(&doc).expect("format");
    let text = String::from_utf8(out).unwrap();

    // Canonical order: font-weight comes immediately after font-size.
    let text_line = text
        .lines()
        .find(|l| l.trim_start().starts_with("text"))
        .expect("must find text line");
    let pos_size = text_line.find(" font-size=").expect("must find font-size=");
    let pos_weight = text_line
        .find(" font-weight=")
        .expect("must find font-weight=");
    assert!(
        pos_size < pos_weight,
        "font-weight must follow font-size; text line: {text_line:?}"
    );

    // Round-trip: re-parse preserves the font_weight token ref.
    let doc2 = adapter.parse(text.as_bytes()).expect("re-parse");
    match &doc2.body.pages[0].children[0] {
        Node::Text(t) => assert_eq!(
            t.font_weight,
            Some(PropertyValue::TokenRef("weight.bold".to_owned())),
            "font-weight must survive the format round-trip"
        ),
        other => panic!("expected Text, got {other:?}"),
    }
}

/// **chain round-trip**: a text node carrying `chain="article"` must survive
/// parse→format→parse, with `chain` emitted on the text line (after `style`)
/// and re-parsed back into the `chain` field.
#[test]
fn test_text_chain_round_trip() {
    use zenith_core::Node;
    let src = r##"zenith version=1 {
  project id="proj.ch" name="CH"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.ch" title="CH" {
    page id="p" w=(px)100 h=(px)100 {
      text id="t1" x=(px)0 y=(px)0 w=(px)80 h=(px)40 chain="article" {
        span "Hello world"
      }
      text id="t2" x=(px)0 y=(px)50 w=(px)80 h=(px)40 chain="article" {
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let out = format_document(&doc).expect("format");
    let text = String::from_utf8(out).unwrap();

    assert!(
        text.contains(" chain=\"article\""),
        "chain attr must be emitted; got:\n{text}"
    );

    let doc2 = adapter.parse(text.as_bytes()).expect("re-parse");
    let page = &doc2.body.pages[0];
    for child in &page.children {
        match child {
            Node::Text(t) => assert_eq!(
                t.chain.as_deref(),
                Some("article"),
                "chain must survive the format round-trip"
            ),
            other => panic!("expected Text, got {other:?}"),
        }
    }
}

/// **text-exclusion round-trip**: a text node carrying
/// `text-exclusion="portrait"` must survive parse→format→parse, with the attr
/// emitted on the text line and re-parsed back into the `text_exclusion` field.
#[test]
fn test_text_exclusion_round_trip() {
    use zenith_core::Node;
    let src = r##"zenith version=1 {
  project id="proj.ex" name="EX"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.ex" title="EX" {
    page id="p" w=(px)200 h=(px)200 {
      rect id="portrait" x=(px)0 y=(px)0 w=(px)80 h=(px)80
      text id="t1" x=(px)0 y=(px)0 w=(px)180 h=(px)180 text-exclusion="portrait" {
        span "Hello world"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let out = format_document(&doc).expect("format");
    let text = String::from_utf8(out).unwrap();

    assert!(
        text.contains(" text-exclusion=\"portrait\""),
        "text-exclusion attr must be emitted; got:\n{text}"
    );

    let doc2 = adapter.parse(text.as_bytes()).expect("re-parse");
    let page = &doc2.body.pages[0];
    let mut saw_text = false;
    for child in &page.children {
        if let Node::Text(t) = child {
            assert_eq!(
                t.text_exclusion.as_deref(),
                Some("portrait"),
                "text-exclusion must survive the format round-trip"
            );
            saw_text = true;
        }
    }
    assert!(saw_text, "expected a Text node in the re-parsed page");
}

/// **drop-cap-lines round-trip**: a text node carrying `drop-cap-lines=3` must
/// survive parse→format→parse, with the attr emitted on the text line and
/// re-parsed back into the `drop_cap_lines` field.
#[test]
fn test_text_drop_cap_lines_round_trip() {
    use zenith_core::Node;
    let src = r##"zenith version=1 {
  project id="proj.dc" name="DC"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.dc" title="DC" {
    page id="p" w=(px)100 h=(px)100 {
      text id="t1" x=(px)0 y=(px)0 w=(px)80 h=(px)40 drop-cap-lines=3 {
        span "Hello world"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let out = format_document(&doc).expect("format");
    let text = String::from_utf8(out).unwrap();

    assert!(
        text.contains(" drop-cap-lines=3"),
        "drop-cap-lines attr must be emitted; got:\n{text}"
    );

    let doc2 = adapter.parse(text.as_bytes()).expect("re-parse");
    let page = &doc2.body.pages[0];
    match &page.children[0] {
        Node::Text(t) => assert_eq!(
            t.drop_cap_lines,
            Some(3),
            "drop-cap-lines must survive the format round-trip"
        ),
        other => panic!("expected Text, got {other:?}"),
    }
}

/// **hyphenate + widow-orphan round-trip**: a text node carrying
/// `hyphenate=#true` and `widow-orphan=2` must survive parse→format→parse, with
/// both attrs emitted and re-parsed into their fields.
#[test]
fn test_text_hyphenate_widow_orphan_round_trip() {
    use zenith_core::Node;
    let src = r##"zenith version=1 {
  project id="proj.hy" name="HY"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.hy" title="HY" {
    page id="p" w=(px)100 h=(px)100 {
      text id="t1" x=(px)0 y=(px)0 w=(px)80 h=(px)40 hyphenate=#true widow-orphan=2 {
        span "Hello world"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let out = format_document(&doc).expect("format");
    let text = String::from_utf8(out).unwrap();

    assert!(
        text.contains(" hyphenate=#true"),
        "hyphenate attr must be emitted; got:\n{text}"
    );
    assert!(
        text.contains(" widow-orphan=2"),
        "widow-orphan attr must be emitted; got:\n{text}"
    );

    let doc2 = adapter.parse(text.as_bytes()).expect("re-parse");
    let page = &doc2.body.pages[0];
    match &page.children[0] {
        Node::Text(t) => {
            assert_eq!(
                t.hyphenate,
                Some(true),
                "hyphenate must survive the format round-trip"
            );
            assert_eq!(
                t.widow_orphan,
                Some(2),
                "widow-orphan must survive the format round-trip"
            );
        }
        other => panic!("expected Text, got {other:?}"),
    }
}

/// **tab-leader round-trip**: a text node carrying `tab-leader="."` must survive
/// parse→format→parse, with the attr emitted and re-parsed into its field.
#[test]
fn test_text_tab_leader_round_trip() {
    use zenith_core::Node;
    let src = r##"zenith version=1 {
  project id="proj.tl" name="TL"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.tl" title="TL" {
    page id="p" w=(px)100 h=(px)100 {
      text id="t1" x=(px)0 y=(px)0 w=(px)80 h=(px)40 tab-leader="." {
        span "Chapter One\t1"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let out = format_document(&doc).expect("format");
    let text = String::from_utf8(out).unwrap();

    assert!(
        text.contains(" tab-leader=\".\""),
        "tab-leader attr must be emitted; got:\n{text}"
    );

    let doc2 = adapter.parse(text.as_bytes()).expect("re-parse");
    let page = &doc2.body.pages[0];
    match &page.children[0] {
        Node::Text(t) => assert_eq!(
            t.tab_leader.as_deref(),
            Some("."),
            "tab-leader must survive the format round-trip"
        ),
        other => panic!("expected Text, got {other:?}"),
    }
}

/// **frame grid columns/rows round-trip**: a frame carrying `layout="grid"
/// columns=2 rows=3` must survive parse→format→parse, with the attrs emitted
/// near `layout` and re-parsed into the `columns`/`rows` fields.
#[test]
fn test_frame_grid_columns_rows_round_trip() {
    use zenith_core::Node;
    let src = r##"zenith version=1 {
  project id="proj.gr" name="GR"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.gr" title="GR" {
    page id="p" w=(px)400 h=(px)400 {
      frame id="f1" x=(px)0 y=(px)0 w=(px)300 h=(px)300 layout="grid" columns=2 rows=3 {
        rect id="r0"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let out = format_document(&doc).expect("format");
    let text = String::from_utf8(out).unwrap();

    assert!(
        text.contains(" columns=2"),
        "columns attr must be emitted; got:\n{text}"
    );
    assert!(
        text.contains(" rows=3"),
        "rows attr must be emitted; got:\n{text}"
    );

    let doc2 = adapter.parse(text.as_bytes()).expect("re-parse");
    let page = &doc2.body.pages[0];
    match &page.children[0] {
        Node::Frame(f) => {
            assert_eq!(f.columns, Some(2), "columns must survive the round-trip");
            assert_eq!(f.rows, Some(3), "rows must survive the round-trip");
        }
        other => panic!("expected Frame, got {other:?}"),
    }
}

/// **text contrast-bg round-trip**: a text node carrying
/// `contrast-bg=(token)"color.photo.shadow"` must survive parse→format→parse,
/// with the attr emitted near `fill` and re-parsed into the `contrast_bg` field.
#[test]
fn test_text_contrast_bg_round_trip() {
    use zenith_core::Node;
    use zenith_core::PropertyValue;
    let src = r##"zenith version=1 {
  project id="proj.cb" name="CB"
  tokens format="zenith-token-v1" {
    token id="color.photo.shadow" type="color" value="#101010"
  }
  styles {
  }
  document id="doc.cb" title="CB" {
    page id="p" w=(px)100 h=(px)100 {
      text id="t1" x=(px)0 y=(px)0 w=(px)80 h=(px)40 contrast-bg=(token)"color.photo.shadow" {
        span "Cover line"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let out = format_document(&doc).expect("format");
    let text = String::from_utf8(out).unwrap();

    assert!(
        text.contains(" contrast-bg=(token)\"color.photo.shadow\""),
        "contrast-bg attr must be emitted; got:\n{text}"
    );

    let doc2 = adapter.parse(text.as_bytes()).expect("re-parse");
    let page = &doc2.body.pages[0];
    match &page.children[0] {
        Node::Text(t) => assert_eq!(
            t.contrast_bg,
            Some(PropertyValue::TokenRef("color.photo.shadow".to_owned())),
            "contrast-bg must survive the format round-trip"
        ),
        other => panic!("expected Text, got {other:?}"),
    }
}

/// **code font-weight round-trip + ordering**: a code node with a `font-weight`
/// token must survive parse→format→parse, and the formatter must place
/// `font-weight` immediately AFTER `font-size` in the canonical output.
#[test]
fn test_code_font_weight_round_trip_and_order() {
    use zenith_core::{Node, PropertyValue};
    let src = r##"zenith version=1 {
  project id="proj.cfw" name="CFW"
  tokens format="zenith-token-v1" {
    token id="size.mono" type="dimension" value=(px)14
    token id="weight.bold" type="fontWeight" value=700
  }
  styles {
  }
  document id="doc.cfw" title="CFW" {
    page id="p" w=(px)400 h=(px)300 {
      code id="c" x=(px)0 y=(px)0 font-size=(token)"size.mono" font-weight=(token)"weight.bold" {
        content "let x = 1;"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let out = format_document(&doc).expect("format");
    let text = String::from_utf8(out).unwrap();

    // Canonical order: font-weight comes immediately after font-size.
    let code_line = text
        .lines()
        .find(|l| l.trim_start().starts_with("code"))
        .expect("must find code line");
    let pos_size = code_line.find(" font-size=").expect("must find font-size=");
    let pos_weight = code_line
        .find(" font-weight=")
        .expect("must find font-weight=");
    assert!(
        pos_size < pos_weight,
        "font-weight must follow font-size in code node; code line: {code_line:?}"
    );

    // Round-trip: re-parse preserves the font_weight token ref.
    let doc2 = adapter.parse(text.as_bytes()).expect("re-parse");
    match &doc2.body.pages[0].children[0] {
        Node::Code(c) => assert_eq!(
            c.font_weight,
            Some(PropertyValue::TokenRef("weight.bold".to_owned())),
            "font-weight must survive the code format round-trip"
        ),
        other => panic!("expected Code, got {other:?}"),
    }
}

/// **Span vertical-align round-trip**: a `span ... vertical-align="super"` parses,
/// formats into the canonical text, and survives parse → format → parse.
#[test]
fn test_span_vertical_align_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.va" name="VA"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.va" title="VA" {
    page id="page.one" w=(px)400 h=(px)400 {
      text id="body" x=(px)10 y=(px)10 w=(px)300 h=(px)100 {
        span "E = mc"
        span "2" vertical-align="super"
        span "; H"
        span "2" vertical-align="sub"
        span "O"
      }
    }
  }
}

"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    let page = &doc.body.pages[0];
    let zenith_core::Node::Text(text_node) = &page.children[0] else {
        panic!("expected a text node");
    };
    assert_eq!(
        text_node.spans[1].vertical_align.as_deref(),
        Some("super"),
        "second span must be superscript"
    );
    assert_eq!(
        text_node.spans[3].vertical_align.as_deref(),
        Some("sub"),
        "fourth span must be subscript"
    );
    assert_eq!(
        text_node.spans[0].vertical_align, None,
        "a plain span must have no vertical-align"
    );

    // Canonical form preserves both vertical-align attributes verbatim.
    let formatted = format_document(&doc).expect("format");
    let text = String::from_utf8(formatted).expect("utf8");
    assert!(
        text.contains("vertical-align=\"super\""),
        "formatted output must carry super vertical-align; got:\n{text}"
    );
    assert!(
        text.contains("vertical-align=\"sub\""),
        "formatted output must carry sub vertical-align; got:\n{text}"
    );

    // Round-trip AST equality. `strip_spans` only zeroes node SOURCE spans
    // (the `vertical_align` content field on `TextSpan` is untouched), so this
    // still proves vertical-align survives the round-trip.
    let reparsed = adapter
        .parse(text.as_bytes())
        .expect("re-parse after format");
    assert_eq!(
        strip_spans(doc),
        strip_spans(reparsed),
        "span vertical-align must survive parse → format → parse"
    );
}

/// **OpenType font-features round-trip**: node, code, and span feature lists
/// parse, format canonically, and survive parse -> format -> parse.
#[test]
fn test_font_features_round_trip() {
    use zenith_core::Node;

    let src = r##"zenith version=1 {
  project id="proj.features" name="Features"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.features" title="Features" {
    page id="page.one" w=(px)400 h=(px)400 {
      text id="body" x=(px)10 y=(px)10 w=(px)300 h=(px)100 font-features="liga=0,kern=1" {
        span "Serif" font-features="ss01=1"
      }
      code id="code" x=(px)10 y=(px)140 w=(px)300 h=(px)100 font-features="calt=0" {
        content "let x = 1;"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    let page = &doc.body.pages[0];
    let Node::Text(text_node) = &page.children[0] else {
        panic!("expected text node");
    };
    assert_eq!(text_node.font_features.as_deref(), Some("liga=0,kern=1"));
    assert_eq!(text_node.spans[0].font_features.as_deref(), Some("ss01=1"));
    let Node::Code(code_node) = &page.children[1] else {
        panic!("expected code node");
    };
    assert_eq!(code_node.font_features.as_deref(), Some("calt=0"));

    let formatted = format_document(&doc).expect("format");
    let text = String::from_utf8(formatted).expect("utf8");
    assert!(text.contains("font-features=\"liga=0,kern=1\""));
    assert!(text.contains("span \"Serif\" font-features=\"ss01=1\""));
    assert!(text.contains("font-features=\"calt=0\""));

    let reparsed = adapter.parse(text.as_bytes()).expect("reparse");
    assert_eq!(strip_spans(doc), strip_spans(reparsed));
}

/// **Letter spacing round-trip**: canonical `letter-spacing` parses on text,
/// code, and span surfaces, while the craft alias `tracking` formats back to
/// canonical `letter-spacing`.
#[test]
fn test_letter_spacing_round_trip_and_tracking_alias() {
    use zenith_core::Node;

    let src = r##"zenith version=1 {
  project id="proj.spacing" name="Spacing"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.spacing" title="Spacing" {
    page id="page.one" w=(px)400 h=(px)400 {
      text id="body" x=(px)10 y=(px)10 w=(px)300 h=(px)100 tracking=(px)1.5 {
        span "Serif" letter-spacing=(px)-0.25
      }
      code id="code" x=(px)10 y=(px)140 w=(px)300 h=(px)100 letter-spacing=(px)2 {
        content "let x = 1;"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    let page = &doc.body.pages[0];
    let Node::Text(text_node) = &page.children[0] else {
        panic!("expected text node");
    };
    assert!(text_node.letter_spacing.is_some());
    assert!(text_node.spans[0].letter_spacing.is_some());
    let Node::Code(code_node) = &page.children[1] else {
        panic!("expected code node");
    };
    assert!(code_node.letter_spacing.is_some());

    let formatted = format_document(&doc).expect("format");
    let text = String::from_utf8(formatted).expect("utf8");
    assert!(text.contains("letter-spacing=(px)1.5"));
    assert!(text.contains("span \"Serif\" letter-spacing=(px)-0.25"));
    assert!(text.contains("letter-spacing=(px)2"));
    assert!(!text.contains("tracking="));

    let reparsed = adapter.parse(text.as_bytes()).expect("reparse");
    assert_eq!(strip_spans(doc), strip_spans(reparsed));
}

/// **Span data-ref + format round-trip**: a `span "" data-ref="rev"
/// format="currency" precision=2` parses to the data fields and formats back
/// identically; a plain span (no data-ref) stays byte-identical.
#[test]
fn test_span_data_ref_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.db" name="DB"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.db" title="DB" {
    page id="page.one" w=(px)400 h=(px)200 {
      text id="body" x=(px)10 y=(px)10 w=(px)300 h=(px)100 {
        span "$0.00" data-ref="rev" format="currency" precision=2
        span " plain"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    let page = &doc.body.pages[0];
    let zenith_core::Node::Text(text_node) = &page.children[0] else {
        panic!("expected a text node");
    };
    let span0 = &text_node.spans[0];
    assert_eq!(
        span0.data_ref.as_deref(),
        Some("rev"),
        "data-ref must parse"
    );
    assert_eq!(
        span0.data_format,
        Some(zenith_core::DataFormat::Currency {
            locale: None,
            precision: Some(2)
        }),
        "format=currency precision=2 must parse into a DataFormat"
    );
    // A span with no data-ref must carry neither data field.
    assert_eq!(text_node.spans[1].data_ref, None);
    assert_eq!(text_node.spans[1].data_format, None);

    // Canonical form carries the data-ref + format attrs verbatim.
    let formatted = format_document(&doc).expect("format");
    let text = String::from_utf8(formatted).expect("utf8");
    assert!(
        text.contains("data-ref=\"rev\""),
        "formatted output must carry data-ref; got:\n{text}"
    );
    assert!(
        text.contains("format=\"currency\""),
        "formatted output must carry the currency format; got:\n{text}"
    );
    assert!(
        text.contains("precision=2"),
        "formatted output must carry the precision; got:\n{text}"
    );
    // The plain span must NOT gain any data attrs (byte-identical when absent).
    assert!(
        text.contains("span \" plain\"\n"),
        "plain span must be emitted with no data attrs; got:\n{text}"
    );

    // Round-trip AST equality (strip_spans only zeroes node SOURCE spans, leaving
    // data_ref/data_format on TextSpan untouched).
    let reparsed = adapter
        .parse(text.as_bytes())
        .expect("re-parse after format");
    assert_eq!(
        strip_spans(doc),
        strip_spans(reparsed),
        "span data-ref + format must survive parse → format → parse"
    );
}

/// **Footnote + span footnote-ref round-trip**: a page-level `footnote` node and
/// a `span ... footnote-ref="fn.1"` both parse, format into the canonical text,
/// and survive parse → format → parse.
#[test]
fn test_footnote_and_footnote_ref_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.fn" name="FN"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.fn" title="FN" {
    page id="page.one" w=(px)400 h=(px)600 {
      text id="body" x=(px)10 y=(px)10 w=(px)300 h=(px)100 {
        span "Strong evidence" footnote-ref="fn.1"
        span " supports this."
      }
      footnote id="fn.1" {
        span "See also Chapter 4."
      }
      footnote id="fn.2" marker="*" {
        span "An annotated aside."
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    let page = &doc.body.pages[0];
    let zenith_core::Node::Text(text_node) = &page.children[0] else {
        panic!("expected a text node first");
    };
    assert_eq!(
        text_node.spans[0].footnote_ref.as_deref(),
        Some("fn.1"),
        "first span must carry footnote-ref fn.1"
    );
    assert_eq!(
        text_node.spans[1].footnote_ref, None,
        "second span must have no footnote-ref"
    );

    let zenith_core::Node::Footnote(fn1) = &page.children[1] else {
        panic!("expected a footnote node second");
    };
    assert_eq!(fn1.id, "fn.1");
    assert_eq!(fn1.marker, None, "fn.1 uses the auto-number");
    assert_eq!(fn1.spans[0].text, "See also Chapter 4.");

    let zenith_core::Node::Footnote(fn2) = &page.children[2] else {
        panic!("expected a second footnote node");
    };
    assert_eq!(fn2.marker.as_deref(), Some("*"), "fn.2 has explicit marker");

    // Canonical form preserves the footnote node + the span footnote-ref.
    let formatted = format_document(&doc).expect("format");
    let text = String::from_utf8(formatted).expect("utf8");
    assert!(
        text.contains("footnote id=\"fn.1\""),
        "formatted output must carry the footnote node; got:\n{text}"
    );
    assert!(
        text.contains("footnote-ref=\"fn.1\""),
        "formatted output must carry the span footnote-ref; got:\n{text}"
    );
    assert!(
        text.contains("marker=\"*\""),
        "formatted output must carry the explicit marker; got:\n{text}"
    );

    let reparsed = adapter
        .parse(text.as_bytes())
        .expect("re-parse after format");
    assert_eq!(
        strip_spans(doc),
        strip_spans(reparsed),
        "footnote + footnote-ref must survive parse → format → parse"
    );
}

/// **`overflow-wrap` round-trip**: a text node's `overflow-wrap="break-word"`
/// must be emitted by the writer and survive parse → format → parse.
#[test]
fn test_overflow_wrap_round_trips() {
    let src = r##"zenith version=1 {
  project id="proj.ow" name="OW"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.ow" title="OW" {
    page id="page.one" w=(px)640 h=(px)360 {
      text id="col3" x=(px)10 y=(px)10 w=(px)120 h=(px)200 overflow-wrap="break-word" {
        span "https://very-long.example.com/x"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");
    assert!(
        formatted_str.contains(r#"overflow-wrap="break-word""#),
        "formatted output must contain overflow-wrap; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse");
    assert_eq!(
        strip_spans(doc),
        strip_spans(reparsed),
        "overflow-wrap must round-trip identically"
    );
}

/// **Hanging-indent round-trip**: `padding-left` plus a NEGATIVE `text-indent`
/// must be emitted by the writer (including the minus sign) and survive
/// parse → format → parse byte-identically.
#[test]
fn test_hanging_indent_round_trips() {
    let src = r##"zenith version=1 {
  project id="proj.hi" name="HI"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.hi" title="HI" {
    page id="page.one" w=(px)1920 h=(px)1080 {
      text id="b1" x=(px)160 y=(px)240 w=(px)1600 h=(px)120 overflow="clip" padding-left=(px)44 text-indent=(px)-44 {
        span "• A hanging bullet whose wrapped lines align past the glyph."
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    // The negative text-indent must have parsed onto the node.
    let page = &doc.body.pages[0];
    let Node::Text(t) = &page.children[0] else {
        panic!("first child must be the text node");
    };
    assert_eq!(
        t.padding_left.as_ref().map(|d| d.value),
        Some(44.0),
        "padding-left must parse"
    );
    assert_eq!(
        t.text_indent.as_ref().map(|d| d.value),
        Some(-44.0),
        "negative text-indent must parse"
    );

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");
    assert!(
        formatted_str.contains("padding-left=(px)44"),
        "formatted output must contain padding-left; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains("text-indent=(px)-44"),
        "formatted output must contain the NEGATIVE text-indent; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse");
    assert_eq!(
        strip_spans(doc),
        strip_spans(reparsed),
        "hanging-indent attributes must round-trip identically"
    );
}

/// `bullet="•"` and `bullet-gap=(px)16` must survive parse → format → parse.
/// The writer must emit both attributes on the text line and re-parse them back
/// into the `bullet` / `bullet_gap` fields.
#[test]
fn test_bullet_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.br" name="BR"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.br" title="BR" {
    page id="page.one" w=(px)1920 h=(px)1080 {
      text id="b1" x=(px)160 y=(px)200 w=(px)1600 h=(px)170 overflow="clip" align="start" bullet="•" bullet-gap=(px)16 {
        span "Revenue grew twelve percent year over year, the strongest result since the restructuring."
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    let page = &doc.body.pages[0];
    let Node::Text(t) = &page.children[0] else {
        panic!("first child must be the text node");
    };
    assert_eq!(t.bullet.as_deref(), Some("•"), "bullet must parse from KDL");
    assert_eq!(
        t.bullet_gap.as_ref().map(|d| d.value),
        Some(16.0),
        "bullet-gap must parse from KDL"
    );

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");
    assert!(
        formatted_str.contains("bullet=\"•\""),
        "formatted output must contain bullet; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains("bullet-gap=(px)16"),
        "formatted output must contain bullet-gap; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse");
    assert_eq!(
        strip_spans(doc),
        strip_spans(reparsed),
        "bullet + bullet-gap must round-trip identically"
    );
}
