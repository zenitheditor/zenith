//! Integration tests for the canonical writer: tokens_styles.
//!
//! Token literals (gradient, shadow, filter, mask, duotone), syntax themes, and
//! the `styles` block — parse, serialize, and round-trip.
//!
//! Moved verbatim from the former in-`src` `format/writer/tests.rs`; the body of
//! every test is unchanged — only import paths were rewritten to the public
//! `zenith_core` surface. Span-stripping helpers live in `common`.

mod common;

use common::*;
use zenith_core::format::format_document;

/// **Radial gradient round-trip**: a `radial=#true` gradient token with
/// `center-x`, `center-y`, and `radius` params must survive parse → format →
/// parse with `kind == GradientKind::Radial` and the same params.
#[test]
fn test_radial_gradient_round_trips() {
    use zenith_core::{GradientKind, TokenLiteral, TokenValue};

    let src = r##"zenith version=1 {
  project id="proj.rgt" name="RGT"
  tokens format="zenith-token-v1" {
    token id="color.a" type="color" value="#ffffff"
    token id="color.b" type="color" value="#000000"
    token id="grad.r" type="gradient" radial=#true center-x=0.5 center-y=0.5 radius=0.7 {
      stop offset=0.0 color=(token)"color.a"
      stop offset=1.0 color=(token)"color.b"
    }
  }
  styles {
  }
  document id="doc.rgt" title="RGT" {
    page id="page.rgt" w=(px)100 h=(px)100 {
      rect id="r" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"grad.r"
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    let formatted = format_document(&doc).expect("format");
    let formatted_str = String::from_utf8(formatted.clone()).expect("utf8");

    // Formatted output must contain radial marker and geometry params.
    assert!(
        formatted_str.contains("radial=#true"),
        "formatted output must contain radial=#true; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains("center-x=0.5"),
        "formatted output must contain center-x; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains("center-y=0.5"),
        "formatted output must contain center-y; got:\n{formatted_str}"
    );
    assert!(
        formatted_str.contains("radius=0.7"),
        "formatted output must contain radius; got:\n{formatted_str}"
    );

    let reparsed = adapter.parse(&formatted).expect("re-parse after format");
    let reparsed2 = adapter
        .parse(&format_document(&reparsed).expect("format 2"))
        .expect("re-parse 2");

    // Find the gradient token in the reparsed doc.
    let grad_token = reparsed2
        .tokens
        .tokens
        .iter()
        .find(|t| t.id == "grad.r")
        .expect("grad.r token must survive round-trip");
    let TokenValue::Literal(TokenLiteral::Gradient(g)) = &grad_token.value else {
        panic!(
            "grad.r must be a gradient literal, got {:?}",
            grad_token.value
        );
    };
    assert_eq!(
        g.kind,
        GradientKind::Radial,
        "kind must be Radial after round-trip"
    );
    assert_eq!(g.center_x, Some(0.5));
    assert_eq!(g.center_y, Some(0.5));
    assert_eq!(g.radius, Some(0.7));
    assert_eq!(g.stops.len(), 2);
}

/// **syntax-theme round-trip**: a code node with `syntax-theme="light"`
/// must parse to `Some(SyntaxTheme::Light)` and format back to
/// `syntax-theme="light"` in the canonical position (between font-size and
/// opacity).
#[test]
fn test_syntax_theme_parse_format_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.sth" name="STH"
  tokens format="zenith-token-v1" {
  }
  styles {
  }
  document id="doc.sth" title="STH" {
    page id="page.sth" w=(px)400 h=(px)300 {
      code id="code.sth" x=(px)10 y=(px)10 language="rust" syntax-theme="light" {
        content "let x = 1;"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse must succeed");
    let code_node = match &doc.body.pages[0].children[0] {
        Node::Code(c) => c,
        other => panic!("expected Code node, got {other:?}"),
    };
    use zenith_core::SyntaxTheme;
    assert_eq!(
        code_node.syntax_theme,
        Some(SyntaxTheme::Light),
        "syntax-theme=\"light\" must parse to Some(SyntaxTheme::Light)"
    );

    let formatted = format_document(&doc).expect("format must succeed");
    let formatted_str = String::from_utf8(formatted).expect("formatted must be utf8");
    assert!(
        formatted_str.contains("syntax-theme=\"light\""),
        "formatter must emit syntax-theme=\"light\"; got:\n{formatted_str}"
    );

    // Canonical position: between font-size and opacity. Since neither
    // font-size nor opacity is set in this fixture, just check that
    // syntax-theme appears and re-parses correctly.
    let doc2 = adapter
        .parse(formatted_str.as_bytes())
        .expect("re-parse after format");
    let code2 = match &doc2.body.pages[0].children[0] {
        Node::Code(c) => c,
        other => panic!("expected Code node on re-parse, got {other:?}"),
    };
    assert_eq!(
        code2.syntax_theme,
        Some(SyntaxTheme::Light),
        "syntax-theme must survive a format → re-parse round-trip"
    );
}

/// **Gradient round-trip**: a gradient token (angle + 2 stops) must
/// parse→format→parse byte-stably, emit the `stop` brace block, and a page
/// background referencing it must NOT flag the stop colors as `token.unused`.
#[test]
fn test_gradient_token_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.grad" name="Grad"
  tokens format="zenith-token-v1" {
    token id="color.navy.top" type="color" value="#001133"
    token id="color.black.bottom" type="color" value="#000000"
    token id="gradient.bg.hero" type="gradient" angle=(deg)90 {
      stop offset=0 color=(token)"color.navy.top"
      stop offset=1 color=(token)"color.black.bottom"
    }
  }
  styles {
  }
  document id="doc.grad" title="Grad" {
    page id="p" w=(px)100 h=(px)100 background=(token)"gradient.bg.hero" {
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc1 = adapter.parse(src.as_bytes()).expect("parse 1");
    let s1 = format_document(&doc1).expect("format 1");
    let formatted = String::from_utf8(s1.clone()).expect("utf8");

    // The gradient emits a brace block with two stop children.
    assert!(
        formatted.contains("type=\"gradient\" angle=(deg)90 {"),
        "expected gradient header; got:\n{formatted}"
    );
    assert!(
        formatted.contains("stop offset=0 color=(token)\"color.navy.top\""),
        "expected first stop; got:\n{formatted}"
    );
    assert!(
        formatted.contains("stop offset=1 color=(token)\"color.black.bottom\""),
        "expected second stop; got:\n{formatted}"
    );

    // Idempotency: format(format(doc)) == format(doc).
    let doc2 = adapter.parse(&s1).expect("parse 2");
    let s2 = format_document(&doc2).expect("format 2");
    assert_eq!(
        formatted,
        String::from_utf8(s2).expect("utf8"),
        "gradient formatting must be idempotent"
    );

    // AST round-trip (spans stripped).
    assert_eq!(
        strip_spans(doc1),
        strip_spans(doc2),
        "gradient AST must survive format round-trip"
    );
}

/// **Gradient fill validates clean**: a page background referencing a
/// gradient token type-checks OK, and the gradient's stop colors are not
/// falsely flagged `token.unused`.
#[test]
fn test_gradient_fill_validates_without_unused() {
    let src = r##"zenith version=1 {
  project id="proj.grad" name="Grad"
  tokens format="zenith-token-v1" {
    token id="color.navy.top" type="color" value="#001133"
    token id="color.black.bottom" type="color" value="#000000"
    token id="gradient.bg.hero" type="gradient" angle=(deg)90 {
      stop offset=0 color=(token)"color.navy.top"
      stop offset=1 color=(token)"color.black.bottom"
    }
  }
  styles {
  }
  document id="doc.grad" title="Grad" {
    page id="p" w=(px)100 h=(px)100 {
      rect id="r" x=(px)0 y=(px)0 w=(px)100 h=(px)100 fill=(token)"gradient.bg.hero"
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    // `validate` runs token resolution internally and merges all diagnostics.
    let report = zenith_core::validate(&doc);

    let codes: Vec<&str> = report.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert!(
        !codes.contains(&"token.incompatible_property"),
        "gradient fill must be type-compatible; codes: {codes:?}"
    );
    assert!(
        !codes.contains(&"token.unused"),
        "gradient stop colors must not be flagged unused; codes: {codes:?}"
    );
    assert!(
        !codes.contains(&"token.raw_visual_literal"),
        "gradient token ref must not be a raw literal; codes: {codes:?}"
    );
}

/// **Shadow round-trip**: a shadow token (2 layers) must parse→format→parse
/// byte-stably, emit the `layer` brace block, and a text node referencing it
/// (via `shadow=(token)"..."`) must survive the round-trip.
#[test]
fn test_shadow_token_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.shadow" name="Shadow"
  tokens format="zenith-token-v1" {
    token id="color.shadow.black" type="color" value="#000000"
    token id="color.glow.cyan" type="color" value="#00ffff"
    token id="shadow.headline" type="shadow" {
      layer dx=(px)8 dy=(px)8 blur=(px)24 color=(token)"color.shadow.black"
      layer dx=(px)0 dy=(px)0 blur=(px)20 color=(token)"color.glow.cyan"
    }
  }
  styles {
  }
  document id="doc.shadow" title="Shadow" {
    page id="p" w=(px)100 h=(px)100 {
      text id="headline" x=(px)0 y=(px)0 w=(px)100 h=(px)40 shadow=(token)"shadow.headline" {
        span "Hi"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc1 = adapter.parse(src.as_bytes()).expect("parse 1");
    let s1 = format_document(&doc1).expect("format 1");
    let formatted = String::from_utf8(s1.clone()).expect("utf8");

    // The shadow emits a brace block with two layer children.
    assert!(
        formatted.contains("type=\"shadow\" {"),
        "expected shadow header; got:\n{formatted}"
    );
    assert!(
        formatted
            .contains("layer dx=(px)8 dy=(px)8 blur=(px)24 color=(token)\"color.shadow.black\""),
        "expected first layer; got:\n{formatted}"
    );
    assert!(
        formatted.contains(" shadow=(token)\"shadow.headline\""),
        "expected node shadow prop; got:\n{formatted}"
    );

    // Idempotency.
    let doc2 = adapter.parse(&s1).expect("parse 2");
    let s2 = format_document(&doc2).expect("format 2");
    assert_eq!(
        formatted,
        String::from_utf8(s2).expect("utf8"),
        "shadow formatting must be idempotent"
    );

    // AST round-trip (spans stripped).
    assert_eq!(
        strip_spans(doc1),
        strip_spans(doc2),
        "shadow AST must survive format round-trip"
    );
}

/// **Shadow on a node validates clean**: a text node referencing a shadow
/// token type-checks OK, and the shadow's layer colors are not falsely
/// flagged `token.unused`.
#[test]
fn test_shadow_node_validates_without_unused() {
    let src = r##"zenith version=1 {
  project id="proj.shadow" name="Shadow"
  tokens format="zenith-token-v1" {
    token id="color.shadow.black" type="color" value="#000000"
    token id="color.glow.cyan" type="color" value="#00ffff"
    token id="shadow.headline" type="shadow" {
      layer dx=(px)8 dy=(px)8 blur=(px)24 color=(token)"color.shadow.black"
      layer dx=(px)0 dy=(px)0 blur=(px)20 color=(token)"color.glow.cyan"
    }
  }
  styles {
  }
  document id="doc.shadow" title="Shadow" {
    page id="p" w=(px)100 h=(px)100 {
      text id="headline" x=(px)0 y=(px)0 w=(px)100 h=(px)40 shadow=(token)"shadow.headline" {
        span "Hi"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let report = zenith_core::validate(&doc);

    let codes: Vec<&str> = report.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert!(
        !codes.contains(&"token.incompatible_property"),
        "shadow ref must be type-compatible; codes: {codes:?}"
    );
    assert!(
        !codes.contains(&"token.unused"),
        "shadow layer colors must not be flagged unused; codes: {codes:?}"
    );
    assert!(
        !codes.contains(&"token.raw_visual_literal"),
        "shadow token ref must not be a raw literal; codes: {codes:?}"
    );
}

/// **Filter round-trip**: a filter token (2 ops, one with an `amount`, one
/// without) must parse→format→parse byte-stably, emit the op brace block, and
/// a text node referencing it (via `filter=(token)"..."`) must survive.
#[test]
fn test_filter_token_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.filter" name="Filter"
  tokens format="zenith-token-v1" {
    token id="filter.photo" type="filter" {
      grayscale amount=0.5
      hue-rotate
    }
  }
  styles {
  }
  document id="doc.filter" title="Filter" {
    page id="p" w=(px)100 h=(px)100 {
      text id="headline" x=(px)0 y=(px)0 w=(px)100 h=(px)40 filter=(token)"filter.photo" {
        span "Hi"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc1 = adapter.parse(src.as_bytes()).expect("parse 1");
    let s1 = format_document(&doc1).expect("format 1");
    let formatted = String::from_utf8(s1.clone()).expect("utf8");

    // The filter emits a brace block with two op children.
    assert!(
        formatted.contains("type=\"filter\" {"),
        "expected filter header; got:\n{formatted}"
    );
    assert!(
        formatted.contains("grayscale amount=0.5"),
        "expected grayscale op with amount; got:\n{formatted}"
    );
    assert!(
        formatted.contains(" filter=(token)\"filter.photo\""),
        "expected node filter prop; got:\n{formatted}"
    );

    // Idempotency.
    let doc2 = adapter.parse(&s1).expect("parse 2");
    let s2 = format_document(&doc2).expect("format 2");
    assert_eq!(
        formatted,
        String::from_utf8(s2).expect("utf8"),
        "filter formatting must be idempotent"
    );

    // AST round-trip (spans stripped).
    assert_eq!(
        strip_spans(doc1),
        strip_spans(doc2),
        "filter AST must survive format round-trip"
    );
}

/// **Filter on a node validates clean**: a text node referencing a filter
/// token type-checks OK and is not flagged as a raw literal.
#[test]
fn test_filter_node_validates_clean() {
    let src = r##"zenith version=1 {
  project id="proj.filter" name="Filter"
  tokens format="zenith-token-v1" {
    token id="filter.photo" type="filter" {
      grayscale amount=0.5
      hue-rotate
    }
  }
  styles {
  }
  document id="doc.filter" title="Filter" {
    page id="p" w=(px)100 h=(px)100 {
      text id="headline" x=(px)0 y=(px)0 w=(px)100 h=(px)40 filter=(token)"filter.photo" {
        span "Hi"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let report = zenith_core::validate(&doc);

    let codes: Vec<&str> = report.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert!(
        !codes.contains(&"token.incompatible_property"),
        "filter ref must be type-compatible; codes: {codes:?}"
    );
    assert!(
        !codes.contains(&"token.raw_visual_literal"),
        "filter token ref must not be a raw literal; codes: {codes:?}"
    );
}

/// **Filter prop wrong type**: a node `filter=(token)"x"` where `x` is a color
/// token must produce `token.incompatible_property`.
#[test]
fn test_filter_node_prop_wrong_type() {
    let src = r##"zenith version=1 {
  project id="proj.filter" name="Filter"
  tokens format="zenith-token-v1" {
    token id="color.not-a-filter" type="color" value="#000000"
  }
  styles {
  }
  document id="doc.filter" title="Filter" {
    page id="p" w=(px)100 h=(px)100 {
      text id="headline" x=(px)0 y=(px)0 w=(px)100 h=(px)40 filter=(token)"color.not-a-filter" {
        span "Hi"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let report = zenith_core::validate(&doc);

    let codes: Vec<&str> = report.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert!(
        codes.contains(&"token.incompatible_property"),
        "a non-filter token in a filter slot must be incompatible; codes: {codes:?}"
    );
}

/// **Mask round-trip**: a mask token (a `rounded` shape with `radius`,
/// `feather`, and `invert=#true`) must parse→format→parse byte-stably, emit the
/// shape brace block, and a rect node referencing it (via `mask=(token)"..."`)
/// must survive.
#[test]
fn test_mask_token_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.mask" name="Mask"
  tokens format="zenith-token-v1" {
    token id="mask.vignette" type="mask" {
      rounded radius=40 feather=60 invert=#true
    }
  }
  styles {
  }
  document id="doc.mask" title="Mask" {
    page id="p" w=(px)100 h=(px)100 {
      rect id="card" x=(px)0 y=(px)0 w=(px)100 h=(px)40 mask=(token)"mask.vignette"
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc1 = adapter.parse(src.as_bytes()).expect("parse 1");
    let s1 = format_document(&doc1).expect("format 1");
    let formatted = String::from_utf8(s1.clone()).expect("utf8");

    // The mask emits a brace block with a single shape child.
    assert!(
        formatted.contains("type=\"mask\" {"),
        "expected mask header; got:\n{formatted}"
    );
    assert!(
        formatted.contains("rounded radius=40 feather=60 invert=#true"),
        "expected rounded shape with radius/feather/invert; got:\n{formatted}"
    );
    assert!(
        formatted.contains(" mask=(token)\"mask.vignette\""),
        "expected node mask prop; got:\n{formatted}"
    );

    // Idempotency.
    let doc2 = adapter.parse(&s1).expect("parse 2");
    let s2 = format_document(&doc2).expect("format 2");
    assert_eq!(
        formatted,
        String::from_utf8(s2).expect("utf8"),
        "mask formatting must be idempotent"
    );

    // AST round-trip (spans stripped).
    assert_eq!(
        strip_spans(doc1),
        strip_spans(doc2),
        "mask AST must survive format round-trip"
    );
}

/// **Mask prop wrong type**: a node `mask=(token)"x"` where `x` is a color token
/// must produce `token.incompatible_property`.
#[test]
fn test_mask_node_prop_wrong_type() {
    let src = r##"zenith version=1 {
  project id="proj.mask" name="Mask"
  tokens format="zenith-token-v1" {
    token id="color.not-a-mask" type="color" value="#000000"
  }
  styles {
  }
  document id="doc.mask" title="Mask" {
    page id="p" w=(px)100 h=(px)100 {
      rect id="card" x=(px)0 y=(px)0 w=(px)100 h=(px)40 mask=(token)"color.not-a-mask"
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let report = zenith_core::validate(&doc);

    let codes: Vec<&str> = report.diagnostics.iter().map(|d| d.code.as_str()).collect();
    assert!(
        codes.contains(&"token.incompatible_property"),
        "a non-mask token in a mask slot must be incompatible; codes: {codes:?}"
    );
}

/// **Duotone filter round-trip**: a duotone op carrying both `shadow` and
/// `highlight` color-token refs (plus `amount`) must parse→format→parse
/// byte-stably and emit `duotone shadow=(token)"…" highlight=(token)"…" amount=…`.
#[test]
fn test_duotone_filter_token_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.duo" name="Duo"
  tokens format="zenith-token-v1" {
    token id="color.sh" type="color" value="#000000"
    token id="color.hi" type="color" value="#ffffff"
    token id="filter.duo" type="filter" {
      duotone shadow=(token)"color.sh" highlight=(token)"color.hi" amount=0.8
    }
  }
  styles {
  }
  document id="doc.duo" title="Duo" {
    page id="p" w=(px)100 h=(px)100 {
      text id="headline" x=(px)0 y=(px)0 w=(px)100 h=(px)40 filter=(token)"filter.duo" {
        span "Hi"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc1 = adapter.parse(src.as_bytes()).expect("parse 1");
    let s1 = format_document(&doc1).expect("format 1");
    let formatted = String::from_utf8(s1.clone()).expect("utf8");

    assert!(
        formatted.contains(
            "duotone shadow=(token)\"color.sh\" highlight=(token)\"color.hi\" amount=0.8"
        ),
        "expected duotone op with both colors and amount; got:\n{formatted}"
    );

    // Idempotency.
    let doc2 = adapter.parse(&s1).expect("parse 2");
    let s2 = format_document(&doc2).expect("format 2");
    assert_eq!(
        formatted,
        String::from_utf8(s2).expect("utf8"),
        "duotone formatting must be idempotent"
    );

    // AST round-trip (spans stripped).
    assert_eq!(
        strip_spans(doc1),
        strip_spans(doc2),
        "duotone AST must survive format round-trip"
    );
}

/// **Noise filter round-trip**: a noise op carrying `seed`, `scale`, and
/// `amount` must parse→format→parse byte-stably and emit those props by name in
/// the canonical `seed`/`scale`/`amount` order.
#[test]
fn test_noise_filter_token_round_trip() {
    let src = r##"zenith version=1 {
  project id="proj.noise" name="Noise"
  tokens format="zenith-token-v1" {
    token id="filter.grain" type="filter" {
      noise seed=7 scale=2 amount=0.3
    }
  }
  styles {
  }
  document id="doc.noise" title="Noise" {
    page id="p" w=(px)100 h=(px)100 {
      text id="headline" x=(px)0 y=(px)0 w=(px)100 h=(px)40 filter=(token)"filter.grain" {
        span "Hi"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc1 = adapter.parse(src.as_bytes()).expect("parse 1");
    let s1 = format_document(&doc1).expect("format 1");
    let formatted = String::from_utf8(s1.clone()).expect("utf8");

    assert!(
        formatted.contains("noise seed=7 scale=2 amount=0.3"),
        "expected noise op with seed/scale/amount; got:\n{formatted}"
    );

    // Idempotency.
    let doc2 = adapter.parse(&s1).expect("parse 2");
    let s2 = format_document(&doc2).expect("format 2");
    assert_eq!(
        formatted,
        String::from_utf8(s2).expect("utf8"),
        "noise formatting must be idempotent"
    );

    // AST round-trip (spans stripped).
    assert_eq!(
        strip_spans(doc1),
        strip_spans(doc2),
        "noise AST must survive format round-trip"
    );
}

/// **Duotone color refs are used transitively**: a node referencing a duotone
/// filter token records the duotone's shadow/highlight color tokens as used, so
/// neither is falsely flagged `token.unused`.
#[test]
fn test_duotone_color_refs_not_unused() {
    let src = r##"zenith version=1 {
  project id="proj.duo" name="Duo"
  tokens format="zenith-token-v1" {
    token id="color.sh" type="color" value="#000000"
    token id="color.hi" type="color" value="#ffffff"
    token id="filter.duo" type="filter" {
      duotone shadow=(token)"color.sh" highlight=(token)"color.hi"
    }
  }
  styles {
  }
  document id="doc.duo" title="Duo" {
    page id="p" w=(px)100 h=(px)100 {
      text id="headline" x=(px)0 y=(px)0 w=(px)100 h=(px)40 filter=(token)"filter.duo" {
        span "Hi"
      }
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");
    let report = zenith_core::validate(&doc);

    let unused: Vec<&str> = report
        .diagnostics
        .iter()
        .filter(|d| d.code == "token.unused")
        .filter_map(|d| d.subject_id.as_deref())
        .collect();
    assert!(
        !unused.contains(&"color.sh") && !unused.contains(&"color.hi"),
        "duotone color tokens must be recorded as used; unused: {unused:?}"
    );
}

/// Source document with style properties to test parsing and formatting.
const WITH_STYLES: &str = r##"zenith version=1 {
  project id="proj.styles" name="Styles Test"
  tokens format="zenith-token-v1" {
    token id="color.text.primary" type="color" value="#111827"
    token id="size.text.title" type="dimension" value=(pt)24
    token id="font.family.body" type="fontFamily" value="Noto Sans"
  }
  styles {
    style id="style.text.title" {
      fill (token)"color.text.primary"
      font-family (token)"font.family.body"
      font-size (token)"size.text.title"
    }
  }
  document id="doc.styles" {
    page id="page.one" w=(px)640 h=(px)360 {
    }
  }
}
"##;

/// Style properties are parsed into `Style.properties` with correct canonical keys.
#[test]
fn style_properties_parsed() {
    use zenith_core::PropertyValue;
    let adapter = KdlAdapter;
    let doc = adapter.parse(WITH_STYLES.as_bytes()).expect("parse");

    assert_eq!(doc.styles.styles.len(), 1);
    let style = &doc.styles.styles[0];
    assert_eq!(style.id, "style.text.title");
    assert_eq!(style.properties.len(), 3);

    assert_eq!(
        style.properties.get("fill"),
        Some(&PropertyValue::TokenRef("color.text.primary".to_owned())),
        "fill must be a TokenRef to color.text.primary"
    );
    assert_eq!(
        style.properties.get("font-family"),
        Some(&PropertyValue::TokenRef("font.family.body".to_owned())),
        "font-family must be a TokenRef to font.family.body"
    );
    assert_eq!(
        style.properties.get("font-size"),
        Some(&PropertyValue::TokenRef("size.text.title".to_owned())),
        "font-size must be a TokenRef to size.text.title"
    );
}

/// Underscore variant keys are canonicalized to hyphenated forms.
#[test]
fn style_underscore_keys_canonicalized() {
    use zenith_core::PropertyValue;
    let src = r##"zenith version=1 {
  project id="proj.usk" name="USK"
  tokens format="zenith-token-v1" {
    token id="size.sw" type="dimension" value=(px)2
  }
  styles {
    style id="style.usk" {
      stroke_width (token)"size.sw"
    }
  }
  document id="doc.usk" {
    page id="page.usk" w=(px)100 h=(px)100 {
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    let style = &doc.styles.styles[0];
    assert!(
        style.properties.contains_key("stroke-width"),
        "stroke_width must be stored under canonical key stroke-width"
    );
    assert!(
        !style.properties.contains_key("stroke_width"),
        "underscore key must not appear in properties map"
    );
    assert_eq!(
        style.properties.get("stroke-width"),
        Some(&PropertyValue::TokenRef("size.sw".to_owned()))
    );
}

/// `padding` and `gap` are recognized token-only dimension style props:
/// they parse into `Style.properties` under their canonical keys and survive
/// a parse → format → parse round-trip.
#[test]
fn style_padding_gap_round_trip() {
    use zenith_core::PropertyValue;
    let src = r##"zenith version=1 {
  project id="proj.pg" name="PG"
  tokens format="zenith-token-v1" {
    token id="space.pad" type="dimension" value=(px)16
    token id="space.gap" type="dimension" value=(px)8
  }
  styles {
    style id="style.flow" {
      gap (token)"space.gap"
      padding (token)"space.pad"
    }
  }
  document id="doc.pg" {
    page id="page.pg" w=(px)200 h=(px)200 {
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    let style = &doc.styles.styles[0];
    assert_eq!(
        style.properties.get("padding"),
        Some(&PropertyValue::TokenRef("space.pad".to_owned())),
        "padding must be a TokenRef to space.pad"
    );
    assert_eq!(
        style.properties.get("gap"),
        Some(&PropertyValue::TokenRef("space.gap".to_owned())),
        "gap must be a TokenRef to space.gap"
    );
    assert!(
        style.unknown_props.is_empty(),
        "padding/gap must be recognized, not captured as unknown props"
    );

    // Round-trip: parse → format → parse preserves both props.
    let formatted = format_document(&doc).expect("format");
    let reparsed = adapter.parse(&formatted).expect("re-parse after format");
    let style2 = &reparsed.styles.styles[0];
    assert_eq!(
        style2.properties.get("padding"),
        Some(&PropertyValue::TokenRef("space.pad".to_owned())),
        "padding must survive round-trip"
    );
    assert_eq!(
        style2.properties.get("gap"),
        Some(&PropertyValue::TokenRef("space.gap".to_owned())),
        "gap must survive round-trip"
    );
}

/// Unknown style child names are captured in `unknown_props`.
#[test]
fn style_unknown_child_captured() {
    let src = r##"zenith version=1 {
  project id="proj.unk" name="UNK"
  tokens format="zenith-token-v1" {
  }
  styles {
    style id="style.unk" {
      bogus "some-value"
    }
  }
  document id="doc.unk" {
    page id="page.unk" w=(px)100 h=(px)100 {
    }
  }
}
"##;
    let adapter = KdlAdapter;
    let doc = adapter.parse(src.as_bytes()).expect("parse");

    let style = &doc.styles.styles[0];
    assert!(style.properties.is_empty(), "no recognized props expected");
    assert!(
        style.unknown_props.contains_key("bogus"),
        "unknown prop 'bogus' must be captured in unknown_props"
    );
}

/// Parse → format → parse round-trips correctly (spans stripped for equality).
#[test]
fn styles_round_trip() {
    let adapter = KdlAdapter;
    let doc_orig = adapter
        .parse(WITH_STYLES.as_bytes())
        .expect("original parse");
    let formatted = format_document(&doc_orig).expect("format");
    let doc_reparsed = adapter.parse(&formatted).expect("re-parse after format");

    let orig_stripped = strip_spans(doc_orig);
    let reparsed_stripped = strip_spans(doc_reparsed);
    assert_eq!(
        orig_stripped.styles, reparsed_stripped.styles,
        "styles must survive round-trip (spans excluded)"
    );
}

/// Format twice → identical bytes (idempotency).
#[test]
fn styles_format_idempotent() {
    let adapter = KdlAdapter;
    let doc = adapter.parse(WITH_STYLES.as_bytes()).expect("parse");
    let s1 = format_document(&doc).expect("format 1");
    let doc2 = adapter.parse(&s1).expect("parse after first format");
    let s2 = format_document(&doc2).expect("format 2");
    assert_eq!(
        String::from_utf8(s1).unwrap(),
        String::from_utf8(s2).unwrap(),
        "styles format must be idempotent"
    );
}
