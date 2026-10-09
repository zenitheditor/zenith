//! `set_node_token` (radius and font tokens on a node) and `set_span_text`
//! (one span's text, its attributes kept).

use zenith_core::{KdlAdapter, KdlSource, Node, PropertyValue, patch_source};
use zenith_tx::{Op, Transaction, TxStatus, run_transaction};

const DOC: &str = r##"zenith version=1 {
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#111111"
    token id="color.accent" type="color" value="#2563eb"
    token id="font.sans" type="fontFamily" value="Noto Sans"
    token id="size.small" type="dimension" value=(px)12
    token id="size.big" type="dimension" value=(px)24
    token id="weight.bold" type="fontWeight" value=700
  }
  styles {}
  document id="d" {
    page id="p" w=(px)200 h=(px)200 {
      // A card.
      rect id="card" x=(px)0 y=(px)0 w=(px)100 h=(px)50 fill=(token)"color.ink" radius=(token)"size.small"
      text id="title" x=(px)0 y=(px)60 w=(px)180 h=(px)40 fill=(token)"color.ink" font-family=(token)"font.sans" font-size=(token)"size.small" {
        span "Hello "
        span "world" fill=(token)"color.accent" font-weight=(token)"weight.bold"
      }
      ellipse id="dot" x=(px)0 y=(px)120 w=(px)20 h=(px)20 fill=(token)"color.ink"
    }
  }
}
"##;

fn run(ops: Vec<Op>) -> zenith_tx::TxResult {
    let doc = KdlAdapter.parse(DOC.as_bytes()).expect("parse");
    run_transaction(
        &doc,
        &Transaction {
            ops,
            permissions: Default::default(),
        },
    )
    .expect("run")
}

fn token(node: &str, property: &str, token: Option<&str>) -> Op {
    Op::SetNodeToken {
        node: node.to_owned(),
        property: property.to_owned(),
        token: token.map(str::to_owned),
    }
}

fn find<'d>(doc: &'d zenith_core::Document, id: &str) -> &'d Node {
    doc.body
        .pages
        .iter()
        .flat_map(|p| &p.children)
        .find(|n| n.id() == Some(id))
        .expect("node")
}

#[test]
fn binds_and_removes_radius_and_font_tokens() {
    let result = run(vec![
        token("card", "radius", Some("size.big")),
        token("title", "font_size", Some("size.big")),
        token("title", "font-weight", Some("weight.bold")),
    ]);
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    let after = &result.document_after;
    let Node::Rect(card) = find(after, "card") else {
        panic!("rect");
    };
    assert_eq!(
        card.radius,
        Some(PropertyValue::TokenRef("size.big".into()))
    );
    let Node::Text(title) = find(after, "title") else {
        panic!("text");
    };
    assert_eq!(
        title.font_size,
        Some(PropertyValue::TokenRef("size.big".into()))
    );
    assert_eq!(
        title.font_weight,
        Some(PropertyValue::TokenRef("weight.bold".into()))
    );
    let removed = run(vec![token("card", "radius", None)]);
    assert_eq!(removed.status, TxStatus::Accepted);
    let Node::Rect(card) = find(&removed.document_after, "card") else {
        panic!("rect");
    };
    assert_eq!(card.radius, None);
}

#[test]
fn rejects_unknown_properties_kinds_and_token_types() {
    let wrong_property = run(vec![token("card", "fill", Some("color.ink"))]);
    assert_eq!(wrong_property.status, TxStatus::Rejected);
    assert!(
        wrong_property
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property")
    );
    let wrong_kind = run(vec![token("dot", "radius", Some("size.big"))]);
    assert_eq!(wrong_kind.status, TxStatus::Rejected);
    assert!(
        wrong_kind
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.unsupported_property"
                && d.message.contains("radius is not supported on a ellipse"))
    );
    // A color token as a font size fails the post-apply validation.
    let wrong_type = run(vec![token("title", "font-size", Some("color.ink"))]);
    assert_eq!(wrong_type.status, TxStatus::Rejected);
}

#[test]
fn span_text_keeps_the_span_attributes() {
    let result = run(vec![Op::SetSpanText {
        node: "title".into(),
        span: 1,
        text: "there".into(),
    }]);
    assert_eq!(
        result.status,
        TxStatus::Accepted,
        "{:?}",
        result.diagnostics
    );
    let Node::Text(title) = find(&result.document_after, "title") else {
        panic!("text");
    };
    let span = title.spans.get(1).expect("span 1");
    assert_eq!(span.text, "there");
    assert_eq!(
        span.fill,
        Some(PropertyValue::TokenRef("color.accent".into()))
    );
    assert_eq!(
        span.font_weight,
        Some(PropertyValue::TokenRef("weight.bold".into()))
    );
    let out_of_range = run(vec![Op::SetSpanText {
        node: "title".into(),
        span: 5,
        text: "x".into(),
    }]);
    assert_eq!(out_of_range.status, TxStatus::Rejected);
    assert!(
        out_of_range
            .diagnostics
            .iter()
            .any(|d| d.code == "tx.invalid_value" && d.message.contains("2 span(s)"))
    );
    let not_text = run(vec![Op::SetSpanText {
        node: "card".into(),
        span: 0,
        text: "x".into(),
    }]);
    assert_eq!(not_text.status, TxStatus::Rejected);
}

#[test]
fn both_ops_patch_in_place_and_keep_comments() {
    let before = KdlAdapter.parse(DOC.as_bytes()).expect("parse");
    let result = run(vec![
        token("card", "radius", Some("size.big")),
        Op::SetSpanText {
            node: "title".into(),
            span: 0,
            text: "Hi ".into(),
        },
    ]);
    assert_eq!(result.status, TxStatus::Accepted);
    let patched = patch_source(DOC, &before, &result.document_after).expect("patch");
    assert!(!patched.reformatted, "fell back to canonical text");
    assert!(patched.text.contains("// A card."));
    assert!(patched.text.contains(r#"radius=(token)"size.big""#));
    assert!(patched.text.contains(r#"span "Hi ""#));
    assert!(
        patched
            .text
            .contains(r#"span "world" fill=(token)"color.accent""#)
    );
    let changed: Vec<(&str, &str)> = DOC
        .lines()
        .zip(patched.text.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(changed.len(), 2, "{changed:#?}");
}
