use super::*;

// ── polygon: point with missing y → node.missing_geometry ─────────────

#[test]
fn polygon_point_missing_coord() {
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![Node::Polygon(PolygonNode {
                id: "poly.missy".to_owned(),
                name: None,
                role: None,
                fill: None,
                stroke: None,
                stroke_width: None,
                stroke_alignment: None,
                fill_rule: None,
                opacity: None,
                visible: None,
                locked: None,
                rotate: None,
                style: None,
                points: vec![
                    Point {
                        x: Some(px(0.0)),
                        y: None,
                    }, // missing y
                    Point {
                        x: Some(px(100.0)),
                        y: Some(px(0.0)),
                    },
                    Point {
                        x: Some(px(50.0)),
                        y: Some(px(100.0)),
                    },
                ],
                source_span: None,
                unknown_props: BTreeMap::new(),
            })],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "node.missing_geometry"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

// ── polygon: fill raw literal → token.raw_visual_literal ─────────────

#[test]
fn polygon_fill_raw_literal() {
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![minimal_polygon(
                "poly.lit",
                Some(PropertyValue::Literal("#ff0000".to_owned())),
            )],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "token.raw_visual_literal"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

// ── text: literal font-size dimension → token.raw_visual_literal ─────

/// A literal `font-size=(px)24` (a `PropertyValue::Dimension`, not a token)
/// must be treated as a raw visual literal — the same advisory a literal
/// color receives. It still resolves at compile time; validate just flags it.
#[test]
fn text_literal_font_size_dimension_is_raw_visual_literal() {
    let font_size = Some(PropertyValue::Dimension(px(24.0)));
    let text = match minimal_text("text.lfs", Some(token_ref("color.fill"))) {
        Node::Text(mut t) => {
            t.font_size = font_size;
            Node::Text(t)
        }
        other => other,
    };
    let doc = doc_with(
        vec![color_token("color.fill")],
        vec![minimal_page("page.one", vec![text])],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "token.raw_visual_literal"),
        "a literal font-size dimension must flag token.raw_visual_literal; codes: {:?}",
        codes(&report)
    );
}

/// A literal `font-size-min="12"` (a `PropertyValue::Dimension`, not a token)
/// must be flagged as a raw visual literal, exactly like `font-size`.
#[test]
fn text_literal_font_size_min_dimension_is_raw_visual_literal() {
    let text = match minimal_text("text.lfsm", Some(token_ref("color.fill"))) {
        Node::Text(mut t) => {
            t.font_size_min = Some(PropertyValue::Dimension(px(12.0)));
            Node::Text(t)
        }
        other => other,
    };
    let doc = doc_with(
        vec![color_token("color.fill")],
        vec![minimal_page("page.one", vec![text])],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "token.raw_visual_literal"),
        "a literal font-size-min dimension must flag token.raw_visual_literal; codes: {:?}",
        codes(&report)
    );
}

// ── polygon: unknown fill-rule warns ──────────────────────────────────

#[test]
fn polygon_unknown_fill_rule_errors() {
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![Node::Polygon(PolygonNode {
                id: "poly.fr".to_owned(),
                name: None,
                role: None,
                fill: None,
                stroke: None,
                stroke_width: None,
                stroke_alignment: None,
                fill_rule: Some("oddeven".to_owned()), // wrong spelling
                opacity: None,
                visible: None,
                locked: None,
                rotate: None,
                style: None,
                points: tri_points(),
                source_span: None,
                unknown_props: BTreeMap::new(),
            })],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "node.invalid_value"),
        "expected node.invalid_value error for bad fill-rule; codes: {:?}",
        codes(&report)
    );
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.invalid_value")
        .expect("must exist");
    assert_eq!(diag.severity, Severity::Error);
    assert!(
        diag.message.contains("fill-rule")
            && diag.message.contains("'oddeven'")
            && diag.message.contains("nonzero, evenodd"),
        "message must name property, value, allowed values; got: {}",
        diag.message
    );
    assert!(report.has_errors());
}

// ── polygon: invalid stroke-alignment warns; valid does not ───────────

#[test]
fn polygon_invalid_stroke_alignment_errors() {
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.sa",
            vec![Node::Polygon(PolygonNode {
                id: "poly.sa".to_owned(),
                name: None,
                role: None,
                fill: None,
                stroke: None,
                stroke_width: None,
                stroke_alignment: Some("middle".to_owned()), // invalid
                fill_rule: None,
                opacity: None,
                visible: None,
                locked: None,
                rotate: None,
                style: None,
                points: tri_points(),
                source_span: None,
                unknown_props: BTreeMap::new(),
            })],
        )],
    );
    let report = validate(&doc);
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.invalid_value")
        .expect("expected node.invalid_value error for bad stroke-alignment");
    assert_eq!(diag.severity, Severity::Error);
    assert!(
        diag.message.contains("stroke-alignment")
            && diag.message.contains("inside, center, outside"),
        "message must mention stroke-alignment and allowed values; got: {}",
        diag.message
    );
    assert!(report.has_errors());
}

#[test]
fn polygon_valid_stroke_alignment_no_warn() {
    for value in ["inside", "center", "outside"] {
        let doc = doc_with(
            vec![],
            vec![minimal_page(
                "page.sa",
                vec![Node::Polygon(PolygonNode {
                    id: "poly.sa".to_owned(),
                    name: None,
                    role: None,
                    fill: None,
                    stroke: None,
                    stroke_width: None,
                    stroke_alignment: Some(value.to_owned()),
                    fill_rule: None,
                    opacity: None,
                    visible: None,
                    locked: None,
                    rotate: None,
                    style: None,
                    points: tri_points(),
                    source_span: None,
                    unknown_props: BTreeMap::new(),
                })],
            )],
        );
        let report = validate(&doc);
        assert!(
            !report
                .diagnostics
                .iter()
                .any(|d| d.code == "node.invalid_value" && d.message.contains("stroke-alignment")),
            "valid stroke-alignment '{value}' must not warn; codes: {:?}",
            codes(&report)
        );
    }
}
