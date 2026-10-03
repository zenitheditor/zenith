//! Integration tests: auto-layout attributes (`frame` container attributes and
//! per-child item attributes) — parse, canonical format, schema, validation.

use zenith_core::format::format_document;
use zenith_core::schema::{attribute_default, attribute_type_for_kind, node_attributes};
use zenith_core::{
    Document, FixHint, KdlAdapter, KdlSource, LayoutAlign, LayoutJustify, LayoutKind,
    LayoutPosition, Node, PropertyValue, Severity, SizeKeyword, ValidationReport, validate,
};

/// Wrap page children in a minimal document with the tokens the fixtures use.
fn doc_src(children: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.al" name="AL"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
    token id="space.s" type="dimension" value=(px)8
  }}
  styles {{
  }}
  document id="doc.al" title="AL" {{
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

fn diags<'a>(report: &'a ValidationReport, code: &str) -> Vec<&'a zenith_core::Diagnostic> {
    report
        .diagnostics
        .iter()
        .filter(|d| d.code == code)
        .collect()
}

fn has(report: &ValidationReport, code: &str) -> bool {
    !diags(report, code).is_empty()
}

fn errors(report: &ValidationReport) -> Vec<String> {
    report
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| format!("{}: {}", d.code, d.message))
        .collect()
}

fn first_frame(doc: &Document) -> &zenith_core::FrameNode {
    match &doc.body.pages[0].children[0] {
        Node::Frame(f) => f,
        other => panic!("expected frame, got {other:?}"),
    }
}

const RECT: &str = r#"rect id="r" h=(px)20 fill=(token)"color.k""#;

// ── Parse ──────────────────────────────────────────────────────────────────

#[test]
fn parse_frame_container_attributes() {
    let doc = parse(&doc_src(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" gap=(px)4 wrap-gap=(token)"space.s" padding=(px)1 padding-x=(px)2 padding-y=(px)3 padding-top=(px)4 padding-right=(px)5 padding-bottom=(px)6 padding-left=(px)7 justify="space-between" align="center" wrap=#true clip=#false fill=(token)"color.k" stroke=(token)"color.k" stroke-width=(token)"space.s" radius=(token)"space.s" {
        rect id="r" h=(px)20
      }"#,
    ));
    let f = first_frame(&doc);
    assert_eq!(f.layout, Some(LayoutKind::Column));
    let c = &f.container;
    assert!(matches!(c.gap, Some(PropertyValue::Dimension(_))));
    assert_eq!(
        c.wrap_gap,
        Some(PropertyValue::TokenRef("space.s".to_owned()))
    );
    for pv in [
        &c.padding,
        &c.padding_x,
        &c.padding_y,
        &c.padding_top,
        &c.padding_right,
        &c.padding_bottom,
        &c.padding_left,
    ] {
        assert!(matches!(pv, Some(PropertyValue::Dimension(_))), "{pv:?}");
    }
    assert_eq!(c.justify, Some(LayoutJustify::SpaceBetween));
    assert_eq!(c.align, Some(LayoutAlign::Center));
    assert_eq!(c.wrap, Some(true));
    assert_eq!(f.clip, Some(false));
    assert!(f.fill.is_some() && f.stroke.is_some());
    assert!(f.stroke_width.is_some() && f.radius.is_some());
    assert!(f.unknown_props.is_empty(), "{:?}", f.unknown_props);
}

#[test]
fn parse_item_attributes_and_size_keywords() {
    let doc = parse(&doc_src(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" {
        rect id="r" w="fill" h="hug" min-w=(px)10 max-w=(token)"space.s" min-h=(px)1 max-h=(px)99 position="absolute"
      }"#,
    ));
    let Node::Rect(r) = &first_frame(&doc).children[0] else {
        panic!("expected rect");
    };
    assert_eq!(r.w, None, "keyword w leaves the dimension unset");
    assert_eq!(r.h, None);
    let item = &r.layout_item;
    assert_eq!(item.w_keyword, Some(SizeKeyword::Fill));
    assert_eq!(item.h_keyword, Some(SizeKeyword::Hug));
    assert!(item.min_w.is_some() && item.max_w.is_some());
    assert!(item.min_h.is_some() && item.max_h.is_some());
    assert_eq!(item.position, Some(LayoutPosition::Absolute));
    assert!(r.unknown_props.is_empty(), "{:?}", r.unknown_props);
}

#[test]
fn parse_keeps_unknown_enum_values() {
    let doc = parse(&doc_src(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="flow" justify="around" align="baseline" {
        rect id="r" x=(px)0 y=(px)0 w=(px)1 h=(px)1 position="sticky"
      }"#,
    ));
    let f = first_frame(&doc);
    assert_eq!(f.layout, Some(LayoutKind::Unknown("flow".to_owned())));
    assert_eq!(
        f.container.justify,
        Some(LayoutJustify::Unknown("around".to_owned()))
    );
    assert_eq!(
        f.container.align,
        Some(LayoutAlign::Unknown("baseline".to_owned()))
    );
    let Node::Rect(r) = &f.children[0] else {
        panic!("expected rect");
    };
    assert_eq!(
        r.layout_item.position,
        Some(LayoutPosition::Unknown("sticky".to_owned()))
    );
}

#[test]
fn parse_non_keyword_string_size_stays_a_literal() {
    let doc = parse(&doc_src(
        r#"      rect id="r" x=(px)0 y=(px)0 w="huge" h=(px)1"#,
    ));
    let Node::Rect(r) = &doc.body.pages[0].children[0] else {
        panic!("expected rect");
    };
    assert_eq!(r.w, Some(PropertyValue::Literal("huge".to_owned())));
    assert_eq!(r.layout_item.w_keyword, None);
}

// ── Format ─────────────────────────────────────────────────────────────────

#[test]
fn format_round_trip_is_idempotent_and_canonical() {
    let src = doc_src(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 radius=(token)"space.s" clip=#true wrap=#false align="stretch" justify="start" padding-left=(px)7 padding=(px)1 gap=(px)4 layout="column" {
        rect id="r" position="auto" max-h=(px)99 min-w=(px)10 h="hug" w="fill"
        text id="t" w="hug" {
          span "x"
        }
        image id="i" asset="a" w=(px)10 h="fill"
      }"#,
    );
    let once = format(&parse(&src));
    let twice = format(&parse(&once));
    assert_eq!(once, twice, "format must be idempotent");
    assert!(
        once.contains(
            r#"frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" gap=(px)4 padding=(px)1 padding-left=(px)7 justify="start" align="stretch" wrap=#false clip=#true radius=(token)"space.s" {"#
        ),
        "canonical frame order; got:\n{once}"
    );
    assert!(
        once.contains(r#"rect id="r" w="fill" h="hug" min-w=(px)10 max-h=(px)99 position="auto""#),
        "canonical item order; got:\n{once}"
    );
    assert!(once.contains(r#"text id="t" w="hug""#), "{once}");
    assert!(once.contains(r#"w=(px)10 h="fill""#), "{once}");
    assert_eq!(parse(&once).body.pages, parse(&twice).body.pages);
}

#[test]
fn format_unknown_enum_values_round_trip() {
    let src = doc_src(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="flow" justify="around" {
        rect id="r" x=(px)0 y=(px)0 w=(px)1 h=(px)1 position="sticky"
      }"#,
    );
    let once = format(&parse(&src));
    assert!(once.contains(r#"layout="flow" justify="around""#), "{once}");
    assert!(once.contains(r#"position="sticky""#), "{once}");
    assert_eq!(once, format(&parse(&once)));
}

#[test]
fn format_without_layout_attrs_is_unchanged() {
    let src = doc_src(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 {
        rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k"
      }"#,
    );
    let once = format(&parse(&src));
    assert!(
        once.contains(r#"frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 {"#),
        "{once}"
    );
    assert!(
        once.contains(r#"rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k""#),
        "{once}"
    );
}

// ── Schema ─────────────────────────────────────────────────────────────────

#[test]
fn schema_lists_frame_container_attributes_with_types() {
    let attrs = node_attributes("frame");
    for name in [
        "layout",
        "gap",
        "wrap-gap",
        "padding",
        "padding-x",
        "padding-y",
        "padding-top",
        "padding-right",
        "padding-bottom",
        "padding-left",
        "justify",
        "align",
        "wrap",
        "clip",
        "fill",
        "stroke",
        "stroke-width",
        "radius",
        "min-w",
        "max-w",
        "min-h",
        "max-h",
        "position",
    ] {
        assert!(attrs.contains(&name), "frame lists {name}: {attrs:?}");
        assert_ne!(
            attribute_type_for_kind("frame", name),
            "string",
            "frame.{name}"
        );
    }
    assert_eq!(
        attribute_type_for_kind("frame", "layout"),
        "enum: absolute|row|column|grid"
    );
    assert_eq!(
        attribute_type_for_kind("frame", "justify"),
        "enum: start|center|end|space-between"
    );
    assert_eq!(
        attribute_type_for_kind("frame", "align"),
        "enum: start|center|end|stretch"
    );
    assert_eq!(
        attribute_type_for_kind("frame", "gap"),
        "px literal or token ref: dimension"
    );
    assert_eq!(
        attribute_type_for_kind("frame", "fill"),
        "token ref: color/gradient"
    );
    assert_eq!(attribute_default("frame", "layout"), Some("absolute"));
    assert_eq!(attribute_default("frame", "align"), Some("stretch"));
    assert_eq!(attribute_default("frame", "justify"), Some("start"));
    assert_eq!(attribute_default("frame", "wrap"), Some("#false"));
}

#[test]
fn schema_lists_item_attributes_on_box_kinds() {
    for kind in ["rect", "text", "image", "group", "table", "shape", "chart"] {
        let attrs = node_attributes(kind);
        for name in ["min-w", "max-w", "min-h", "max-h", "position"] {
            assert!(attrs.contains(&name), "{kind} lists {name}");
        }
        assert_eq!(
            attribute_type_for_kind(kind, "w"),
            "px literal, token ref: dimension, or enum: hug|fill"
        );
        assert_eq!(
            attribute_type_for_kind(kind, "position"),
            "enum: auto|absolute"
        );
    }
    assert!(!node_attributes("line").contains(&"position"));
    assert_eq!(
        attribute_type_for_kind("shape", "padding"),
        "px literal or token ref: dimension"
    );
}

// ── Validation: accepted documents ─────────────────────────────────────────

#[test]
fn column_children_need_no_geometry() {
    let report = check(&format!(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" gap=(px)4 padding=(px)8 clip=#true {{
        {RECT}
        text id="t" {{
          span "x"
        }}
        frame id="inner" h=(px)40 layout="column" {{
          {RECT2}
        }}
      }}"#,
        RECT2 = r#"rect id="r2" h=(px)10 fill=(token)"color.k""#
    ));
    assert!(errors(&report).is_empty(), "{:?}", errors(&report));
    assert!(
        !has(&report, "token.raw_visual_literal"),
        "gap/padding px are geometry"
    );
    assert!(!has(&report, "layout.inert_attribute"));
}

#[test]
fn layout_frame_children_skip_child_overflow() {
    let report = check(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)100 h=(px)100 layout="grid" columns=1 {
        rect id="r" x=(px)500 y=(px)500 w=(px)10 h=(px)10 fill=(token)"color.k"
      }"#,
    );
    assert!(!has(&report, "frame.child_overflow"));
}

#[test]
fn shape_padding_accepts_px_literal() {
    let report = check(
        r#"      shape id="s" x=(px)0 y=(px)0 w=(px)100 h=(px)50 padding=(px)6 fill=(token)"color.k""#,
    );
    assert!(
        !has(&report, "token.raw_visual_literal"),
        "{:?}",
        report.diagnostics
    );
    assert!(errors(&report).is_empty(), "{:?}", errors(&report));
}

// ── Validation: diagnostics ────────────────────────────────────────────────

#[test]
fn removed_flow_value_is_invalid() {
    let report = check(&format!(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="flow" {{
        {RECT}
      }}"#
    ));
    let d = diags(&report, "node.invalid_value");
    assert_eq!(d.len(), 1, "{:?}", report.diagnostics);
    assert!(
        d[0].message.contains("absolute, row, column, grid"),
        "{}",
        d[0].message
    );
}

#[test]
fn invalid_enum_values_carry_a_fix() {
    let report = check(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="colum" justify="centre" align="strech" {
        rect id="r" h=(px)20 position="absolut" fill=(token)"color.k"
      }"#,
    );
    let fixes: Vec<_> = diags(&report, "node.invalid_value")
        .iter()
        .filter_map(|d| d.fix().cloned())
        .collect();
    for (property, to) in [
        ("layout", "column"),
        ("justify", "center"),
        ("align", "stretch"),
        ("position", "absolute"),
    ] {
        assert!(
            fixes.iter().any(|f| matches!(f, FixHint::ReplaceValue { property: p, to: t, .. } if p == property && t == to)),
            "{property} → {to}; fixes: {fixes:?}"
        );
    }
}

#[test]
fn container_features_validate_clean() {
    for attrs in [
        r#"layout="row""#,
        r#"layout="column" wrap=#true"#,
        r#"layout="column" justify="center""#,
        r#"layout="row" justify="space-between" align="end""#,
    ] {
        let report = check(&format!(
            r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 {attrs} {{
        rect id="r" h=(px)20 w=(px)10 fill=(token)"color.k"
      }}"#
        ));
        assert!(errors(&report).is_empty(), "{attrs}: {:?}", errors(&report));
    }
}

fn instance_doc(instance: &str) -> String {
    format!(
        r##"zenith version=1 {{
  project id="proj.al" name="AL"
  tokens format="zenith-token-v1" {{
    token id="color.k" type="color" value="#000000"
  }}
  styles {{
  }}
  components {{
    component id="comp.c" {{
      rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k"
    }}
  }}
  document id="doc.al" title="AL" {{
    page id="p" w=(px)800 h=(px)800 {{
      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="row" {{
        {instance}
      }}
    }}
  }}
}}
"##
    )
}

#[test]
fn instance_in_layout_frame_validates_clean() {
    for instance in [
        r#"instance id="i" component="comp.c""#,
        r#"instance id="i" component="comp.c" w="fill" min-w=(px)20 max-h=(px)40"#,
        r#"instance id="i" component="comp.c" w=(px)30 h="hug""#,
        r#"instance id="i" component="comp.c" x=(px)5 y=(px)5 position="absolute""#,
    ] {
        let report = validate(&parse(&instance_doc(instance)));
        assert!(
            errors(&report).is_empty(),
            "{instance}: {:?}",
            errors(&report)
        );
        assert!(
            !has(&report, "layout.position_ignored"),
            "{instance}: {:?}",
            report.diagnostics
        );
    }
}

#[test]
fn instance_item_attributes_parse_and_format() {
    let doc = parse(&instance_doc(
        r#"instance id="i" component="comp.c" w="fill" h=(px)30 min-w=(px)20 position="auto""#,
    ));
    let Some(zenith_core::Node::Frame(f)) = doc.body.pages[0].children.first() else {
        panic!("frame");
    };
    let Some(zenith_core::Node::Instance(i)) = f.children.first() else {
        panic!("instance");
    };
    assert_eq!(
        i.layout_item.w_keyword,
        Some(zenith_core::SizeKeyword::Fill)
    );
    assert!(i.w.is_none());
    assert!(i.h.is_some());
    assert!(i.layout_item.min_w.is_some());
    let text = String::from_utf8(format_document(&doc).expect("format")).expect("utf8");
    assert!(
        text.contains(
            r#"instance id="i" component="comp.c" w="fill" h=(px)30 min-w=(px)20 position="auto""#
        ),
        "{text}"
    );
    let again = String::from_utf8(format_document(&parse(&text)).expect("format")).expect("utf8");
    assert_eq!(text, again);
}

#[test]
fn instance_position_ignored_in_flow() {
    let report = validate(&parse(&instance_doc(
        r#"instance id="i" component="comp.c" x=(px)5 y=(px)5"#,
    )));
    assert!(
        has(&report, "layout.position_ignored"),
        "{:?}",
        report.diagnostics
    );
}

#[test]
fn frame_paint_attributes_are_supported() {
    for attrs in [
        r#"fill=(token)"color.k""#,
        r#"layout="column" fill=(token)"color.k""#,
        r#"stroke=(token)"color.k" stroke-width=(token)"space.s""#,
        r#"radius=(token)"space.s""#,
    ] {
        let report = check(&format!(
            r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 {attrs} {{
        rect id="r" h=(px)20 x=(px)0 y=(px)0 w=(px)10 fill=(token)"color.k"
      }}"#
        ));
        assert!(errors(&report).is_empty(), "{attrs}: {:?}", errors(&report));
    }
}

#[test]
fn defaults_spelled_out_are_supported() {
    let report = check(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" justify="start" align="stretch" wrap=#false {
        rect id="r" h=(px)20 position="auto" fill=(token)"color.k"
      }"#,
    );
    assert!(errors(&report).is_empty(), "{:?}", errors(&report));
}

#[test]
fn item_features_validate_clean() {
    for attrs in [
        r#"w="fill""#,
        r#"h="hug""#,
        r#"min-w=(px)4"#,
        r#"max-h=(token)"space.s""#,
    ] {
        let report = check(&format!(
            r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" {{
        rect id="r" h=(px)20 {attrs} fill=(token)"color.k"
      }}"#
        ));
        assert!(
            !has(&report, "layout.inert_attribute"),
            "{attrs}: {:?}",
            report.diagnostics
        );
    }
}

#[test]
fn layout_frame_without_size_hugs() {
    let report = check(&format!(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 layout="column" {{
        {RECT}
      }}"#
    ));
    assert!(
        !has(&report, "node.missing_geometry"),
        "{:?}",
        report.diagnostics
    );
    assert!(errors(&report).is_empty(), "{:?}", errors(&report));
}

#[test]
fn explicit_hug_on_layout_frame_is_live() {
    let report = check(&format!(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h="hug" layout="column" {{
        {RECT}
      }}"#
    ));
    assert!(errors(&report).is_empty(), "{:?}", errors(&report));
    assert!(!has(&report, "layout.inert_attribute"));
}

#[test]
fn position_absolute_needs_placement() {
    let report = check(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" {
        rect id="r" w=(px)10 h=(px)20 position="absolute" fill=(token)"color.k"
      }"#,
    );
    assert!(
        has(&report, "layout.absolute_unplaced"),
        "{:?}",
        report.diagnostics
    );

    let placed = check(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" {
        rect id="r" x=(px)5 y=(px)5 w=(px)10 h=(px)20 position="absolute" fill=(token)"color.k"
      }"#,
    );
    assert!(!has(&placed, "layout.absolute_unplaced"));
    assert!(!has(&placed, "layout.position_ignored"));
    assert!(errors(&placed).is_empty(), "{:?}", errors(&placed));
}

#[test]
fn layout_pages_leave_geometry_checks_to_the_scene() {
    // An in-flow child with authored (ignored) x past the page edge: validation
    // reports no off_canvas, because the scene checks the laid-out geometry.
    let report = check(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" {
        rect id="r" x=(px)5000 h=(px)20 fill=(token)"color.k"
      }
      rect id="far" x=(px)5000 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k""#,
    );
    assert!(
        !has(&report, "layout.off_canvas"),
        "{:?}",
        report.diagnostics
    );

    // The geometry pass reports the page-level node on that page.
    let doc = parse(&doc_src(
        r#"      rect id="far" x=(px)5000 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k"
      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" {
        rect id="r" h=(px)20 fill=(token)"color.k"
      }"#,
    ));
    let pages = zenith_core::layout_geometry_checks(&doc);
    assert_eq!(pages.len(), 1);
    assert!(
        pages[0]
            .iter()
            .any(|d| d.code == "layout.off_canvas" && d.subject_id.as_deref() == Some("far")),
        "{:?}",
        pages[0]
    );

    // A page without a layout frame keeps its checks in validation.
    let plain = parse(&doc_src(
        r#"      rect id="far" x=(px)5000 y=(px)0 w=(px)10 h=(px)10 fill=(token)"color.k""#,
    ));
    assert!(
        validate(&plain)
            .diagnostics
            .iter()
            .any(|d| d.code == "layout.off_canvas")
    );
    assert!(zenith_core::layout_geometry_checks(&plain)[0].is_empty());
}

#[test]
fn xy_on_in_flow_child_is_ignored() {
    let report = check(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" {
        rect id="r" x=(px)5 h=(px)20 fill=(token)"color.k"
        rect id="r2" anchor="center" h=(px)20 fill=(token)"color.k"
      }"#,
    );
    let d = diags(&report, "layout.position_ignored");
    assert_eq!(d.len(), 2, "{:?}", report.diagnostics);
    assert_eq!(d[0].severity, Severity::Advisory);
    for (subject, attr) in [("r", "x"), ("r2", "anchor")] {
        assert!(
            d.iter().any(|d| d.subject_id.as_deref() == Some(subject)
                && d.fix()
                    == Some(&zenith_core::FixHint::RemoveProperty {
                        property: attr.to_owned()
                    })),
            "{subject} {attr}: {d:?}"
        );
    }
}

#[test]
fn each_ignored_position_attribute_gets_its_own_fix() {
    let report = check(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="row" {
        rect id="r" x=(px)5 y=(px)6 w=(px)20 h=(px)20 fill=(token)"color.k"
      }"#,
    );
    let props: Vec<String> = diags(&report, "layout.position_ignored")
        .iter()
        .filter_map(|d| {
            if let Some(zenith_core::FixHint::RemoveProperty { property }) = d.fix() {
                Some(property.clone())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(props, ["x", "y"], "{:?}", report.diagnostics);
}

#[test]
fn inert_attributes_are_advised() {
    // Container attributes on an absolute frame.
    let report = check(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 gap=(px)4 justify="start" {
        rect id="r" x=(px)0 y=(px)0 w=(px)10 h=(px)10 min-w=(px)2 fill=(token)"color.k"
      }"#,
    );
    // One advisory per attribute, each with its own removal fix.
    let d = diags(&report, "layout.inert_attribute");
    assert_eq!(d.len(), 3, "{:?}", report.diagnostics);
    for (subject, attr) in [("f", "gap"), ("f", "justify"), ("r", "min-w")] {
        assert!(
            d.iter().any(|d| d.subject_id.as_deref() == Some(subject)
                && d.message.contains(&format!("{attr} has no effect"))
                && d.fix()
                    == Some(&zenith_core::FixHint::RemoveProperty {
                        property: attr.to_owned()
                    })),
            "{subject} {attr}: {d:?}"
        );
    }

    // Stack-only attributes on a grid frame.
    let grid = check(
        r#"      frame id="g" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="grid" columns=2 gap=(px)4 align="start" {
        rect id="r" fill=(token)"color.k"
      }"#,
    );
    let d = diags(&grid, "layout.inert_attribute");
    assert_eq!(d.len(), 1, "{:?}", grid.diagnostics);
    assert!(d[0].message.contains("align") && !d[0].message.contains("gap"));
}

#[test]
fn conflicting_sizes_are_errors() {
    let min_max = check(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" {
        rect id="r" h=(px)20 min-w=(px)50 max-w=(px)10 fill=(token)"color.k"
      }"#,
    );
    assert!(
        has(&min_max, "layout.conflicting_size"),
        "{:?}",
        min_max.diagnostics
    );

    let wrap_hug = check(&format!(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 layout="column" wrap=#true {{
        {RECT}
      }}"#
    ));
    assert!(
        has(&wrap_hug, "layout.conflicting_size"),
        "{:?}",
        wrap_hug.diagnostics
    );

    let fit_text = check(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" {
        text id="t" overflow="fit" {
          span "x"
        }
      }"#,
    );
    assert!(
        has(&fit_text, "layout.conflicting_size"),
        "{:?}",
        fit_text.diagnostics
    );

    let fit_fixed = check(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" {
        text id="t" h=(px)40 overflow="fit" {
          span "x"
        }
      }"#,
    );
    assert!(
        !has(&fit_fixed, "layout.conflicting_size"),
        "{:?}",
        fit_fixed.diagnostics
    );
}

#[test]
fn layout_dimensions_reject_strings() {
    let report = check(&format!(
        r#"      frame id="f" x=(px)0 y=(px)0 w=(px)200 h=(px)200 layout="column" gap="wide" {{
        {RECT}
      }}"#
    ));
    assert!(
        has(&report, "node.invalid_geometry"),
        "{:?}",
        report.diagnostics
    );
}

#[test]
fn layout_codes_are_catalogued() {
    for (code, severity) in [
        ("layout.unsized_child", Severity::Error),
        ("layout.child_overflow", Severity::Advisory),
        ("layout.fill_in_hug_parent", Severity::Advisory),
        ("layout.conflicting_size", Severity::Error),
        ("layout.position_ignored", Severity::Advisory),
        ("layout.absolute_unplaced", Severity::Error),
        ("layout.inert_attribute", Severity::Advisory),
    ] {
        let info = zenith_core::diag_catalog::lookup(code).unwrap_or_else(|| panic!("{code}"));
        assert_eq!(info.severity, severity, "{code}");
    }
    assert!(zenith_core::diag_catalog::lookup("layout.not_yet_supported").is_none());
}
