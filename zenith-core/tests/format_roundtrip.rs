//! Integration tests for the canonical writer: roundtrip.
//!
//! Idempotency, AST round-trip equality, number/literal formatting, canonical
//! property ordering, booleans, and `doc-id` — the writer's document-level core.
//!
//! Moved verbatim from the former in-`src` `format/writer/tests.rs`; the body of
//! every test is unchanged — only import paths were rewritten to the public
//! `zenith_core` surface. Span-stripping helpers live in `common`.

mod common;

use common::*;
use zenith_core::format::format_document;

/// A minimal `.zen` document used as the idempotency fixture.
const MINIMAL: &str = r##"zenith version=1 {
  project id="proj.test" name="Test Project"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#f8fafc"
    token id="size.title" type="dimension" value=(pt)48
    token id="font.weight.bold" type="fontWeight" value=700
    token id="lh.body" type="number" value=1.45
  }
  styles {
  }
  document id="doc.test" title="Test Doc" {
    page id="page.one" name="One" w=(px)640 h=(px)360 background=(token)"color.bg" {
      rect id="bg.rect" x=(px)0 y=(px)0 w=(px)640 h=(px)360 fill=(token)"color.bg"
      text id="label" x=(px)10 y=(px)10 w=(px)200 h=(px)50 align="center" fill=(token)"color.text" {
        span "Hello Zenith"
      }
    }
  }
}
"##;

/// A `.zen` document with a `code` node whose content stresses every escape
/// path: leading spaces, a blank line, a tab, an embedded quote, and a
/// literal backslash.
const CODE_DOC: &str = r##"zenith version=1 {
  project id="proj.code" name="Code Project"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.code" title="Code Doc" {
    page id="page.one" w=(px)640 h=(px)360 {
      code id="snippet" x=(px)96 y=(px)320 w=(px)560 h=(px)180 overflow="clip" language="rust" line-numbers=#false tab-width=4 {
        content "fn main() {\n    let path = \"c:\\\\tmp\";\n\n\tprintln!(\"hi\");\n}"
      }
    }
  }
}
"##;

/// **Idempotency test**: format once → `s1`; format again → `s2`; assert equal.
#[test]
fn test_idempotency() {
    let adapter = KdlAdapter;
    let doc1 = adapter
        .parse(MINIMAL.as_bytes())
        .expect("parse 1 must succeed");
    let s1 = format_document(&doc1).expect("format 1 must succeed");

    let doc2 = adapter.parse(&s1).expect("parse 2 must succeed");
    let s2 = format_document(&doc2).expect("format 2 must succeed");

    assert_eq!(
        String::from_utf8(s1.clone()).unwrap(),
        String::from_utf8(s2).unwrap(),
        "format must be idempotent"
    );
}

/// **Round-trip AST equality**: parse → format → parse must yield the same AST
/// (excluding source spans, which reflect byte positions in the original source).
#[test]
fn test_round_trip_ast_equality() {
    let adapter = KdlAdapter;
    let doc_orig = adapter.parse(MINIMAL.as_bytes()).expect("original parse");
    let formatted = format_document(&doc_orig).expect("format");
    let doc_reparsed = adapter.parse(&formatted).expect("re-parse after format");

    // Compare with spans stripped — spans are byte-position metadata that
    // legitimately differ between the original source and the reformatted
    // canonical form; they are not part of the document semantics.
    let orig_stripped = strip_spans(doc_orig);
    let reparsed_stripped = strip_spans(doc_reparsed);
    assert_eq!(
        orig_stripped, reparsed_stripped,
        "re-parsed AST must equal original (spans excluded)"
    );
}

/// **`baseline-grid` round-trip**: a page's `baseline-grid=(px)14` must survive
/// parse → format → parse, mirroring the `bleed` dimension round-trip.
#[test]
fn test_baseline_grid_round_trips() {
    let src = r##"zenith version=1 {
  project id="proj.bg" name="BG"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.bg" title="BG" {
    page id="page.one" w=(px)640 h=(px)360 baseline-grid=(px)14 {
      rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"c"
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let page = &doc.body.pages[0];
    assert!(
        page.baseline_grid.is_some(),
        "baseline-grid must parse onto the page"
    );

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");
    assert!(
        formatted_str.contains("baseline-grid=(px)14"),
        "formatted output must contain baseline-grid; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse");
    assert_eq!(
        strip_spans(doc).body.pages[0].baseline_grid,
        strip_spans(reparsed).body.pages[0].baseline_grid,
        "baseline-grid must round-trip identically"
    );
}

/// **`font-size-min` round-trip**: a text node's `font-size-min=(token)"…"`
/// (the `overflow="autofit"` floor) must survive parse → format → parse,
/// mirroring `font-size`.
#[test]
fn test_font_size_min_round_trips() {
    let src = r##"zenith version=1 {
  project id="proj.fsm" name="FSM"
  tokens format="zenith-token-v1" {
    token id="size.title.min" type="dimension" value=(px)12
  }
  styles {
  }
  document id="doc.fsm" title="FSM" {
    page id="page.one" w=(px)640 h=(px)360 {
      text id="t" x=(px)0 y=(px)0 w=(px)200 h=(px)40 overflow="autofit" font-size-min=(token)"size.title.min" {
        span "Hi"
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
        formatted_str.contains(r#"font-size-min=(token)"size.title.min""#),
        "formatted output must contain font-size-min; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse");
    assert_eq!(
        strip_spans(doc).body.pages[0].children,
        strip_spans(reparsed).body.pages[0].children,
        "font-size-min must round-trip identically"
    );
}

/// **Code content verbatim round-trip**: parse → format → parse must yield a
/// BYTE-IDENTICAL content blob, and the formatter must be idempotent.
#[test]
fn test_code_content_verbatim_round_trip() {
    let adapter = KdlAdapter;
    let doc1 = adapter.parse(CODE_DOC.as_bytes()).expect("parse 1");

    // Decoded content captured from the first parse.
    let original = match &doc1.body.pages[0].children[0] {
        Node::Code(c) => c.content.clone(),
        other => panic!("expected Code node, got {other:?}"),
    };
    // Sanity: the fixture really exercises every escape class.
    assert!(original.contains('\n') && original.contains('\t'));
    assert!(original.contains('"') && original.contains('\\'));

    let s1 = format_document(&doc1).expect("format 1");
    let doc2 = adapter.parse(&s1).expect("parse 2");
    let reparsed = match &doc2.body.pages[0].children[0] {
        Node::Code(c) => c.content.clone(),
        other => panic!("expected Code node, got {other:?}"),
    };
    assert_eq!(
        original, reparsed,
        "code content must round-trip byte-identically"
    );

    // Idempotency: format(format(doc)) == format(doc).
    let s2 = format_document(&doc2).expect("format 2");
    assert_eq!(
        String::from_utf8(s1).unwrap(),
        String::from_utf8(s2).unwrap(),
        "code formatting must be idempotent"
    );
}

/// **Number formatting**: token `(pt)48` must round-trip as `(pt)48`.
#[test]
fn test_pt_48_round_trips() {
    let src = r##"zenith version=1 {
  project id="proj.t" name="T"
  tokens format="zenith-token-v1" {
    token id="size.title" type="dimension" value=(pt)48
  }
  styles {
  }
  document id="doc.t" title="T" {
    page id="p" w=(px)100 h=(px)100 {
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let out = format_document(&doc).expect("format");
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("value=(pt)48"),
        "expected `value=(pt)48` in output, got:\n{text}"
    );
    assert!(
        !text.contains("(pt)48.0"),
        "must not contain (pt)48.0 in output"
    );
}

/// **Literal visual dimension round-trip**: a `stroke-width=(px)2` literal
/// must format as `(px)2` (not `(px)2.0`, not `"2"`) and re-parse back to a
/// `Dimension(2.0, Px)`.
#[test]
fn test_literal_dimension_round_trips() {
    use zenith_core::{Dimension, Node, PropertyValue, Unit};
    let src = r##"zenith version=1 {
  project id="proj.ld" name="LD"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.ld" title="LD" {
    page id="p" w=(px)100 h=(px)100 {
      rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 stroke-width=(px)2
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let out = format_document(&doc).expect("format");
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("stroke-width=(px)2"),
        "expected `stroke-width=(px)2`, got:\n{text}"
    );
    assert!(
        !text.contains("(px)2.0"),
        "must not emit (px)2.0; got:\n{text}"
    );
    assert!(
        !text.contains("stroke-width=\"2\""),
        "must not emit a quoted literal; got:\n{text}"
    );

    // Re-parse the formatted output → still a Dimension(2.0, Px).
    let doc2 = adapter.parse(text.as_bytes()).expect("re-parse");
    match &doc2.body.pages[0].children[0] {
        Node::Rect(r) => assert_eq!(
            r.stroke_width,
            Some(PropertyValue::Dimension(Dimension {
                value: 2.0,
                unit: Unit::Px,
            }))
        ),
        other => panic!("expected Rect, got {other:?}"),
    }
}

/// **Canonical property order**: a rect with `fill` before `x` in source
/// must be formatted with `x` before `fill`.
#[test]
fn test_canonical_property_order_rect() {
    // Source has fill before x — non-canonical order.
    let src = r##"zenith version=1 {
  project id="proj.order" name="Order"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
  }
  styles {
  }
  document id="doc.order" title="Order" {
    page id="p" w=(px)100 h=(px)100 {
      rect id="r" fill=(token)"color.bg" x=(px)10 y=(px)20 w=(px)50 h=(px)50
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let out = format_document(&doc).expect("format");
    let text = String::from_utf8(out).unwrap();

    // Find positions of `x=` and `fill=` in the rect line.
    let rect_line = text
        .lines()
        .find(|l| l.trim_start().starts_with("rect"))
        .expect("must find rect line");
    let pos_x = rect_line.find(" x=").expect("must find x= on rect line");
    let pos_fill = rect_line
        .find(" fill=")
        .expect("must find fill= on rect line");
    assert!(
        pos_x < pos_fill,
        "x= must appear before fill= in canonical output; rect line: {rect_line:?}"
    );
}

/// **Booleans**: `visible=#false` must emit with `#false`, not `false` or `"false"`.
#[test]
fn test_boolean_format() {
    let src = r##"zenith version=1 {
  project id="proj.bool" name="Bool"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.bool" title="Bool" {
    page id="p" w=(px)100 h=(px)100 {
      rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 visible=#false
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let out = format_document(&doc).expect("format");
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("visible=#false"),
        "expected `visible=#false`, got:\n{text}"
    );
}

/// **`doc-id` round-trip**: a root `zenith` node carrying `doc-id="my-doc-123"`
/// must parse onto `doc.doc_id`, be re-emitted verbatim by the formatter, and
/// survive a parse → format → parse cycle with byte-identical output.
#[test]
fn test_doc_id_round_trips() {
    let src = r##"zenith version=1 doc-id="my-doc-123" {
  project id="proj.did" name="DocId"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
  }
  styles {
  }
  document id="doc.did" title="DocId" {
    page id="page.one" w=(px)640 h=(px)360 {
      rect id="r" x=(px)0 y=(px)0 w=(px)640 h=(px)360 fill=(token)"color.bg"
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");

    assert_eq!(
        doc.doc_id.as_deref(),
        Some("my-doc-123"),
        "doc-id must parse onto doc.doc_id"
    );

    let formatted = format_document(&doc).expect("format must succeed");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");

    assert!(
        formatted_str.contains("doc-id=\"my-doc-123\""),
        "formatted output must contain doc-id=\"my-doc-123\"; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse after format");
    assert_eq!(
        doc.doc_id, reparsed.doc_id,
        "doc_id must round-trip identically"
    );

    // Idempotency: format twice → byte-identical output.
    let formatted2 = format_document(&reparsed).expect("format 2 must succeed");
    assert_eq!(
        formatted, formatted2,
        "doc-id formatting must be idempotent"
    );
}

/// **`doc-id` absent is `None`**: a root `zenith` node without a `doc-id`
/// attribute must produce `doc.doc_id == None` after parsing.
#[test]
fn test_doc_id_absent_is_none() {
    let src = r##"zenith version=1 {
  project id="proj.nodid" name="NoDid"
  tokens format="zenith-token-v1" {
    token id="color.bg" type="color" value="#ffffff"
  }
  styles {
  }
  document id="doc.nodid" title="NoDid" {
    page id="page.one" w=(px)640 h=(px)360 {
      rect id="r" x=(px)0 y=(px)0 w=(px)640 h=(px)360 fill=(token)"color.bg"
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");

    assert!(
        doc.doc_id.is_none(),
        "doc_id must be None when doc-id is absent from the zenith node"
    );
}

/// A document with NO assets block → empty AssetBlock (not an error).
#[test]
fn test_absent_assets_block_is_empty() {
    let adapter = KdlAdapter;
    let doc = adapter
        .parse(MINIMAL.as_bytes())
        .expect("parse must succeed");
    assert!(
        doc.assets.assets.is_empty(),
        "absent assets block must yield an empty AssetBlock"
    );
}

#[test]
fn test_text_and_code_kern_pair_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.kern" name="Kern"
  tokens format="zenith-token-v1" {
    token id="size.kern.tight" type="dimension" value=(px)-3
  }
  styles {
  }
  document id="doc.kern" title="Kern" {
    page id="page.one" w=(px)640 h=(px)360 {
      text id="headline" x=(px)10 y=(px)10 w=(px)300 h=(px)80 {
        kern-pair "A" "V" by=(px)-4
        kern-pair "T" "o" by=(token)"size.kern.tight"
        span "AV To"
      }
      code id="snippet" x=(px)10 y=(px)120 w=(px)300 h=(px)80 {
        kern-pair "=" ">" by=(px)-2
        content "a => b"
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
        formatted_str.contains("kern-pair \"A\" \"V\" by=(px)-4"),
        "text literal kern-pair must format canonically; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains("kern-pair \"T\" \"o\" by=(token)\"size.kern.tight\""),
        "text token kern-pair must format canonically; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains("kern-pair \"=\" \">\" by=(px)-2"),
        "code kern-pair must format before content; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse");
    assert_eq!(
        strip_spans(doc),
        strip_spans(reparsed),
        "kern-pair AST must survive parse -> format -> parse"
    );
}

#[test]
fn test_kern_pair_validation_reports_empty_duplicate_and_bad_by() {
    let src = r##"zenith version=1 {
  project id="proj.kern.bad" name="Kern Bad"
  tokens format="zenith-token-v1" {
    token id="color.bad" type="color" value="#ff0000"
  }
  styles {
  }
  document id="doc.kern.bad" title="Kern Bad" {
    page id="page.one" w=(px)640 h=(px)360 {
      text id="bad" x=(px)10 y=(px)10 w=(px)300 h=(px)80 {
        kern-pair "" "V" by="tight"
        kern-pair "A" "V" by=(token)"color.bad"
        kern-pair "T" "o" by=(pct)10
        kern-pair "A" "V" by=(px)-2
        span "AV"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let report = validate(&doc);
    let codes: Vec<_> = report.diagnostics.iter().map(|d| d.code.as_str()).collect();

    assert!(codes.contains(&"kerning.empty_pair"), "got {codes:?}");
    assert!(codes.contains(&"kerning.duplicate_pair"), "got {codes:?}");
    assert!(codes.contains(&"token.raw_visual_literal"), "got {codes:?}");
    assert!(
        codes.contains(&"token.incompatible_property"),
        "got {codes:?}"
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "token.incompatible_property"
                && d.message.contains("pixel-convertible")),
        "expected non-pixel kerning dimension diagnostic; got {:?}",
        report.diagnostics
    );
}
