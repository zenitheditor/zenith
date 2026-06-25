//! Integration tests: contrast validation.
//!
//! Test bodies moved verbatim from the former in-`src` `validate/check/tests/`
//! concern files; only import paths changed (`crate::`/`super::common` ->
//! `zenith_core::`/`common`).

use std::collections::BTreeMap;

mod common;

use common::*;

// ══════════════════════════════════════════════════════════════════════
// WCAG 3 (APCA) contrast advisory tests
// ══════════════════════════════════════════════════════════════════════

/// Build a dimension token in pt.
fn dim_token_pt(id: &str, value: f64) -> Token {
    Token {
        id: id.to_owned(),
        token_type: TokenType::Dimension,
        value: TokenValue::Literal(TokenLiteral::Dimension(Dimension {
            value,
            unit: Unit::Pt,
        })),
        source_span: None,
    }
}

/// Build a font-weight token.
fn fw_token(id: &str, weight: f64) -> Token {
    Token {
        id: id.to_owned(),
        token_type: TokenType::FontWeight,
        value: TokenValue::Literal(TokenLiteral::Number(weight)),
        source_span: None,
    }
}

/// Helper: build a page with a background color token reference.
fn page_with_bg(id: &str, bg_token_id: &str, children: Vec<Node>) -> Page {
    Page {
        id: id.to_owned(),
        name: None,
        width: px(1280.0),
        height: px(720.0),
        background: Some(PropertyValue::TokenRef(bg_token_id.to_owned())),
        bleed: None,
        margin_inner: None,
        margin_outer: None,
        margin_top: None,
        margin_bottom: None,
        baseline_grid: None,
        line_jumps: None,
        parity: None,
        master: None,
        safe_zones: Vec::new(),
        folds: Vec::new(),
        block_styles: Vec::new(),
        children,
        source_span: None,
    }
}

/// Build a text node with explicit fill and optional font-size / font-weight.
fn text_with_fill_and_size(
    id: &str,
    fill_token: Option<&str>,
    font_size_token: Option<&str>,
    font_weight_token: Option<&str>,
) -> Node {
    Node::Text(Box::new(zenith_core::TextNode {
        shadow: None,
        filter: None,
        mask: None,
        id: id.to_owned(),
        name: None,
        role: None,
        x: Some(px(0.0)),
        y: Some(px(0.0)),
        w: Some(px(200.0)),
        h: Some(px(40.0)),
        align: None,
        v_align: None,
        direction: None,
        overflow: None,
        overflow_wrap: None,
        style: None,
        fill: fill_token.map(|t| PropertyValue::TokenRef(t.to_owned())),
        stroke: None,
        stroke_width: None,
        contrast_bg: None,
        font_family: None,
        font_size: font_size_token.map(|t| PropertyValue::TokenRef(t.to_owned())),
        font_size_min: None,
        font_weight: font_weight_token.map(|t| PropertyValue::TokenRef(t.to_owned())),
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        blend_mode: None,
        blur: None,
        chain: None,
        drop_cap_lines: None,
        hyphenate: None,
        widow_orphan: None,
        tab_leader: None,
        text_exclusion: None,
        padding_left: None,
        text_indent: None,
        content_format: None,
        src: None,
        bullet: None,
        bullet_gap: None,
        anchor: None,
        anchor_zone: None,
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        spans: vec![],
        block_styles: Vec::new(),
        source_span: None,
        unknown_props: BTreeMap::new(),
    }))
}

/// Light gray (#aaaaaa) text on white page at 16 px → APCA Lc ~46 < 60
/// → `contrast.low` warning.
#[test]
fn low_contrast_normal_text_warns() {
    let doc = doc_with(
        vec![
            color_token_hex("color.bg", "#ffffff"),
            color_token_hex("color.text", "#aaaaaa"),
        ],
        vec![page_with_bg(
            "page.one",
            "color.bg",
            vec![text_with_fill_and_size(
                "text.one",
                Some("color.text"),
                None,
                None,
            )],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "contrast.low"),
        "light gray on white should warn contrast.low; codes: {:?}",
        codes(&report)
    );
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "contrast.low")
        .expect("must exist");
    assert_eq!(diag.severity, Severity::Warning);
    assert!(!report.has_errors(), "contrast.low must not be an error");
}

/// Black (#000000) text on white page → APCA Lc ~106 → NO warning.
#[test]
fn high_contrast_text_no_warning() {
    let doc = doc_with(
        vec![
            color_token_hex("color.bg", "#ffffff"),
            color_token_hex("color.text", "#000000"),
        ],
        vec![page_with_bg(
            "page.one",
            "color.bg",
            vec![text_with_fill_and_size(
                "text.one",
                Some("color.text"),
                None,
                None,
            )],
        )],
    );
    let report = validate(&doc);
    assert!(
        !has_code(&report, "contrast.low"),
        "black on white must NOT warn contrast.low; codes: {:?}",
        codes(&report)
    );
}

/// Large text (20 pt ≈ 26.67 px, which is >= 24 px) with a mid-contrast
/// color (#777777, APCA Lc ~71 on white) clears the large-text minimum
/// (Lc 45) → NO warning.
///
/// Note: 20 pt × (4/3) = 26.67 px, which exceeds the 24 px large-text cut-off.
#[test]
fn large_text_passes_lower_threshold_no_warning() {
    let doc = doc_with(
        vec![
            color_token_hex("color.bg", "#ffffff"),
            color_token_hex("color.text", "#777777"), // APCA Lc ~71 on white — clears large min (45)
            dim_token_pt("size.large", 20.0),         // 20pt ≈ 26.67px >= 24px → large
        ],
        vec![page_with_bg(
            "page.one",
            "color.bg",
            vec![text_with_fill_and_size(
                "text.one",
                Some("color.text"),
                Some("size.large"),
                None,
            )],
        )],
    );
    let report = validate(&doc);
    assert!(
        !has_code(&report, "contrast.low"),
        "large text (#777 on white, Lc ~71) should pass the 45 large-text threshold; codes: {:?}",
        codes(&report)
    );
}

/// Small bold text (18 pt ≈ 24 px, which is exactly 24 px → large) with
/// mid-contrast (#777777, APCA Lc ~71 on white) → clears large min (45) → NO warning.
#[test]
fn bold_large_text_passes_lower_threshold() {
    let doc = doc_with(
        vec![
            color_token_hex("color.bg", "#ffffff"),
            color_token_hex("color.text", "#777777"),
            dim_token_pt("size.18pt", 18.0), // 18pt ≈ 24px → exactly at large boundary
            fw_token("weight.bold", 700.0),
        ],
        vec![page_with_bg(
            "page.one",
            "color.bg",
            vec![text_with_fill_and_size(
                "text.one",
                Some("color.text"),
                Some("size.18pt"),
                Some("weight.bold"),
            )],
        )],
    );
    let report = validate(&doc);
    assert!(
        !has_code(&report, "contrast.low"),
        "18pt bold (large text, Lc ~71) should clear the 45 large-text threshold; codes: {:?}",
        codes(&report)
    );
}

/// Text node with no fill → no contrast check → no warning.
#[test]
fn text_without_fill_skips_contrast_check() {
    let doc = doc_with(
        vec![color_token_hex("color.bg", "#ffffff")],
        vec![page_with_bg(
            "page.one",
            "color.bg",
            vec![text_with_fill_and_size("text.one", None, None, None)],
        )],
    );
    let report = validate(&doc);
    assert!(
        !has_code(&report, "contrast.low"),
        "text with no fill must not produce contrast.low; codes: {:?}",
        codes(&report)
    );
}

/// Page with no background token → contrast checks are skipped entirely.
#[test]
fn no_page_background_skips_contrast_check() {
    let doc = doc_with(
        vec![color_token_hex("color.text", "#aaaaaa")],
        vec![minimal_page(
            "page.one",
            vec![text_with_fill_and_size(
                "text.one",
                Some("color.text"),
                None,
                None,
            )],
        )],
    );
    let report = validate(&doc);
    assert!(
        !has_code(&report, "contrast.low"),
        "page with no background must not produce contrast.low; codes: {:?}",
        codes(&report)
    );
}

/// Build a text node with an explicit fill token AND a `contrast-bg` hint token.
fn text_with_fill_and_contrast_bg(id: &str, fill_token: &str, contrast_bg_token: &str) -> Node {
    Node::Text(Box::new(zenith_core::TextNode {
        shadow: None,
        filter: None,
        mask: None,
        id: id.to_owned(),
        name: None,
        role: None,
        x: Some(px(0.0)),
        y: Some(px(0.0)),
        w: Some(px(200.0)),
        h: Some(px(40.0)),
        align: None,
        v_align: None,
        direction: None,
        overflow: None,
        overflow_wrap: None,
        style: None,
        fill: Some(PropertyValue::TokenRef(fill_token.to_owned())),
        stroke: None,
        stroke_width: None,
        contrast_bg: Some(PropertyValue::TokenRef(contrast_bg_token.to_owned())),
        font_family: None,
        font_size: None,
        font_size_min: None,
        font_weight: None,
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        blend_mode: None,
        blur: None,
        chain: None,
        drop_cap_lines: None,
        hyphenate: None,
        widow_orphan: None,
        tab_leader: None,
        text_exclusion: None,
        padding_left: None,
        text_indent: None,
        content_format: None,
        src: None,
        bullet: None,
        bullet_gap: None,
        anchor: None,
        anchor_zone: None,
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        spans: vec![],
        block_styles: Vec::new(),
        source_span: None,
        unknown_props: BTreeMap::new(),
    }))
}

/// A `contrast-bg` hint takes TOP priority over the page background: a dark fill
/// with a dark `contrast-bg` on a WHITE page must still warn `contrast.low`
/// (judged against the hint, not the page bg), and the message names the hint.
#[test]
fn contrast_bg_hint_used_as_background() {
    // Dark hint + dark fill → low contrast despite the white page bg.
    let dark = doc_with(
        vec![
            color_token_hex("color.bg", "#ffffff"),
            color_token_hex("color.text", "#222222"),
            color_token_hex("color.photo.shadow", "#101010"),
        ],
        vec![page_with_bg(
            "page.one",
            "color.bg",
            vec![text_with_fill_and_contrast_bg(
                "coverline",
                "color.text",
                "color.photo.shadow",
            )],
        )],
    );
    let report = validate(&dark);
    assert!(
        has_code(&report, "contrast.low"),
        "dark fill on a dark contrast-bg hint must warn contrast.low; codes: {:?}",
        codes(&report)
    );
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "contrast.low")
        .expect("must exist");
    assert!(
        diag.message.contains("contrast-bg hint"),
        "message must name the contrast-bg hint as the bg source; got: {}",
        diag.message
    );

    // Light hint + dark fill → high contrast → NO warning (hint overrides bg).
    let light = doc_with(
        vec![
            color_token_hex("color.bg", "#000000"),
            color_token_hex("color.text", "#111111"),
            color_token_hex("color.photo.light", "#fafafa"),
        ],
        vec![page_with_bg(
            "page.one",
            "color.bg",
            vec![text_with_fill_and_contrast_bg(
                "coverline",
                "color.text",
                "color.photo.light",
            )],
        )],
    );
    let report = validate(&light);
    assert!(
        !has_code(&report, "contrast.low"),
        "dark fill on a light contrast-bg hint must NOT warn contrast.low; codes: {:?}",
        codes(&report)
    );
}

// ── Table cell fill regression tests ──────────────────────────────────────────

/// Build a minimal `TableNode` with one body row containing one cell, where the
/// cell holds a single text child.
fn table_with_cell_text(
    cell_fill: Option<PropertyValue>,
    table_fill: Option<PropertyValue>,
    header_fill: Option<PropertyValue>,
    header_rows: Option<u32>,
    text_fill_token: &str,
) -> Node {
    let text = minimal_text(
        "cell.text",
        Some(PropertyValue::TokenRef(text_fill_token.to_owned())),
    );
    let cell = TableCell {
        colspan: 1,
        rowspan: 1,
        children: vec![text],
        fill: cell_fill,
        border: None,
        border_width: None,
        h_align: None,
        v_align: None,
        source_span: None,
        unknown_props: BTreeMap::new(),
    };
    let row = TableRow {
        cells: vec![cell],
        source_span: None,
        unknown_props: BTreeMap::new(),
    };
    Node::Table(Box::new(TableNode {
        id: "table.one".to_owned(),
        name: None,
        role: None,
        x: Some(px(0.0)),
        y: Some(px(0.0)),
        w: Some(px(400.0)),
        h: Some(px(200.0)),
        columns: vec![],
        rows: vec![row],
        header_rows,
        flows: None,
        gap: None,
        cell_padding: None,
        border_collapse: None,
        fill: table_fill,
        border: None,
        border_width: None,
        header_fill,
        header_style: None,
        h_align: None,
        v_align: None,
        style: None,
        opacity: None,
        visible: None,
        locked: None,
        rotate: None,
        anchor: None,
        anchor_zone: None,
        anchor_sibling: None,
        anchor_edge: None,
        anchor_gap: None,
        anchor_parent: None,
        source_span: None,
        unknown_props: BTreeMap::new(),
    }))
}

/// White text (`#ffffff`) in a dark-blue-filled cell (`#003087`) on a white
/// page must NOT fire `contrast.low` — the cell fill is the effective bg.
/// APCA Lc of white on #003087 ≈ 83, which clears the Lc 60 threshold.
#[test]
fn white_text_in_dark_cell_no_false_positive() {
    let doc = doc_with(
        vec![
            color_token_hex("color.page", "#ffffff"),
            color_token_hex("color.cell", r##"#003087"##),
            color_token_hex("color.text", "#ffffff"),
        ],
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![table_with_cell_text(
                Some(PropertyValue::TokenRef("color.cell".to_owned())),
                None,
                None,
                None,
                "color.text",
            )],
        )],
    );
    let report = validate(&doc);
    assert!(
        !has_code(&report, "contrast.low"),
        "white text in a dark-blue cell should NOT warn contrast.low (cell fill is bg); codes: {:?}",
        codes(&report)
    );
}

/// White text (`#ffffff`) in a light-gray-filled cell (`#dddddd`) on a white
/// page SHOULD still fire `contrast.low` — the cell fill is the bg and it gives
/// insufficient contrast. APCA Lc of white on #dddddd ≈ 21 < 60.
#[test]
fn white_text_in_light_cell_still_warns() {
    let doc = doc_with(
        vec![
            color_token_hex("color.page", "#ffffff"),
            color_token_hex("color.cell", r##"#dddddd"##),
            color_token_hex("color.text", "#ffffff"),
        ],
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![table_with_cell_text(
                Some(PropertyValue::TokenRef("color.cell".to_owned())),
                None,
                None,
                None,
                "color.text",
            )],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "contrast.low"),
        "white text in a light-gray cell should warn contrast.low; codes: {:?}",
        codes(&report)
    );
}

/// When a cell has NO fill and the table has NO fill, the check must fall back
/// to the page background — existing behavior is preserved.
#[test]
fn cell_no_fill_falls_back_to_page_bg() {
    // Light gray text (#aaaaaa) on white page → Lc ~46 < 60 → warns.
    let doc = doc_with(
        vec![
            color_token_hex("color.page", "#ffffff"),
            color_token_hex("color.text", r##"#aaaaaa"##),
        ],
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![table_with_cell_text(None, None, None, None, "color.text")],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "contrast.low"),
        "light-gray text in an unfilled cell must still warn via page-bg fallback; codes: {:?}",
        codes(&report)
    );
}

/// Table-level `fill` is used as the cell bg when cell has no per-cell fill.
/// White text (`#ffffff`) on a dark table fill (`#003087`) should NOT warn.
#[test]
fn table_fill_used_when_cell_has_no_fill() {
    let doc = doc_with(
        vec![
            color_token_hex("color.page", "#ffffff"),
            color_token_hex("color.table", r##"#003087"##),
            color_token_hex("color.text", "#ffffff"),
        ],
        vec![page_with_bg(
            "page.one",
            "color.page",
            vec![table_with_cell_text(
                None,
                Some(PropertyValue::TokenRef("color.table".to_owned())),
                None,
                None,
                "color.text",
            )],
        )],
    );
    let report = validate(&doc);
    assert!(
        !has_code(&report, "contrast.low"),
        "white text on dark table.fill should NOT warn; codes: {:?}",
        codes(&report)
    );
}

/// A raw literal `contrast-bg` value is rejected as `token.raw_visual_literal`,
/// consistent with `fill`/`stroke`.
#[test]
fn contrast_bg_literal_rejected() {
    let mut text = match text_with_fill_and_contrast_bg("t", "color.text", "color.bg") {
        Node::Text(t) => t,
        _ => unreachable!(),
    };
    // Overwrite the hint with a RAW literal.
    text.contrast_bg = Some(PropertyValue::Literal("#000000".to_owned()));
    let doc = doc_with(
        vec![
            color_token_hex("color.bg", "#ffffff"),
            color_token_hex("color.text", "#000000"),
        ],
        vec![page_with_bg("page.one", "color.bg", vec![Node::Text(text)])],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "token.raw_visual_literal"),
        "a raw-literal contrast-bg must flag token.raw_visual_literal; codes: {:?}",
        codes(&report)
    );
}
