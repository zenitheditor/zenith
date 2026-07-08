//! Integration tests: nodes validation.
//!
//! Test bodies moved verbatim from the former in-`src` `validate/check/tests/`
//! concern files; only import paths changed (`crate::`/`super::common` ->
//! `zenith_core::`/`common`).

use std::collections::BTreeMap;

mod common;

use common::*;

// ── Test 1: clean minimal doc has no errors ───────────────────────────

#[test]
fn clean_doc_no_errors() {
    // A page with a rect and a text, both using a color token for fill.
    let doc = doc_with(
        vec![color_token("color.fill")],
        vec![minimal_page(
            "page.one",
            vec![
                minimal_rect("rect.one", Some(token_ref("color.fill"))),
                minimal_text("text.one", Some(token_ref("color.fill"))),
            ],
        )],
    );
    let report = validate(&doc);
    // The token is used twice; no unused advisory either.
    assert!(
        report.diagnostics.is_empty(),
        "expected no diagnostics, got: {:?}",
        codes(&report)
    );
    assert!(!report.has_errors());
}

#[test]
fn font_features_validate_tags_and_values() {
    let src = r##"zenith version=1 {
  project id="proj.features" name="Features"
  tokens format="zenith-token-v1" {}
  styles {}
  document id="doc.features" title="Features" {
    page id="page.one" w=(px)400 h=(px)400 {
      text id="body" x=(px)10 y=(px)10 w=(px)300 h=(px)100 font-features="liga=0,bad" {
        span "Text" font-features="kern=on" font-alternates="styleset(0)"
      }
      code id="code" x=(px)10 y=(px)140 w=(px)300 h=(px)100 font-features="toolong=1" font-alternates="character-variant(100),stylistic=on" {
        content "let x = 1;"
      }
    }
  }
}
"##;
    let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
    let report = validate(&doc);
    let invalid_count = report
        .diagnostics
        .iter()
        .filter(|d| d.code == "font.invalid_feature")
        .count();
    assert_eq!(
        invalid_count,
        6,
        "expected invalid tag/value diagnostics, got codes: {:?}",
        codes(&report)
    );
}

#[test]
fn letter_spacing_validates_dimension_tokens() {
    let src = r##"zenith version=1 {
  project id="proj.spacing" name="Spacing"
  tokens format="zenith-token-v1" {
    token id="size.track" type="dimension" value=(px)1
  }
  styles {}
  document id="doc.spacing" title="Spacing" {
    page id="page.one" w=(px)400 h=(px)400 {
      text id="body" x=(px)10 y=(px)10 w=(px)300 h=(px)100 letter-spacing=(token)"size.track" {
        span "Text" letter-spacing=(token)"size.track"
      }
      code id="code" x=(px)10 y=(px)140 w=(px)300 h=(px)100 letter-spacing=(token)"size.track" {
        content "let x = 1;"
      }
    }
  }
}
"##;
    let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
    let report = validate(&doc);
    assert!(
        !report.has_errors(),
        "expected no errors, got codes: {:?}",
        codes(&report)
    );
}

#[test]
fn letter_spacing_rejects_wrong_token_type() {
    let src = r##"zenith version=1 {
  project id="proj.spacing.bad" name="Spacing Bad"
  tokens format="zenith-token-v1" {
    token id="color.ink" type="color" value="#111111"
  }
  styles {}
  document id="doc.spacing.bad" title="Spacing Bad" {
    page id="page.one" w=(px)400 h=(px)400 {
      text id="body" x=(px)10 y=(px)10 w=(px)300 h=(px)100 letter-spacing=(token)"color.ink" {
        span "Text" letter-spacing=(token)"color.ink"
      }
      code id="code" x=(px)10 y=(px)140 w=(px)300 h=(px)100 letter-spacing=(token)"color.ink" {
        content "let x = 1;"
      }
    }
  }
}
"##;
    let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
    let report = validate(&doc);
    let incompatible_count = report
        .diagnostics
        .iter()
        .filter(|d| d.code == "token.incompatible_property")
        .count();
    assert_eq!(
        incompatible_count,
        3,
        "expected text/span/code incompatible-property diagnostics, got codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

// ── Test 2: duplicate id across two nodes ─────────────────────────────

#[test]
fn duplicate_node_id_produces_id_duplicate() {
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![
                minimal_rect("node.dup", None),
                minimal_rect("node.dup", None),
            ],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "id.duplicate"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

// ── Test 3: rect missing w ────────────────────────────────────────────

#[test]
fn rect_missing_w_produces_node_missing_geometry() {
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![Node::Rect(Box::new(RectNode {
                shadow: None,
                filter: None,
                mask: None,
                id: "rect.no-w".to_owned(),
                name: None,
                role: None,
                x: Some(pxv(0.0)),
                y: Some(pxv(0.0)),
                w: None, // missing
                h: Some(pxv(100.0)),
                radius: None,
                radius_tl: None,
                radius_tr: None,
                radius_br: None,
                radius_bl: None,
                style: None,
                fill: None,
                stroke: None,
                stroke_width: None,
                stroke_alignment: None,
                stroke_dash: None,
                stroke_gap: None,
                stroke_linecap: None,
                border_top: None,
                border_bottom: None,
                border_left: None,
                border_right: None,
                border_width: None,
                stroke_outer: None,
                stroke_outer_width: None,
                opacity: None,
                visible: None,
                locked: None,
                rotate: None,
                blend_mode: None,
                blur: None,
                anchor: None,
                anchor_zone: None,
                anchor_sibling: None,
                anchor_edge: None,
                anchor_gap: None,
                anchor_parent: None,
                source_span: None,
                unknown_props: BTreeMap::new(),
            }))],
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

// ── Test 4: fill referencing a missing token ──────────────────────────

#[test]
fn fill_with_missing_token_ref_produces_unknown_reference() {
    let doc = doc_with(
        vec![], // no tokens defined
        vec![minimal_page(
            "page.one",
            vec![minimal_rect(
                "rect.one",
                Some(token_ref("color.does.not.exist")),
            )],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "token.unknown_reference"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

#[test]
fn mesh_perspective_requires_vanishing_point() {
    let src = r##"zenith version=1 {
  project id="proj.mesh" name="Mesh"
  tokens format="zenith-token-v1" {
    token id="color.grid" type="color" value="#203040"
  }
  styles {}
  document id="doc.mesh" title="Mesh" {
    page id="page.mesh" w=(px)300 h=(px)200 {
      mesh id="grid" kind="perspective" x=(px)0 y=(px)0 w=(px)300 h=(px)200 rows=4 columns=4 stroke=(token)"color.grid" stroke-width=(px)1
    }
  }
}
"##;
    let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
    let report = validate(&doc);
    assert!(
        has_code(&report, "node.missing_geometry"),
        "perspective mesh without vanishing point must report missing geometry; codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

#[test]
fn mesh_zero_rows_and_columns_are_invalid() {
    let src = r##"zenith version=1 {
  project id="proj.mesh" name="Mesh"
  tokens format="zenith-token-v1" {
    token id="color.grid" type="color" value="#203040"
  }
  styles {}
  document id="doc.mesh" title="Mesh" {
    page id="page.mesh" w=(px)300 h=(px)200 {
      mesh id="grid" x=(px)0 y=(px)0 w=(px)300 h=(px)200 rows=0 columns=0 stroke=(token)"color.grid" stroke-width=(px)1
    }
  }
}
"##;
    let doc = KdlAdapter.parse(src.as_bytes()).expect("parse");
    let report = validate(&doc);
    assert!(
        has_code(&report, "mesh.invalid_rows"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(
        has_code(&report, "mesh.invalid_columns"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

// ── Test 4b: font-weight referencing a missing token ──────────────────

#[test]
fn font_weight_with_missing_token_ref_produces_unknown_reference() {
    let text = Node::Text(Box::new(TextNode {
        shadow: None,
        filter: None,
        mask: None,
        id: "text.fw".to_owned(),
        name: None,
        role: None,
        x: Some(pxv(0.0)),
        y: Some(pxv(0.0)),
        w: Some(pxv(200.0)),
        h: Some(pxv(40.0)),
        align: None,
        v_align: None,
        direction: None,
        overflow: None,
        overflow_wrap: None,
        style: None,
        fill: None,
        stroke: None,
        stroke_width: None,
        contrast_bg: None,
        font_family: None,
        font_size: None,
        font_size_min: None,
        font_weight: Some(token_ref("weight.does.not.exist")),
        font_features: None,
        font_alternates: None,
        letter_spacing: None,
        kerning_pairs: Vec::new(),
        opacity: None,
        visible: None,
        locked: None,
        selectable: None,
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
    }));
    let doc = doc_with(vec![], vec![minimal_page("page.one", vec![text])]);
    let report = validate(&doc);
    assert!(
        has_code(&report, "token.unknown_reference"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

// ── Test 5: fill referencing a fontFamily token (wrong type) ──────────

#[test]
fn fill_with_wrong_type_token_produces_incompatible_property() {
    let doc = doc_with(
        vec![font_family_token("font.body")],
        vec![minimal_page(
            "page.one",
            vec![minimal_rect("rect.one", Some(token_ref("font.body")))],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "token.incompatible_property"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

// ── Test 6: fill="#ff0000" raw literal → raw_visual_literal ──────────

#[test]
fn fill_raw_literal_produces_raw_visual_literal() {
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![minimal_rect(
                "rect.one",
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

// ── Test 7: unknown node kind → node.unknown_kind (Warning) ──────────

#[test]
fn unknown_node_kind_produces_warning_not_error() {
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![Node::Unknown(Box::new(UnknownNode {
                kind: "sparkle".to_owned(),
                id: None,
                unknown_props: std::collections::BTreeMap::new(),
                children: Vec::new(),
                source_span: None,
            }))],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "node.unknown_kind"),
        "codes: {:?}",
        codes(&report)
    );
    // Must NOT be an error.
    assert!(
        !report.has_errors(),
        "unknown_kind should be Warning, not Error. codes: {:?}",
        codes(&report)
    );
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_kind")
        .expect("should exist");
    assert_eq!(diag.severity, Severity::Warning);
}

/// **Unknown node id participates in duplicate-id detection**: an unknown node
/// whose `id` collides with another node's id must trigger `id.duplicate`,
/// proving the unknown node's id is registered like a known node's.
#[test]
fn unknown_node_id_participates_in_duplicate_detection() {
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![
                minimal_rect("node.dup", None),
                unknown_node("sparkle", Some("node.dup"), vec![]),
            ],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "id.duplicate"),
        "unknown node id must participate in duplicate detection. codes: {:?}",
        codes(&report)
    );
    // It still warns about the unknown kind.
    assert!(
        has_code(&report, "node.unknown_kind"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

/// **Known child of an unknown node is still validated**: a `rect` with a bad
/// token ref declared inside an unknown parent must still produce its
/// `token.unknown_reference` diagnostic, proving child recursion descends into
/// known children of unknown nodes.
#[test]
fn known_child_of_unknown_node_is_validated() {
    let doc = doc_with(
        vec![], // no tokens defined → the ref is dangling
        vec![minimal_page(
            "page.one",
            vec![unknown_node(
                "sparkle",
                Some("fx"),
                vec![minimal_rect(
                    "inner.rect",
                    Some(token_ref("color.does.not.exist")),
                )],
            )],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "token.unknown_reference"),
        "a known child's bad token ref must be validated. codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

// ── Test 8: defined-but-unreferenced token → token.unused (Advisory) ─

#[test]
fn unused_token_produces_advisory() {
    // Define two color tokens; only reference one of them.
    let doc = doc_with(
        vec![color_token("color.used"), color_token("color.unused")],
        vec![minimal_page(
            "page.one",
            vec![minimal_rect("rect.one", Some(token_ref("color.used")))],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "token.unused"),
        "codes: {:?}",
        codes(&report)
    );
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "token.unused")
        .expect("should exist");
    assert_eq!(diag.severity, Severity::Advisory);
    // Advisory only — no errors.
    assert!(
        !report.has_errors(),
        "should not be error, codes: {:?}",
        codes(&report)
    );
    // The unused subject should be the unreferenced token.
    assert_eq!(diag.subject_id.as_deref(), Some("color.unused"));
}

// ── Test 8b: set-aware token.unused / token.set_partially_used ─────────

/// A single-token `set` (only one token carries a given `set` id) behaves
/// exactly like the no-`set` case: an unreferenced member still gets a plain
/// per-token `token.unused`, not `token.set_partially_used`.
#[test]
fn singleton_set_still_produces_plain_token_unused() {
    let doc = doc_with(
        vec![color_token_with_set("color.lonely", "@zenith/theme.solo")],
        vec![minimal_page("page.one", vec![])],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "token.unused"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(
        !has_code(&report, "token.set_partially_used"),
        "a singleton set must not emit token.set_partially_used; codes: {:?}",
        codes(&report)
    );
}

/// A multi-token `set` with none of its members referenced collapses into a
/// single `token.set_partially_used` advisory; no per-token `token.unused` is
/// emitted for any set member.
#[test]
fn multi_token_set_fully_unused_collapses_to_one_advisory() {
    let doc = doc_with(
        vec![
            color_token_with_set("color.a", "@zenith/theme.cobalt"),
            color_token_with_set("color.b", "@zenith/theme.cobalt"),
            color_token_with_set("color.c", "@zenith/theme.cobalt"),
        ],
        vec![minimal_page("page.one", vec![])],
    );
    let report = validate(&doc);
    assert!(
        !has_code(&report, "token.unused"),
        "multi-token set must suppress per-token token.unused; codes: {:?}",
        codes(&report)
    );
    let matches: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.code == "token.set_partially_used")
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "exactly one set_partially_used advisory expected; got: {:?}",
        codes(&report)
    );
    assert_eq!(matches[0].severity, Severity::Advisory);
    assert_eq!(
        matches[0].subject_id.as_deref(),
        Some("@zenith/theme.cobalt")
    );
    assert!(
        matches[0].message.contains("none of 3 tokens referenced"),
        "message should report 0/3; got: {}",
        matches[0].message
    );
}

/// A multi-token `set` with some (but not all) members referenced still
/// collapses into a single `token.set_partially_used` advisory reporting the
/// used/total split.
#[test]
fn multi_token_set_partially_used_reports_used_count() {
    let doc = doc_with(
        vec![
            color_token_with_set("color.a", "@zenith/theme.cobalt"),
            color_token_with_set("color.b", "@zenith/theme.cobalt"),
            color_token_with_set("color.c", "@zenith/theme.cobalt"),
        ],
        vec![minimal_page(
            "page.one",
            vec![minimal_rect("rect.one", Some(token_ref("color.a")))],
        )],
    );
    let report = validate(&doc);
    assert!(
        !has_code(&report, "token.unused"),
        "multi-token set must suppress per-token token.unused; codes: {:?}",
        codes(&report)
    );
    let matches: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.code == "token.set_partially_used")
        .collect();
    assert_eq!(matches.len(), 1, "codes: {:?}", codes(&report));
    assert!(
        matches[0].message.contains("1 of 3 tokens referenced"),
        "message should report 1/3; got: {}",
        matches[0].message
    );
}

/// A multi-token `set` with every member referenced emits nothing at all for
/// that set — no `token.unused`, no `token.set_partially_used`.
#[test]
fn multi_token_set_fully_used_emits_nothing() {
    let doc = doc_with(
        vec![
            color_token_with_set("color.a", "@zenith/theme.cobalt"),
            color_token_with_set("color.b", "@zenith/theme.cobalt"),
        ],
        vec![minimal_page(
            "page.one",
            vec![
                minimal_rect("rect.one", Some(token_ref("color.a"))),
                minimal_rect("rect.two", Some(token_ref("color.b"))),
            ],
        )],
    );
    let report = validate(&doc);
    assert!(
        !has_code(&report, "token.unused"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(
        !has_code(&report, "token.set_partially_used"),
        "fully-used set must emit nothing; codes: {:?}",
        codes(&report)
    );
}

// ── Bonus: duplicate id between token and node ────────────────────────

#[test]
fn duplicate_id_token_vs_node() {
    // Token id collides with node id.
    let doc = doc_with(
        vec![color_token("shared.id")],
        vec![minimal_page(
            "page.one",
            vec![minimal_rect("shared.id", Some(token_ref("shared.id")))],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "id.duplicate"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

// ── Bonus: page with unknown unit on width ────────────────────────────

#[test]
fn page_unknown_unit_produces_invalid_geometry() {
    let doc = doc_with(
        vec![],
        vec![Page {
            id: "page.bad".to_owned(),
            name: None,
            source: None,
            fit: None,
            width: Dimension {
                value: 1280.0,
                unit: Unit::Unknown("em".to_owned()),
            },
            height: px(720.0),
            background: None,
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
            construction: zenith_core::ConstructionBlock::default(),
            block_styles: Vec::new(),
            children: vec![],
            source_span: None,
        }],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "node.invalid_geometry"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

// ── Page dimensions must be strictly positive ────────────────────────

/// Build a single-page document whose page has the given width/height dims.
fn doc_with_page_dims(width: Dimension, height: Dimension) -> Document {
    let mut page = minimal_page("page.dim", vec![]);
    page.width = width;
    page.height = height;
    doc_with(vec![], vec![page])
}

#[test]
fn page_zero_width_produces_out_of_range() {
    let report = validate(&doc_with_page_dims(px(0.0), px(720.0)));
    assert!(
        has_code(&report, "value.out_of_range"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

#[test]
fn page_negative_height_produces_out_of_range() {
    let report = validate(&doc_with_page_dims(px(1280.0), px(-100.0)));
    assert!(
        has_code(&report, "value.out_of_range"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

#[test]
fn page_positive_dims_no_out_of_range() {
    let report = validate(&doc_with_page_dims(px(1280.0), px(720.0)));
    assert!(
        !has_code(&report, "value.out_of_range"),
        "positive dims must not trip value.out_of_range; codes: {:?}",
        codes(&report)
    );
}

// ── A document must contain at least one page ────────────────────────

#[test]
fn empty_document_produces_no_pages_error() {
    let report = validate(&doc_with(vec![], vec![]));
    assert!(
        has_code(&report, "document.no_pages"),
        "codes: {:?}",
        codes(&report)
    );
    assert!(report.has_errors());
}

// ── Bonus: node with unknown property → node.unknown_property ─────────

#[test]
fn unknown_property_on_rect_produces_warning() {
    let mut unknown_props = BTreeMap::new();
    unknown_props.insert(
        "magic-glow".to_owned(),
        zenith_core::UnknownProperty {
            value: zenith_core::UnknownValue::String("true".to_owned()),
            ty: None,
        },
    );
    let doc = doc_with(
        vec![],
        vec![minimal_page(
            "page.one",
            vec![Node::Rect(Box::new(RectNode {
                shadow: None,
                filter: None,
                mask: None,
                id: "rect.one".to_owned(),
                name: None,
                role: None,
                x: Some(pxv(0.0)),
                y: Some(pxv(0.0)),
                w: Some(pxv(50.0)),
                h: Some(pxv(50.0)),
                radius: None,
                radius_tl: None,
                radius_tr: None,
                radius_br: None,
                radius_bl: None,
                style: None,
                fill: None,
                stroke: None,
                stroke_width: None,
                stroke_alignment: None,
                stroke_dash: None,
                stroke_gap: None,
                stroke_linecap: None,
                border_top: None,
                border_bottom: None,
                border_left: None,
                border_right: None,
                border_width: None,
                stroke_outer: None,
                stroke_outer_width: None,
                opacity: None,
                visible: None,
                locked: None,
                rotate: None,
                blend_mode: None,
                blur: None,
                anchor: None,
                anchor_zone: None,
                anchor_sibling: None,
                anchor_edge: None,
                anchor_gap: None,
                anchor_parent: None,
                source_span: None,
                unknown_props,
            }))],
        )],
    );
    let report = validate(&doc);
    assert!(
        has_code(&report, "node.unknown_property"),
        "codes: {:?}",
        codes(&report)
    );
    let diag = report
        .diagnostics
        .iter()
        .find(|d| d.code == "node.unknown_property")
        .expect("should exist");
    assert_eq!(diag.severity, Severity::Warning);
    assert!(!report.has_errors());
}
