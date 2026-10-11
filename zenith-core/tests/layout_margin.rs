//! Integration tests: per-child `margin*` item attributes — parse, canonical
//! format, schema, and validation (dimension form, negatives, inert sites).

use zenith_core::format::format_document;
use zenith_core::schema::{attribute_default, attribute_type_for_kind, node_attributes};
use zenith_core::{
    Document, FixHint, KdlAdapter, KdlSource, Node, PropertyValue, Severity, ValidationReport,
    validate,
};

const MARGINS: [&str; 7] = [
    "margin",
    "margin-x",
    "margin-y",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
];

fn doc_src(children: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.lm" name="LM"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
    token id="space.s" type="dimension" value=(px)8
    token id="space.neg" type="dimension" value=(px)-8
  }}
  styles {{
  }}
  document id="doc.lm" title="LM" {{
    page id="p" w=(px)800 h=(px)800 {{
{children}
    }}
  }}
}}
"##
    )
}

fn parse(src: &str) -> Document {
    KdlAdapter.parse(src.as_bytes()).expect("parse")
}

fn format(doc: &Document) -> String {
    String::from_utf8(format_document(doc).expect("format")).expect("utf8")
}

fn check(children: &str) -> ValidationReport {
    validate(&parse(&doc_src(children)))
}

fn in_column(attrs: &str) -> String {
    format!(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" {{
        rect id="r" h=(px)20 {attrs} fill=(token)"color.k"
      }}"#
    )
}

fn codes(report: &ValidationReport) -> Vec<&str> {
    report.diagnostics.iter().map(|d| d.code.as_str()).collect()
}

fn errors(report: &ValidationReport) -> Vec<String> {
    report
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| format!("{}: {}", d.code, d.message))
        .collect()
}

/// The `layout.inert_attribute` messages and their removal fixes.
fn inert(report: &ValidationReport) -> Vec<(String, Option<FixHint>)> {
    report
        .diagnostics
        .iter()
        .filter(|d| d.code == "layout.inert_attribute")
        .map(|d| (d.message.clone(), d.fix().cloned()))
        .collect()
}

#[test]
fn parse_every_margin_attribute() {
    let doc = parse(&doc_src(&in_column(
        r#"margin=(px)1 margin-x=(px)2 margin-y=(token)"space.s" margin-top=(px)4 margin-right=(px)5 margin-bottom=(px)6 margin-left=(px)7"#,
    )));
    let Node::Frame(f) = &doc.body.pages[0].children[0] else {
        panic!("expected frame");
    };
    let Node::Rect(r) = &f.children[0] else {
        panic!("expected rect");
    };
    let li = &r.layout_item;
    assert!(r.unknown_props.is_empty(), "{:?}", r.unknown_props);
    assert_eq!(
        li.margin_y,
        Some(PropertyValue::TokenRef("space.s".to_owned()))
    );
    for (name, value) in li.margins() {
        assert!(value.is_some(), "{name}");
    }
}

#[test]
fn format_round_trip_is_canonical_and_idempotent() {
    let src = doc_src(&in_column(
        r#"margin-left=(px)7 margin-bottom=(px)6 margin=(px)1 position="auto" margin-top=(px)40 margin-y=(px)3 margin-right=(px)5 margin-x=(px)2 min-w=(px)10"#,
    ));
    let once = format(&parse(&src));
    let twice = format(&parse(&once));
    assert_eq!(once, twice, "format must be idempotent");
    assert!(
        once.contains(
            r#"min-w=(px)10 position="auto" margin=(px)1 margin-x=(px)2 margin-y=(px)3 margin-top=(px)40 margin-right=(px)5 margin-bottom=(px)6 margin-left=(px)7"#
        ),
        "canonical item order; got:\n{once}"
    );
}

#[test]
fn schema_lists_margins_on_box_kinds() {
    for kind in ["rect", "text", "frame", "instance", "chart", "mesh"] {
        let attrs = node_attributes(kind);
        for name in MARGINS {
            assert!(attrs.contains(&name), "{kind} lists {name}");
            assert_eq!(
                attribute_type_for_kind(kind, name),
                "px literal or token ref: dimension (>= 0)"
            );
        }
    }
    assert_eq!(attribute_default("rect", "margin"), Some("(px)0"));
    assert_eq!(attribute_default("rect", "margin-top"), Some("margin-y"));
    assert!(!node_attributes("line").contains(&"margin-top"));
}

#[test]
fn margins_in_a_column_validate_clean() {
    for attrs in [
        r#"margin=(px)4"#,
        r#"margin-top=(px)40"#,
        r#"margin-x=(token)"space.s""#,
        r#"margin-bottom=(pt)6"#,
        r#"margin-left=(px)0"#,
    ] {
        let report = check(&in_column(attrs));
        assert!(errors(&report).is_empty(), "{attrs}: {:?}", errors(&report));
        assert!(inert(&report).is_empty(), "{attrs}: {:?}", inert(&report));
    }
}

#[test]
fn negative_margin_is_invalid_value() {
    for attrs in [r#"margin-top=(px)-4"#, r#"margin=(token)"space.neg""#] {
        let report = check(&in_column(attrs));
        let hits: Vec<_> = report
            .diagnostics
            .iter()
            .filter(|d| d.code == "node.invalid_value")
            .collect();
        assert_eq!(hits.len(), 1, "{attrs}: {:?}", report.diagnostics);
        assert_eq!(hits[0].severity, Severity::Error);
        assert!(
            hits[0].message.contains("must be >= 0"),
            "{}",
            hits[0].message
        );
    }
}

#[test]
fn malformed_margin_is_rejected_like_padding() {
    let bad_unit = check(&in_column(r#"margin=(em)4"#));
    assert!(
        codes(&bad_unit).contains(&"node.invalid_geometry"),
        "{:?}",
        bad_unit.diagnostics
    );
    let bare = check(&in_column(r#"margin-top="40""#));
    assert!(
        codes(&bare).contains(&"node.invalid_geometry"),
        "{:?}",
        bare.diagnostics
    );
    let wrong_token = check(&in_column(r#"margin-left=(token)"color.k""#));
    assert!(
        !errors(&wrong_token).is_empty(),
        "{:?}",
        wrong_token.diagnostics
    );
    let missing_token = check(&in_column(r#"margin-left=(token)"space.none""#));
    assert!(
        !errors(&missing_token).is_empty(),
        "{:?}",
        missing_token.diagnostics
    );
}

#[test]
fn margin_outside_a_stack_is_inert() {
    for frame in [
        r#"frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200"#,
        r#"frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="grid" columns=2"#,
    ] {
        let report = check(&format!(
            r#"      {frame} {{
        rect id="r" x=(px)0 y=(px)0 w=(px)20 h=(px)20 margin-top=(px)4 fill=(token)"color.k"
      }}"#
        ));
        let found = inert(&report);
        assert_eq!(found.len(), 1, "{frame}: {:?}", report.diagnostics);
        assert!(
            found[0]
                .0
                .contains("margin-top has no effect outside a row/column frame"),
            "{}",
            found[0].0
        );
        assert_eq!(
            found[0].1,
            Some(FixHint::RemoveProperty {
                property: "margin-top".to_owned()
            })
        );
    }
}

#[test]
fn margin_on_absolute_child_is_inert() {
    let report = check(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="row" {
        rect id="r" x=(px)0 y=(px)0 w=(px)20 h=(px)20 position="absolute" margin=(px)4 margin-left=(px)2 fill=(token)"color.k"
      }"#,
    );
    let found = inert(&report);
    assert_eq!(found.len(), 2, "{:?}", report.diagnostics);
    assert!(
        found[0]
            .0
            .contains("margin has no effect on a position=\"absolute\" child"),
        "{}",
        found[0].0
    );
    assert!(found[1].0.contains("margin-left"), "{}", found[1].0);
}
