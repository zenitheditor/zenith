//! Integration tests for the `defaults { … }` block (document and page scope),
//! the enum-valued style keys (`align`, `v-align`), the style `shadow` key, and
//! the `shape` `shadow` attribute: parse, canonical format, and validation.

use zenith_core::format::format_document;
use zenith_core::{
    DefaultsKind, Diagnostic, Document, FixHint, KdlAdapter, KdlSource, PropertyValue, Severity,
    validate,
};

const TOKENS: &str = r##"tokens format="zenith-token-v1" {
    token id="color.text" type="color" value="#111111"
    token id="size.body" type="dimension" value=(px)16
    token id="shadow.card" type="shadow" {
      layer dx=(px)0 dy=(px)4 blur=(px)8 color=(token)"color.text"
    }
  }"##;

/// A document with `styles_body` inside `styles`, `top` after `styles`, and
/// `page_body` inside the single page.
fn src(styles_body: &str, top: &str, page_body: &str) -> String {
    format!(
        r##"zenith version=1 {{
  {TOKENS}
  styles {{
    {styles_body}
  }}
  {top}
  document id="doc" {{
    page id="p1" w=(px)400 h=(px)300 {{
      {page_body}
    }}
  }}
}}
"##
    )
}

const STYLES: &str = r#"style id="body" { fill (token)"color.text" }
    style id="box" { fill (token)"color.text" }
    style id="box.label" { fill (token)"color.text" }"#;

fn parse(source: &str) -> Document {
    KdlAdapter.parse(source.as_bytes()).expect("parse")
}

fn fmt(doc: &Document) -> String {
    String::from_utf8(format_document(doc).expect("format")).expect("utf8")
}

fn diags(source: &str) -> Vec<Diagnostic> {
    validate(&parse(source)).diagnostics
}

fn with_code<'a>(all: &'a [Diagnostic], code: &str) -> Vec<&'a Diagnostic> {
    all.iter().filter(|d| d.code == code).collect()
}

// ── Parse + format ──────────────────────────────────────────────────────────

#[test]
fn document_defaults_round_trip_sorted_and_idempotent() {
    let s = src(
        STYLES,
        r#"defaults {
    text style="body"
    shape style="box" text-style="box.label"
    connector style="box"
  }"#,
        "",
    );
    let doc = parse(&s);
    assert_eq!(doc.defaults.entries.len(), 3);
    let shape = doc.defaults.get(DefaultsKind::Shape).expect("shape row");
    assert_eq!(shape.style, "box");
    assert_eq!(shape.text_style.as_deref(), Some("box.label"));

    let once = fmt(&doc);
    let expected = "  defaults {\n    connector style=\"box\"\n    shape style=\"box\" \
                    text-style=\"box.label\"\n    text style=\"body\"\n  }\n";
    assert!(once.contains(expected), "{once}");
    // Canonical position: right after `styles`.
    let styles_end = once.find("  styles {").expect("styles");
    let defaults_at = once.find("  defaults {").expect("defaults");
    let document_at = once.find("  document id=").expect("document");
    assert!(
        styles_end < defaults_at && defaults_at < document_at,
        "{once}"
    );

    let twice = fmt(&parse(&once));
    assert_eq!(once, twice);
    assert!(
        validate(&doc)
            .diagnostics
            .iter()
            .all(|d| !d.code.starts_with("defaults."))
    );
}

#[test]
fn page_defaults_round_trip_at_page_body_start() {
    let s = src(
        STYLES,
        "",
        r#"rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.text"
      defaults { text style="body"; rect style="box" }"#,
    );
    let doc = parse(&s);
    let page = &doc.body.pages[0];
    assert_eq!(page.defaults.entries.len(), 2);
    assert_eq!(page.children.len(), 1, "defaults is not a renderable child");

    let once = fmt(&doc);
    let expected = "page id=\"p1\" w=(px)400 h=(px)300 {\n      defaults {\n        \
                    rect style=\"box\"\n        text style=\"body\"\n      }\n";
    assert!(once.contains(expected), "{once}");
    assert_eq!(once, fmt(&parse(&once)));
}

#[test]
fn absent_defaults_writes_nothing() {
    let doc = parse(&src(STYLES, "", ""));
    assert!(doc.defaults.is_empty());
    assert!(!fmt(&doc).contains("defaults"));
}

#[test]
fn rejected_rows_round_trip_after_accepted_rows() {
    let s = src(
        STYLES,
        r#"defaults {
    txet style="body"
    text style="body"
    text style="box"
    mesh style="box"
  }"#,
        "",
    );
    let once = fmt(&parse(&s));
    let expected = "  defaults {\n    text style=\"body\"\n    txet style=\"body\"\n    \
                    text style=\"box\"\n    mesh style=\"box\"\n  }\n";
    assert!(once.contains(expected), "{once}");
    let reparsed = parse(&once);
    assert_eq!(
        reparsed
            .defaults
            .get(DefaultsKind::Text)
            .map(|e| e.style.as_str()),
        Some("body"),
        "the first row keeps winning after a format"
    );
    assert_eq!(once, fmt(&reparsed));
}

// ── Validation ──────────────────────────────────────────────────────────────

#[test]
fn unknown_kind_suggests_and_hints_unique_fix() {
    let all = diags(&src(STYLES, r#"defaults { txet style="body" }"#, ""));
    let found = with_code(&all, "defaults.unknown_kind");
    assert_eq!(found.len(), 1, "{all:?}");
    let d = found[0];
    assert_eq!(d.severity, Severity::Error);
    assert!(d.message.contains("did you mean 'text'?"), "{}", d.message);
    assert_eq!(
        d.fix(),
        Some(&FixHint::ReplaceValue {
            property: "kind".into(),
            from: "txet".into(),
            to: "text".into(),
        })
    );
    assert!(d.span.is_some());
}

#[test]
fn unknown_kind_without_near_match_lists_kinds() {
    let all = diags(&src(STYLES, r#"defaults { zzzzzz style="body" }"#, ""));
    let d = with_code(&all, "defaults.unknown_kind")[0];
    assert!(!d.message.contains("did you mean"), "{}", d.message);
    assert!(
        d.message.contains("use one of: chart, code"),
        "{}",
        d.message
    );
    assert!(d.fix().is_none());
}

#[test]
fn unsupported_kinds_are_errors() {
    for kind in ["instance", "light", "mesh"] {
        let all = diags(&src(
            STYLES,
            &format!("defaults {{ {kind} style=\"body\" }}"),
            "",
        ));
        let found = with_code(&all, "defaults.unsupported_kind");
        assert_eq!(found.len(), 1, "{kind}: {all:?}");
        assert!(found[0].message.contains(kind), "{}", found[0].message);
        assert!(with_code(&all, "defaults.unknown_kind").is_empty());
    }
}

#[test]
fn duplicate_kind_is_an_error() {
    let all = diags(&src(
        STYLES,
        r#"defaults { text style="body"; text style="box" }"#,
        "",
    ));
    let found = with_code(&all, "defaults.duplicate_kind");
    assert_eq!(found.len(), 1, "{all:?}");
    assert!(found[0].message.contains("'text'"), "{}", found[0].message);
}

#[test]
fn unknown_style_suggests_declared_id() {
    let all = diags(&src(STYLES, r#"defaults { text style="bodu" }"#, ""));
    let found = with_code(&all, "defaults.unknown_style");
    assert_eq!(found.len(), 1, "{all:?}");
    assert!(
        found[0].message.contains("did you mean 'body'?"),
        "{}",
        found[0].message
    );
    assert_eq!(
        found[0].fix(),
        Some(&FixHint::ReplaceValue {
            property: "style".into(),
            from: "bodu".into(),
            to: "body".into(),
        })
    );
}

#[test]
fn unknown_text_style_and_page_scope_subject() {
    let all = diags(&src(
        STYLES,
        "",
        r#"defaults { shape style="box" text-style="nope" }"#,
    ));
    let found = with_code(&all, "defaults.unknown_style");
    assert_eq!(found.len(), 1, "{all:?}");
    assert!(
        found[0].message.contains("page 'p1'"),
        "{}",
        found[0].message
    );
    assert!(
        found[0].message.contains("text-style=\"nope\""),
        "{}",
        found[0].message
    );
    assert_eq!(found[0].subject_id.as_deref(), Some("p1"));
}

#[test]
fn text_style_on_label_less_kind_is_an_error() {
    let all = diags(&src(
        STYLES,
        r#"defaults { rect style="box" text-style="box.label"; shape style="box" text-style="box.label" }"#,
        "",
    ));
    let found = with_code(&all, "defaults.text_style_unsupported");
    assert_eq!(found.len(), 1, "{all:?}");
    assert!(found[0].message.contains("'rect'"), "{}", found[0].message);
}

#[test]
fn unknown_row_attribute_is_an_error_with_rename() {
    let all = diags(&src(
        STYLES,
        r#"defaults { text style="body" stlye="x" }"#,
        "",
    ));
    let found = with_code(&all, "defaults.unknown_property");
    assert_eq!(found.len(), 1, "{all:?}");
    assert!(
        found[0].message.contains("did you mean 'style'?"),
        "{}",
        found[0].message
    );
}

#[test]
fn row_children_use_block_unknown_child() {
    let all = diags(&src(
        STYLES,
        r#"defaults { text style="body" { extra } }"#,
        "",
    ));
    let found = with_code(&all, "block.unknown_child");
    assert_eq!(found.len(), 1, "{all:?}");
    assert!(found[0].message.contains("'extra'"), "{}", found[0].message);
    let page = diags(&src(
        STYLES,
        "",
        r#"defaults { text style="body" { extra } }"#,
    ));
    assert_eq!(with_code(&page, "block.unknown_child").len(), 1, "{page:?}");
}

#[test]
fn defaults_codes_are_catalogued_errors() {
    for code in [
        "defaults.unknown_kind",
        "defaults.unsupported_kind",
        "defaults.unknown_style",
        "defaults.duplicate_kind",
        "defaults.text_style_unsupported",
        "defaults.unknown_property",
        "style.invalid_value",
    ] {
        let info = zenith_core::diag_catalog::lookup(code).unwrap_or_else(|| panic!("{code}"));
        assert_eq!(info.severity, Severity::Error, "{code}");
    }
}

// ── Style keys ──────────────────────────────────────────────────────────────

#[test]
fn style_enum_and_shadow_keys_round_trip() {
    let s = src(
        r#"style id="card" {
      align "center"
      v_align "middle"
      shadow (token)"shadow.card"
    }"#,
        "",
        "",
    );
    let doc = parse(&s);
    let style = &doc.styles.styles[0];
    assert_eq!(
        style.properties.get("align"),
        Some(&PropertyValue::Literal("center".into()))
    );
    assert_eq!(
        style.properties.get("v-align"),
        Some(&PropertyValue::Literal("middle".into()))
    );
    assert_eq!(
        style.properties.get("shadow"),
        Some(&PropertyValue::TokenRef("shadow.card".into()))
    );
    let once = fmt(&doc);
    assert!(
        once.contains(
            "align \"center\"\n      shadow (token)\"shadow.card\"\n      v-align \"middle\"\n"
        ),
        "{once}"
    );
    assert_eq!(once, fmt(&parse(&once)));
    let all = validate(&doc).diagnostics;
    assert!(
        all.iter().all(
            |d| !d.code.starts_with("style.") && !d.code.starts_with("token.")
                || d.code == "token.unused"
        ),
        "{all:?}"
    );
}

#[test]
fn style_enum_value_outside_list_is_invalid_value_with_fix() {
    let all = diags(&src(r#"style id="s" { align "centre" }"#, "", ""));
    let found = with_code(&all, "style.invalid_value");
    assert_eq!(found.len(), 1, "{all:?}");
    assert!(
        found[0].message.contains("did you mean 'center'?"),
        "{}",
        found[0].message
    );
    assert_eq!(
        found[0].fix(),
        Some(&FixHint::ReplaceValue {
            property: "align".into(),
            from: "centre".into(),
            to: "center".into(),
        })
    );
}

#[test]
fn style_enum_key_rejects_token_reference() {
    let all = diags(&src(
        r#"style id="s" { v-align (token)"size.body" }"#,
        "",
        "",
    ));
    let found = with_code(&all, "style.invalid_value");
    assert_eq!(found.len(), 1, "{all:?}");
    assert!(
        found[0].message.contains("plain enum value"),
        "{}",
        found[0].message
    );
}

#[test]
fn style_shadow_requires_a_shadow_token() {
    let all = diags(&src(
        r#"style id="s" { shadow (token)"size.body" }"#,
        "",
        "",
    ));
    assert!(
        all.iter().any(|d| d.code == "token.incompatible_property"),
        "{all:?}"
    );
}

// ── Shape shadow ────────────────────────────────────────────────────────────

#[test]
fn shape_shadow_round_trips_and_validates() {
    let s = src(
        "",
        "",
        r#"shape id="s1" x=(px)10 y=(px)10 w=(px)100 h=(px)60 fill=(token)"color.text" shadow=(token)"shadow.card" { span "Hi" }"#,
    );
    let doc = parse(&s);
    let once = fmt(&doc);
    assert!(once.contains(r#"shadow=(token)"shadow.card""#), "{once}");
    assert_eq!(once, fmt(&parse(&once)));
    let all = validate(&doc).diagnostics;
    assert!(
        !all.iter()
            .any(|d| d.code.starts_with("token.") && d.code != "token.unused"),
        "{all:?}"
    );

    let bad = diags(&src(
        "",
        "",
        r#"shape id="s1" x=(px)10 y=(px)10 w=(px)100 h=(px)60 shadow=(token)"color.text" { span "Hi" }"#,
    ));
    assert!(
        bad.iter().any(|d| d.code == "token.incompatible_property"),
        "{bad:?}"
    );
}

#[test]
fn justify_style_align_on_shape_or_table_warns() {
    let styles = r#"style id="j" { align "justify" }
    style id="c" { align "center" }"#;
    let shape = |attrs: &str| {
        format!(r#"shape id="s1" x=(px)10 y=(px)10 w=(px)100 h=(px)60 {attrs} {{ span "Hi" }}"#)
    };
    let all = diags(&src(styles, "", &shape(r#"style="j""#)));
    let found = with_code(&all, "style.align_unsupported");
    assert_eq!(found.len(), 1, "{all:?}");
    assert_eq!(found[0].severity, Severity::Warning);
    assert!(
        found[0].message.contains("shape 's1'"),
        "{}",
        found[0].message
    );
    assert!(found[0].message.contains("justify"), "{}", found[0].message);

    // An explicit h-align overrides the style, so nothing is ignored.
    let all = diags(&src(styles, "", &shape(r#"style="j" h-align="end""#)));
    assert!(with_code(&all, "style.align_unsupported").is_empty());
    // A label text-style align outranks the shape style align: no warning.
    let with_label = format!("{styles}\n    style id=\"lbl\" {{ align \"end\" }}");
    let all = diags(&src(
        &with_label,
        "",
        &shape(r#"style="j" text-style="lbl""#),
    ));
    assert!(with_code(&all, "style.align_unsupported").is_empty());
    // A start/center/end style value applies.
    let all = diags(&src(styles, "", &shape(r#"style="c""#)));
    assert!(with_code(&all, "style.align_unsupported").is_empty());

    let table = r#"table id="t1" x=(px)10 y=(px)10 w=(px)100 h=(px)60 style="j" {
        column width=(px)100
        row { cell { } }
      }"#;
    let all = diags(&src(styles, "", table));
    let found = with_code(&all, "style.align_unsupported");
    assert_eq!(found.len(), 1, "{all:?}");
    assert!(
        found[0].message.contains("table 't1'"),
        "{}",
        found[0].message
    );
    // Text takes justify: no warning.
    let all = diags(&src(
        styles,
        "",
        r#"text id="t" x=(px)0 y=(px)0 w=(px)100 h=(px)20 style="j" { span "x" }"#,
    ));
    assert!(with_code(&all, "style.align_unsupported").is_empty());
    assert!(zenith_core::diag_catalog::lookup("style.align_unsupported").is_some());
}
