//! Integration tests for the `pattern` node: parse, format, and round-trip —
//! including the single TEMPLATE `motif` child — plus the missing-motif parse
//! error and the absent-pattern byte-identical guarantee.

mod common;

use common::*;
use zenith_core::format::format_document;

/// **Pattern parse + format + round-trip (with motif)**: a `pattern` with the
/// pattern-specific props (kind/seed/count/spacing/jitter), geometry, fill, and
/// an `ellipse` motif child parses into the expected `PatternNode` (motif as the
/// right Node kind with its fields), formats back out preserving everything, and
/// survives a format → re-parse round-trip (spans stripped).
#[test]
fn pattern_parse_format_round_trip_with_motif() {
    let src = r##"zenith version=1 {
  project id="proj.pat" name="Pat"
  tokens format="zenith-token-v1" {
    token id="color.dot" type="color" value="#334155"
  }
  styles {
  }
  document id="doc.pat" title="Pat" {
    page id="page.pat" w=(px)800 h=(px)600 {
      pattern id="p.dots" kind="grid" x=(px)0 y=(px)0 w=(px)800 h=(px)600 seed=7 count=24 spacing=(px)40 jitter=0.3 fill=(token)"color.dot" {
        ellipse id="e.dot" w=(px)8 h=(px)8 fill=(token)"color.dot"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");

    let pat = match &doc.body.pages[0].children[0] {
        Node::Pattern(p) => p,
        other => panic!("expected Pattern node, got {other:?}"),
    };
    assert_eq!(pat.id, "p.dots");
    assert_eq!(pat.kind, "grid");
    assert_eq!(pat.seed, Some(7));
    assert_eq!(pat.count, Some(24));
    assert_eq!(
        pat.spacing,
        Some(Dimension {
            value: 40.0,
            unit: Unit::Px
        })
    );
    assert_eq!(pat.jitter, Some(0.3));
    assert_eq!(pat.fill, Some(token_ref("color.dot")));
    assert_eq!(
        pat.w,
        Some(Dimension {
            value: 800.0,
            unit: Unit::Px
        })
    );

    // The motif is parsed as the right Node kind with its own fields.
    match pat.motif.as_ref() {
        Node::Ellipse(e) => {
            assert_eq!(e.id, "e.dot");
            assert_eq!(
                e.w,
                Some(Dimension {
                    value: 8.0,
                    unit: Unit::Px
                })
            );
            assert_eq!(e.fill, Some(token_ref("color.dot")));
        }
        other => panic!("expected Ellipse motif, got {other:?}"),
    }

    // The formatter emits the pattern-specific props and the motif block.
    let formatted = format_document(&doc).expect("format must succeed");
    let formatted_str = String::from_utf8(formatted.clone()).expect("formatted must be utf8");
    assert!(
        formatted_str.contains("pattern id=\"p.dots\" kind=\"grid\""),
        "formatter must emit pattern id + kind; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains("seed=7"),
        "formatter must emit seed; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains("count=24"),
        "formatter must emit count; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains("spacing=(px)40"),
        "formatter must emit spacing; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains("jitter=0.3"),
        "formatter must emit jitter; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains("ellipse id=\"e.dot\""),
        "formatter must emit the motif block; got:\n{formatted_str}"
    );

    // Round-trip: re-parse equals the first parse (spans stripped).
    let reparsed = adapter.parse(&formatted).expect("re-parse after format");
    assert_eq!(
        strip_spans(doc),
        strip_spans(reparsed),
        "pattern (with motif) must round-trip identically"
    );
}

/// **Pattern with NO motif child is a parse error**: a `pattern` with an empty
/// (or absent) children block has no template motif and must fail to parse.
#[test]
fn pattern_without_motif_is_parse_error() {
    let src = r##"zenith version=1 {
  project id="proj.nomotif" name="NoMotif"
  styles {
  }
  document id="doc.nomotif" title="NoMotif" {
    page id="page.nomotif" w=(px)400 h=(px)300 {
      pattern id="p.empty" kind="grid" x=(px)0 y=(px)0 w=(px)400 h=(px)300
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let result = adapter.parse(src.as_bytes());
    assert!(
        result.is_err(),
        "a pattern with no motif child must be a parse error"
    );
}

/// **Absent pattern is byte-identical**: a document that uses NO `pattern` node
/// formats exactly as it did before the feature existed (additive guarantee).
#[test]
fn absent_pattern_byte_identical() {
    let src = r##"zenith version=1 {
  project id="proj.np" name="NoPattern"
  tokens format="zenith-token-v1" {
    token id="color.fill" type="color" value="#112233"
  }
  styles {
  }
  document id="doc.np" title="NoPattern" {
    page id="page.np" w=(px)400 h=(px)300 {
      rect id="r.one" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"color.fill"
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let formatted = format_document(&doc).expect("format must succeed");
    let formatted_str = String::from_utf8(formatted).expect("formatted must be utf8");
    assert!(
        !formatted_str.contains("pattern"),
        "a document without a pattern must not emit the keyword; got:\n{formatted_str}"
    );
}
